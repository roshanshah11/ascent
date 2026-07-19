use ascent_domain::Motor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::SimEngine;
use crate::events::{Event, EventKind};
use crate::rocket::{Environment, Rocket};
use crate::sim::{SimConfig, SimResult};
use crate::summary::SimSummary;

const STATE_LEN: usize = 13;
type State = [f64; STATE_LEN];

const POSITION: usize = 0;
const VELOCITY: usize = 3;
const ATTITUDE: usize = 6;
const ANGULAR_RATE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlightPhase {
    Pad,
    Rail,
    Ascent,
    Descent,
    Grounded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SixDofSample {
    pub time_s: f64,
    pub position_m: [f64; 3],
    pub velocity_ms: [f64; 3],
    pub attitude_wxyz: [f64; 4],
    pub angular_rate_rad_s: [f64; 3],
    pub phase: FlightPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SixDofResult {
    pub summary: SimSummary,
    pub history: Vec<SixDofSample>,
    pub landing_position_m: [f64; 3],
    pub weathercock_pitch_deg: f64,
}

/// Tree-derived rigid-body and aerodynamic inputs supplied by the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SixDofVehicle {
    pub cg_from_nose_m: f64,
    pub pitch_yaw_inertia_kgm2: f64,
    pub cp_from_nose_m: f64,
    pub cn_alpha_per_rad: f64,
    pub reference_area_m2: f64,
}

/// One piecewise-constant, three-dimensional inertial wind layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wind3DLayer {
    pub altitude_m: f64,
    pub velocity_ms: [f64; 3],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Wind3DProfile {
    pub layers: Vec<Wind3DLayer>,
}

impl Wind3DProfile {
    pub fn calm() -> Self {
        Self::default()
    }

    fn velocity_at(&self, altitude_m: f64) -> [f64; 3] {
        self.layers
            .iter()
            .filter(|layer| layer.altitude_m <= altitude_m)
            .max_by(|a, b| a.altitude_m.total_cmp(&b.altitude_m))
            .map(|layer| layer.velocity_ms)
            .unwrap_or([0.0; 3])
    }
}

/// Launch attitude relative to the local vertical inertial frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SixDofLaunch {
    pub tilt_rad: f64,
    pub azimuth_rad: f64,
}

impl SixDofLaunch {
    pub fn vertical() -> Self {
        Self {
            tilt_rad: 0.0,
            azimuth_rad: 0.0,
        }
    }
}

/// Immutable configured six-degree-of-freedom engine.
pub struct SixDofEngine {
    vehicle: SixDofVehicle,
    wind: Wind3DProfile,
    launch: SixDofLaunch,
}

impl SixDofEngine {
    pub fn new(
        vehicle: SixDofVehicle,
        wind: Wind3DProfile,
        launch: SixDofLaunch,
    ) -> Result<Self, String> {
        let positive = [
            ("pitch/yaw inertia", vehicle.pitch_yaw_inertia_kgm2),
            ("reference area", vehicle.reference_area_m2),
        ];
        for (name, value) in positive {
            if !value.is_finite() || value <= 0.0 {
                return Err(format!("{name} must be finite and positive"));
            }
        }
        for (name, value) in [
            ("CG", vehicle.cg_from_nose_m),
            ("CP", vehicle.cp_from_nose_m),
            ("CN-alpha", vehicle.cn_alpha_per_rad),
            ("launch tilt", launch.tilt_rad),
            ("launch azimuth", launch.azimuth_rad),
        ] {
            if !value.is_finite() {
                return Err(format!("{name} must be finite"));
            }
        }
        if vehicle.cn_alpha_per_rad < 0.0 {
            return Err("CN-alpha must be non-negative".into());
        }
        if vehicle.cg_from_nose_m < 0.0 {
            return Err("CG from nose must be non-negative".into());
        }
        if vehicle.cp_from_nose_m < 0.0 {
            return Err("CP from nose must be non-negative".into());
        }
        if !(0.0..=std::f64::consts::FRAC_PI_4).contains(&launch.tilt_rad) {
            return Err("launch tilt must be within the 0 to 45 degree envelope".into());
        }
        for layer in &wind.layers {
            if !layer.altitude_m.is_finite()
                || layer
                    .velocity_ms
                    .iter()
                    .any(|component| !component.is_finite())
            {
                return Err("wind layers must contain only finite values".into());
            }
        }
        Ok(Self {
            vehicle,
            wind,
            launch,
        })
    }

    pub fn run_detailed(
        &self,
        rocket: &Rocket,
        motor: &Motor,
        env: &Environment,
        config: &SimConfig,
    ) -> Result<SixDofResult, String> {
        validate_run_inputs(rocket, env, config)?;

        let dt = config.dt_s;
        let burn_time = motor.burn_time();
        let rail_axis = launch_axis(&self.launch);
        let launch_attitude = launch_quaternion(&self.launch);
        let mut state = [0.0; STATE_LEN];
        state[ATTITUDE..ATTITUDE + 4].copy_from_slice(&launch_attitude);
        let mut t = 0.0;
        let mut phase = FlightPhase::Pad;
        let mut history = vec![sample(t, state, phase)];
        let mut events = Vec::new();
        let mut steps = 0_u64;
        let mut max_velocity = 0.0_f64;
        let mut rail_exit_time = None;
        let mut weathercock_pitch_deg = None;

        while t < config.max_time_s {
            if phase == FlightPhase::Pad {
                let mass_now = rocket.dry_mass_kg + motor.mass_at(t);
                let mass_next = rocket.dry_mass_kg + motor.mass_at(t + dt);
                let opposing_gravity = env.gravity_ms2 * rail_axis[2];
                let held = motor.thrust_at(t) <= mass_now * opposing_gravity
                    && motor.thrust_at(t + dt) <= mass_next * opposing_gravity;
                if held {
                    if t >= burn_time {
                        break;
                    }
                    t += dt;
                    steps += 1;
                    continue;
                }
                events.push(Event {
                    kind: EventKind::Liftoff,
                    t,
                    altitude_m: 0.0,
                    velocity_ms: 0.0,
                });
                phase = FlightPhase::Rail;
            }

            let mut next = rk4_step(
                t,
                state,
                dt,
                phase,
                rocket,
                motor,
                env,
                &self.vehicle,
                &self.wind,
                rail_axis,
            )?;
            normalize_state_quaternion(&mut next)?;

            if phase == FlightPhase::Rail
                && dot3(vector3(state, POSITION), rail_axis) <= 0.0
                && dot3(vector3(state, VELOCITY), rail_axis) <= 0.0
                && t < burn_time
            {
                constrain_rail_state(&mut next, rail_axis, launch_attitude, true);
            }

            let from_rail_distance = dot3(vector3(state, POSITION), rail_axis);
            let to_rail_distance = dot3(vector3(next, POSITION), rail_axis);
            if phase == FlightPhase::Rail && from_rail_distance > 0.0 && to_rail_distance <= 0.0 {
                let frac =
                    (from_rail_distance / (from_rail_distance - to_rail_distance)).clamp(0.0, 1.0);
                state = interpolate_state(state, next, frac)?;
                constrain_rail_state(&mut state, rail_axis, launch_attitude, true);
                state[POSITION..POSITION + 3].fill(0.0);
                state[VELOCITY..VELOCITY + 3].fill(0.0);
                t += frac * dt;
                phase = FlightPhase::Pad;
                steps += 1;
                history.push(sample(t, state, phase));
                continue;
            }
            if phase == FlightPhase::Rail
                && from_rail_distance < env.rail_length_m
                && to_rail_distance >= env.rail_length_m
            {
                let frac = ((env.rail_length_m - from_rail_distance)
                    / (to_rail_distance - from_rail_distance))
                    .clamp(0.0, 1.0);
                let event = interpolated_event(EventKind::RailExit, t, dt, state, next, frac);
                rail_exit_time = Some(event.t);
                events.push(event);
                phase = FlightPhase::Ascent;
            }

            if events.iter().all(|event| event.kind != EventKind::Burnout) && t + dt >= burn_time {
                let frac = ((burn_time - t) / dt).clamp(0.0, 1.0);
                events.push(interpolated_event(
                    EventKind::Burnout,
                    t,
                    dt,
                    state,
                    next,
                    frac,
                ));
            }

            if phase == FlightPhase::Ascent
                && state[VELOCITY + 2] > 0.0
                && next[VELOCITY + 2] <= 0.0
            {
                let frac = (state[VELOCITY + 2] / (state[VELOCITY + 2] - next[VELOCITY + 2]))
                    .clamp(0.0, 1.0);
                let apogee = interpolated_event(EventKind::Apogee, t, dt, state, next, frac);
                events.push(apogee);
                if rocket.recovery.is_some() {
                    events.push(Event {
                        kind: EventKind::RecoveryDeploy,
                        ..apogee
                    });
                }
                state = interpolate_state(state, next, frac)?;
                state[VELOCITY + 2] = 0.0;
                state[ANGULAR_RATE..ANGULAR_RATE + 3].fill(0.0);
                t = apogee.t;
                phase = FlightPhase::Descent;
                steps += 1;
                history.push(sample(t, state, phase));
                continue;
            }

            if phase == FlightPhase::Descent
                && state[POSITION + 2] > 0.0
                && next[POSITION + 2] <= 0.0
            {
                let frac = (state[POSITION + 2] / (state[POSITION + 2] - next[POSITION + 2]))
                    .clamp(0.0, 1.0);
                let landing = interpolated_event(EventKind::Landing, t, dt, state, next, frac);
                state = interpolate_state(state, next, frac)?;
                state[POSITION + 2] = 0.0;
                t = landing.t;
                events.push(landing);
                steps += 1;
                history.push(sample(t, state, FlightPhase::Grounded));
                break;
            }

            state = next;
            t += dt;
            steps += 1;
            max_velocity = max_velocity.max(norm3(vector3(state, VELOCITY)));
            history.push(sample(t, state, phase));

            if phase == FlightPhase::Ascent
                && weathercock_pitch_deg.is_none()
                && rail_exit_time.is_some_and(|exit| t >= exit + 0.5)
            {
                weathercock_pitch_deg = Some(pitch_from_quaternion(vector4(state, ATTITUDE)));
            }
        }

        let event = |kind| events.iter().find(|event| event.kind == kind).copied();
        let apogee = event(EventKind::Apogee);
        let burnout = event(EventKind::Burnout);
        let rail_exit = event(EventKind::RailExit);
        let landing = event(EventKind::Landing);
        let sim_result = SimResult {
            samples: Vec::new(),
            events,
            apogee_m: apogee.map_or(state[POSITION + 2], |event| event.altitude_m),
            apogee_time_s: apogee.map_or(t, |event| event.t),
            burnout_time_s: burnout.map_or(burn_time, |event| event.t),
            burnout_velocity_ms: burnout.map_or(0.0, |event| event.velocity_ms),
            burnout_altitude_m: burnout.map_or(0.0, |event| event.altitude_m),
            max_velocity_ms: max_velocity,
            rail_exit_velocity_ms: rail_exit.map_or(f64::NAN, |event| event.velocity_ms),
            landing_time_s: landing.map_or(f64::NAN, |event| event.t),
            landing_velocity_ms: landing.map_or(f64::NAN, |event| event.velocity_ms),
            steps,
        };
        let mut summary = SimSummary::from_result(&sim_result, rocket, motor, env, config);
        summary.input_hash = self.input_hash(rocket, motor, env, config);

        Ok(SixDofResult {
            summary,
            history,
            landing_position_m: vector3(state, POSITION),
            weathercock_pitch_deg: weathercock_pitch_deg.unwrap_or(0.0),
        })
    }

    fn input_hash(
        &self,
        rocket: &Rocket,
        motor: &Motor,
        env: &Environment,
        config: &SimConfig,
    ) -> String {
        let canonical = serde_json::json!({
            "rocket": rocket,
            "motor": motor,
            "environment": env,
            "config": config,
            "sixdof_vehicle": self.vehicle,
            "sixdof_wind": self.wind,
            "sixdof_launch": self.launch,
        });
        let bytes = serde_json::to_vec(&canonical).expect("six-DOF inputs serialize");
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }
}

impl SimEngine for SixDofEngine {
    fn id(&self) -> &'static str {
        "ascent-sixdof"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn run(
        &self,
        rocket: &Rocket,
        motor: &Motor,
        env: &Environment,
        config: &SimConfig,
    ) -> Result<SimSummary, String> {
        self.run_detailed(rocket, motor, env, config)
            .map(|result| result.summary)
    }
}

/// One burn phase of a staged 6-DOF flight, in burn order. Mirrors
/// `PlanarStage`: `dry_mass_kg` is what separates with this stage, and
/// `vehicle` describes the stack configuration this stage flies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SixDofStage {
    pub dry_mass_kg: f64,
    pub motor: Motor,
    /// Delay after this stage's burnout before separation; the next stage
    /// ignites at separation. Ignored for the final stage.
    pub separation_delay_s: f64,
    pub vehicle: SixDofVehicle,
    pub drag: Option<crate::rocket::DragModel>,
}

/// Staged 6-DOF flight with variable-mass handoff: same phase logic as
/// `run_detailed`, a stage-local motor clock, steps snapped to the exact
/// separation instant, and `StageSeparation`/`StageIgnition` in the event
/// timeline. Apogee → descent arms only on the final stage.
pub fn simulate_sixdof_staged(
    stages: &[SixDofStage],
    recovery: Option<crate::rocket::Recovery>,
    wind: &Wind3DProfile,
    launch: &SixDofLaunch,
    env: &Environment,
    config: &SimConfig,
) -> Result<SixDofResult, String> {
    if stages.is_empty() {
        return Err("at least one stage required".into());
    }
    let last = stages.len() - 1;
    let carried_above = |k: usize| -> f64 {
        stages[k + 1..]
            .iter()
            .map(|s| s.dry_mass_kg + s.motor.mass_at(0.0))
            .sum::<f64>()
    };
    let effective_rocket = |k: usize| -> Rocket {
        Rocket {
            name: String::new(),
            dry_mass_kg: stages[k].dry_mass_kg + carried_above(k),
            drag: stages[k].drag.clone(),
            recovery: recovery.clone(),
        }
    };

    // Per-stage engines re-run the input validation on each vehicle.
    let engines: Vec<SixDofEngine> = stages
        .iter()
        .map(|s| SixDofEngine::new(s.vehicle.clone(), wind.clone(), launch.clone()))
        .collect::<Result<_, _>>()?;

    let mut active = 0usize;
    let mut rocket = effective_rocket(0);
    validate_run_inputs(&rocket, env, config)?;

    let dt = config.dt_s;
    let rail_axis = launch_axis(launch);
    let launch_attitude = launch_quaternion(launch);
    let mut state = [0.0; STATE_LEN];
    state[ATTITUDE..ATTITUDE + 4].copy_from_slice(&launch_attitude);
    let mut t = 0.0;
    let mut tau = 0.0; // active-stage motor clock
    let mut burnout_emitted = false;
    let mut phase = FlightPhase::Pad;
    let mut history = vec![sample(t, state, phase)];
    let mut events: Vec<Event> = Vec::new();
    let mut steps = 0_u64;
    let mut max_velocity = 0.0_f64;
    let mut rail_exit_time = None;
    let mut weathercock_pitch_deg = None;

    while t < config.max_time_s {
        let motor = &stages[active].motor;
        let vehicle = &engines[active].vehicle;
        let burn_time = motor.burn_time();

        if phase == FlightPhase::Pad {
            let mass_now = rocket.dry_mass_kg + motor.mass_at(tau);
            let mass_next = rocket.dry_mass_kg + motor.mass_at(tau + dt);
            let opposing_gravity = env.gravity_ms2 * rail_axis[2];
            let held = motor.thrust_at(tau) <= mass_now * opposing_gravity
                && motor.thrust_at(tau + dt) <= mass_next * opposing_gravity;
            if held {
                if tau >= burn_time {
                    break;
                }
                t += dt;
                tau += dt;
                steps += 1;
                continue;
            }
            events.push(Event {
                kind: EventKind::Liftoff,
                t,
                altitude_m: 0.0,
                velocity_ms: 0.0,
            });
            phase = FlightPhase::Rail;
        }

        let sep_local = burn_time + stages[active].separation_delay_s;
        let step = if active < last && tau + dt > sep_local {
            sep_local - tau
        } else {
            dt
        };

        if step > 1e-12 {
            let mut next = rk4_step(
                tau, state, step, phase, &rocket, motor, env, vehicle, wind, rail_axis,
            )?;
            normalize_state_quaternion(&mut next)?;

            if phase == FlightPhase::Rail
                && dot3(vector3(state, POSITION), rail_axis) <= 0.0
                && dot3(vector3(state, VELOCITY), rail_axis) <= 0.0
                && tau < burn_time
            {
                constrain_rail_state(&mut next, rail_axis, launch_attitude, true);
            }

            let from_rail_distance = dot3(vector3(state, POSITION), rail_axis);
            let to_rail_distance = dot3(vector3(next, POSITION), rail_axis);
            if phase == FlightPhase::Rail && from_rail_distance > 0.0 && to_rail_distance <= 0.0 {
                let frac = (from_rail_distance / (from_rail_distance - to_rail_distance))
                    .clamp(0.0, 1.0);
                state = interpolate_state(state, next, frac)?;
                constrain_rail_state(&mut state, rail_axis, launch_attitude, true);
                state[POSITION..POSITION + 3].fill(0.0);
                state[VELOCITY..VELOCITY + 3].fill(0.0);
                t += frac * step;
                tau += frac * step;
                phase = FlightPhase::Pad;
                steps += 1;
                history.push(sample(t, state, phase));
                continue;
            }
            if phase == FlightPhase::Rail
                && from_rail_distance < env.rail_length_m
                && to_rail_distance >= env.rail_length_m
            {
                let frac = ((env.rail_length_m - from_rail_distance)
                    / (to_rail_distance - from_rail_distance))
                    .clamp(0.0, 1.0);
                let event = interpolated_event(EventKind::RailExit, t, step, state, next, frac);
                rail_exit_time = Some(event.t);
                events.push(event);
                phase = FlightPhase::Ascent;
            }

            if !burnout_emitted && tau + step >= burn_time - 1e-9 {
                let frac = if step > 0.0 {
                    ((burn_time - tau) / step).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                events.push(interpolated_event(
                    EventKind::Burnout,
                    t,
                    step,
                    state,
                    next,
                    frac,
                ));
                burnout_emitted = true;
            }

            if active == last
                && phase == FlightPhase::Ascent
                && state[VELOCITY + 2] > 0.0
                && next[VELOCITY + 2] <= 0.0
            {
                let frac = (state[VELOCITY + 2] / (state[VELOCITY + 2] - next[VELOCITY + 2]))
                    .clamp(0.0, 1.0);
                let apogee = interpolated_event(EventKind::Apogee, t, step, state, next, frac);
                events.push(apogee);
                if rocket.recovery.is_some() {
                    events.push(Event {
                        kind: EventKind::RecoveryDeploy,
                        ..apogee
                    });
                }
                state = interpolate_state(state, next, frac)?;
                state[VELOCITY + 2] = 0.0;
                state[ANGULAR_RATE..ANGULAR_RATE + 3].fill(0.0);
                tau += apogee.t - t;
                t = apogee.t;
                phase = FlightPhase::Descent;
                steps += 1;
                history.push(sample(t, state, phase));
                continue;
            }

            if phase == FlightPhase::Descent
                && state[POSITION + 2] > 0.0
                && next[POSITION + 2] <= 0.0
            {
                let frac = (state[POSITION + 2] / (state[POSITION + 2] - next[POSITION + 2]))
                    .clamp(0.0, 1.0);
                let landing = interpolated_event(EventKind::Landing, t, step, state, next, frac);
                state = interpolate_state(state, next, frac)?;
                state[POSITION + 2] = 0.0;
                t = landing.t;
                events.push(landing);
                steps += 1;
                history.push(sample(t, state, FlightPhase::Grounded));
                break;
            }

            state = next;
            t += step;
            tau += step;
            steps += 1;
            max_velocity = max_velocity.max(norm3(vector3(state, VELOCITY)));
            history.push(sample(t, state, phase));

            if phase == FlightPhase::Ascent
                && weathercock_pitch_deg.is_none()
                && rail_exit_time.is_some_and(|exit| t >= exit + 0.5)
            {
                weathercock_pitch_deg = Some(pitch_from_quaternion(vector4(state, ATTITUDE)));
            }
        }

        if active < last && tau >= sep_local - 1e-12 {
            let mark = |kind| Event {
                kind,
                t,
                altitude_m: state[POSITION + 2],
                velocity_ms: norm3(vector3(state, VELOCITY)),
            };
            if !burnout_emitted {
                events.push(mark(EventKind::Burnout));
            }
            events.push(mark(EventKind::StageSeparation));
            active += 1;
            rocket = effective_rocket(active);
            tau = 0.0;
            burnout_emitted = false;
            events.push(mark(EventKind::StageIgnition));
            if phase == FlightPhase::Rail {
                phase = FlightPhase::Ascent;
            }
        }
    }

    let event = |kind| events.iter().find(|event| event.kind == kind).copied();
    let apogee = event(EventKind::Apogee);
    let burnout = event(EventKind::Burnout);
    let rail_exit = event(EventKind::RailExit);
    let landing = event(EventKind::Landing);
    let final_motor = &stages[last].motor;
    let sim_result = SimResult {
        samples: Vec::new(),
        events,
        apogee_m: apogee.map_or(state[POSITION + 2], |event| event.altitude_m),
        apogee_time_s: apogee.map_or(t, |event| event.t),
        burnout_time_s: burnout.map_or(final_motor.burn_time(), |event| event.t),
        burnout_velocity_ms: burnout.map_or(0.0, |event| event.velocity_ms),
        burnout_altitude_m: burnout.map_or(0.0, |event| event.altitude_m),
        max_velocity_ms: max_velocity,
        rail_exit_velocity_ms: rail_exit.map_or(f64::NAN, |event| event.velocity_ms),
        landing_time_s: landing.map_or(f64::NAN, |event| event.t),
        landing_velocity_ms: landing.map_or(f64::NAN, |event| event.velocity_ms),
        steps,
    };
    let mut summary = SimSummary::from_result(&sim_result, &rocket, final_motor, env, config);
    // Hash the full staged input set, not just the final configuration.
    let canonical = serde_json::json!({
        "stages": stages,
        "recovery": recovery,
        "environment": env,
        "config": config,
        "wind": wind,
        "launch": launch,
    });
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&canonical).expect("staged six-DOF inputs serialize"));
    summary.input_hash = format!("{:x}", hasher.finalize());

    Ok(SixDofResult {
        summary,
        history,
        landing_position_m: vector3(state, POSITION),
        weathercock_pitch_deg: weathercock_pitch_deg.unwrap_or(0.0),
    })
}

#[allow(clippy::too_many_arguments)]
fn derivative(
    t: f64,
    state: State,
    phase: FlightPhase,
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    vehicle: &SixDofVehicle,
    wind: &Wind3DProfile,
    rail_axis: [f64; 3],
) -> Result<State, String> {
    let mut derivative = [0.0; STATE_LEN];
    let position = vector3(state, POSITION);
    let velocity = vector3(state, VELOCITY);
    let attitude = vector4(state, ATTITUDE);
    let angular_rate = vector3(state, ANGULAR_RATE);
    let mass = rocket.dry_mass_kg + motor.mass_at(t);
    let thrust = motor.thrust_at(t);
    let density = env.atmosphere.density_at(position[2].max(0.0));
    let body_cda = rocket
        .drag
        .as_ref()
        .map(|drag| drag.cd * drag.reference_area_m2)
        .unwrap_or(0.0);

    if phase == FlightPhase::Rail {
        let along_velocity = dot3(velocity, rail_axis);
        let along_air_velocity = along_velocity - dot3(wind.velocity_at(position[2]), rail_axis);
        let drag = 0.5 * density * body_cda * along_air_velocity.abs() * along_air_velocity;
        let along_acceleration = (thrust - drag) / mass - env.gravity_ms2 * rail_axis[2];
        derivative[POSITION..POSITION + 3].copy_from_slice(&scale3(rail_axis, along_velocity));
        derivative[VELOCITY..VELOCITY + 3].copy_from_slice(&scale3(rail_axis, along_acceleration));
        return Ok(derivative);
    }

    derivative[POSITION..POSITION + 3].copy_from_slice(&velocity);
    let relative_air = sub3(velocity, wind.velocity_at(position[2]));
    let airspeed = norm3(relative_air);
    let mut force_inertial = add3(
        scale3(rotate_body_to_inertial(attitude, [0.0, 0.0, 1.0])?, thrust),
        [0.0, 0.0, -mass * env.gravity_ms2],
    );
    let drag_cda = if phase == FlightPhase::Descent {
        body_cda
            + rocket
                .recovery
                .as_ref()
                .map(|recovery| recovery.chute_cd * recovery.chute_area_m2)
                .unwrap_or(0.0)
    } else {
        body_cda
    };
    if airspeed > 1e-9 {
        force_inertial = add3(
            force_inertial,
            scale3(relative_air, -0.5 * density * drag_cda * airspeed),
        );
    }

    if phase == FlightPhase::Ascent {
        let air_body = rotate_inertial_to_body(attitude, relative_air)?;
        let axial = air_body[2];
        if axial > 1e-3 {
            let lateral = [air_body[0], air_body[1]];
            let lateral_speed = (lateral[0] * lateral[0] + lateral[1] * lateral[1]).sqrt();
            let alpha = lateral_speed.atan2(axial);
            let dynamic_scale = 0.5
                * density
                * airspeed
                * airspeed
                * vehicle.reference_area_m2
                * vehicle.cn_alpha_per_rad;
            let normal_scale = if lateral_speed > 1e-12 {
                -dynamic_scale * alpha / lateral_speed
            } else {
                0.0
            };
            let normal_body = [normal_scale * lateral[0], normal_scale * lateral[1], 0.0];
            force_inertial = add3(
                force_inertial,
                rotate_body_to_inertial(attitude, normal_body)?,
            );
            let margin = vehicle.cp_from_nose_m - vehicle.cg_from_nose_m;
            derivative[ANGULAR_RATE] = margin * normal_body[1] / vehicle.pitch_yaw_inertia_kgm2;
            derivative[ANGULAR_RATE + 1] =
                -margin * normal_body[0] / vehicle.pitch_yaw_inertia_kgm2;
        }
        let q_dot = quaternion_derivative(attitude, angular_rate);
        derivative[ATTITUDE..ATTITUDE + 4].copy_from_slice(&q_dot);
    }

    derivative[VELOCITY..VELOCITY + 3].copy_from_slice(&scale3(force_inertial, 1.0 / mass));
    Ok(derivative)
}

#[allow(clippy::too_many_arguments)]
fn rk4_step(
    t: f64,
    state: State,
    dt: f64,
    phase: FlightPhase,
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    vehicle: &SixDofVehicle,
    wind: &Wind3DProfile,
    rail_axis: [f64; 3],
) -> Result<State, String> {
    let evaluate = |stage_t, stage_state| {
        derivative(
            stage_t,
            stage_state,
            phase,
            rocket,
            motor,
            env,
            vehicle,
            wind,
            rail_axis,
        )
    };
    let k1 = evaluate(t, state)?;
    let k2 = evaluate(t + dt / 2.0, add_scaled(state, k1, dt / 2.0))?;
    let k3 = evaluate(t + dt / 2.0, add_scaled(state, k2, dt / 2.0))?;
    let k4 = evaluate(t + dt, add_scaled(state, k3, dt))?;
    let mut next = state;
    for index in 0..STATE_LEN {
        next[index] += dt / 6.0 * (k1[index] + 2.0 * k2[index] + 2.0 * k3[index] + k4[index]);
    }
    next[ANGULAR_RATE + 2] = 0.0;
    Ok(next)
}

fn validate_run_inputs(
    rocket: &Rocket,
    env: &Environment,
    config: &SimConfig,
) -> Result<(), String> {
    if !config.dt_s.is_finite() || config.dt_s <= 0.0 {
        return Err("timestep must be finite and positive".into());
    }
    if !config.max_time_s.is_finite() || config.max_time_s <= 0.0 {
        return Err("maximum time must be finite and positive".into());
    }
    for (name, value) in [
        ("rocket dry mass", rocket.dry_mass_kg),
        ("gravity", env.gravity_ms2),
        ("rail length", env.rail_length_m),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(format!("{name} must be finite and positive"));
        }
    }
    if let Some(drag) = &rocket.drag {
        for (name, value) in [
            ("drag coefficient", drag.cd),
            ("drag area", drag.reference_area_m2),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!("{name} must be finite and non-negative"));
            }
        }
    }
    if let Some(recovery) = &rocket.recovery {
        for (name, value) in [
            ("recovery drag coefficient", recovery.chute_cd),
            ("recovery area", recovery.chute_area_m2),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!("{name} must be finite and non-negative"));
            }
        }
    }
    Ok(())
}

fn launch_axis(launch: &SixDofLaunch) -> [f64; 3] {
    let (sin_tilt, cos_tilt) = launch.tilt_rad.sin_cos();
    let (sin_azimuth, cos_azimuth) = launch.azimuth_rad.sin_cos();
    [sin_tilt * cos_azimuth, sin_tilt * sin_azimuth, cos_tilt]
}

fn launch_quaternion(launch: &SixDofLaunch) -> [f64; 4] {
    let half = launch.tilt_rad / 2.0;
    let (sin_half, cos_half) = half.sin_cos();
    let (sin_azimuth, cos_azimuth) = launch.azimuth_rad.sin_cos();
    [
        cos_half,
        -sin_azimuth * sin_half,
        cos_azimuth * sin_half,
        0.0,
    ]
}

fn sample(time_s: f64, state: State, phase: FlightPhase) -> SixDofSample {
    SixDofSample {
        time_s,
        position_m: vector3(state, POSITION),
        velocity_ms: vector3(state, VELOCITY),
        attitude_wxyz: vector4(state, ATTITUDE),
        angular_rate_rad_s: vector3(state, ANGULAR_RATE),
        phase,
    }
}

fn interpolated_event(
    kind: EventKind,
    t: f64,
    dt: f64,
    from: State,
    to: State,
    frac: f64,
) -> Event {
    Event {
        kind,
        t: t + frac * dt,
        altitude_m: from[POSITION + 2] + frac * (to[POSITION + 2] - from[POSITION + 2]),
        velocity_ms: from[VELOCITY + 2] + frac * (to[VELOCITY + 2] - from[VELOCITY + 2]),
    }
}

fn interpolate_state(from: State, to: State, frac: f64) -> Result<State, String> {
    let mut state = from;
    for index in 0..STATE_LEN {
        state[index] += frac * (to[index] - from[index]);
    }
    normalize_state_quaternion(&mut state)?;
    Ok(state)
}

fn constrain_rail_state(
    state: &mut State,
    axis: [f64; 3],
    attitude: [f64; 4],
    clamp_nonnegative: bool,
) {
    let distance = dot3(vector3(*state, POSITION), axis);
    let speed = dot3(vector3(*state, VELOCITY), axis);
    let distance = if clamp_nonnegative {
        distance.max(0.0)
    } else {
        distance
    };
    let speed = if clamp_nonnegative {
        speed.max(0.0)
    } else {
        speed
    };
    state[POSITION..POSITION + 3].copy_from_slice(&scale3(axis, distance));
    state[VELOCITY..VELOCITY + 3].copy_from_slice(&scale3(axis, speed));
    state[ATTITUDE..ATTITUDE + 4].copy_from_slice(&attitude);
    state[ANGULAR_RATE..ANGULAR_RATE + 3].fill(0.0);
}

fn normalize_state_quaternion(state: &mut State) -> Result<(), String> {
    let normalized = normalize_quaternion(vector4(*state, ATTITUDE))?;
    state[ATTITUDE..ATTITUDE + 4].copy_from_slice(&normalized);
    Ok(())
}

fn normalize_quaternion(q: [f64; 4]) -> Result<[f64; 4], String> {
    let norm = q
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= f64::EPSILON {
        return Err("attitude quaternion has zero or non-finite norm".into());
    }
    Ok([q[0] / norm, q[1] / norm, q[2] / norm, q[3] / norm])
}

fn quaternion_derivative(q: [f64; 4], omega: [f64; 3]) -> [f64; 4] {
    let product = quaternion_multiply(q, [0.0, omega[0], omega[1], omega[2]]);
    [
        0.5 * product[0],
        0.5 * product[1],
        0.5 * product[2],
        0.5 * product[3],
    ]
}

fn quaternion_multiply(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

fn rotate_body_to_inertial(q: [f64; 4], vector: [f64; 3]) -> Result<[f64; 3], String> {
    let q = normalize_quaternion(q)?;
    let rotated = quaternion_multiply(
        quaternion_multiply(q, [0.0, vector[0], vector[1], vector[2]]),
        [q[0], -q[1], -q[2], -q[3]],
    );
    Ok([rotated[1], rotated[2], rotated[3]])
}

fn rotate_inertial_to_body(q: [f64; 4], vector: [f64; 3]) -> Result<[f64; 3], String> {
    let q = normalize_quaternion(q)?;
    let conjugate = [q[0], -q[1], -q[2], -q[3]];
    let rotated = quaternion_multiply(
        quaternion_multiply(conjugate, [0.0, vector[0], vector[1], vector[2]]),
        q,
    );
    Ok([rotated[1], rotated[2], rotated[3]])
}

fn pitch_from_quaternion(q: [f64; 4]) -> f64 {
    rotate_body_to_inertial(q, [0.0, 0.0, 1.0])
        .map(|axis| axis[0].atan2(axis[2]).to_degrees())
        .unwrap_or(f64::NAN)
}

fn add_scaled(state: State, derivative: State, scale: f64) -> State {
    let mut result = state;
    for index in 0..STATE_LEN {
        result[index] += scale * derivative[index];
    }
    result
}

fn vector3(state: State, offset: usize) -> [f64; 3] {
    [state[offset], state[offset + 1], state[offset + 2]]
}

fn vector4(state: State, offset: usize) -> [f64; 4] {
    [
        state[offset],
        state[offset + 1],
        state[offset + 2],
        state[offset + 3],
    ]
}

fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale3(vector: [f64; 3], scale: f64) -> [f64; 3] {
    [vector[0] * scale, vector[1] * scale, vector[2] * scale]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm3(vector: [f64; 3]) -> f64 {
    dot3(vector, vector).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtmosphereModel;

    #[test]
    fn barrowman_normal_force_is_linear_in_angle_of_attack() {
        let motor = Motor::from_json(include_str!(
            "../../ascent-domain/data/motors/estes_c6.json"
        ))
        .unwrap();
        let rocket = Rocket {
            name: "force-law fixture".into(),
            dry_mass_kg: 1.0,
            drag: None,
            recovery: None,
        };
        let env = Environment {
            gravity_ms2: 0.0,
            atmosphere: AtmosphereModel::ConstantDensity(1.0),
            rail_length_m: 1.0,
        };
        let vehicle = SixDofVehicle {
            cg_from_nose_m: 0.1,
            pitch_yaw_inertia_kgm2: 1.0,
            cp_from_nose_m: 0.1,
            cn_alpha_per_rad: 1.0,
            reference_area_m2: 1.0,
        };
        let mut state = [0.0; STATE_LEN];
        state[VELOCITY..VELOCITY + 3].copy_from_slice(&[6.0, 8.0, 10.0]);
        state[ATTITUDE] = 1.0;

        let rate = derivative(
            motor.burn_time(),
            state,
            FlightPhase::Ascent,
            &rocket,
            &motor,
            &env,
            &vehicle,
            &Wind3DProfile::calm(),
            [0.0, 0.0, 1.0],
        )
        .unwrap();
        let mass = rocket.dry_mass_kg + motor.mass_at(motor.burn_time());
        let actual_normal_force = [
            rate[VELOCITY] * mass,
            rate[VELOCITY + 1] * mass,
            rate[VELOCITY + 2] * mass,
        ];
        let magnitude = -100.0 * std::f64::consts::FRAC_PI_4;
        let expected_normal_force = [magnitude * 0.6, magnitude * 0.8, 0.0];

        for axis in 0..3 {
            assert!(
                (actual_normal_force[axis] - expected_normal_force[axis]).abs() <= 1e-12,
                "normal force {:?} N must equal 3D linear-alpha force {:?} N",
                actual_normal_force,
                expected_normal_force,
            );
        }
    }
}
