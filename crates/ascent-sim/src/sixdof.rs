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
                if let Some(recovery) = &rocket.recovery {
                    events.push(Event {
                        kind: EventKind::RecoveryDeploy,
                        ..apogee
                    });
                    if recovery
                        .main_deploy_altitude_m
                        .is_some_and(|deploy_m| apogee.altitude_m <= deploy_m)
                    {
                        events.push(Event {
                            kind: EventKind::MainDeploy,
                            ..apogee
                        });
                    }
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

            // Dual-deploy main: descending through the configured altitude
            // restarts the step from the crossing (same discipline as
            // apogee), so the main's drag applies from exactly there.
            if phase == FlightPhase::Descent
                && events
                    .iter()
                    .all(|event| event.kind != EventKind::MainDeploy)
            {
                if let Some(deploy_m) = rocket
                    .recovery
                    .as_ref()
                    .and_then(|recovery| recovery.main_deploy_altitude_m)
                {
                    if state[POSITION + 2] > deploy_m && next[POSITION + 2] <= deploy_m {
                        let frac = ((state[POSITION + 2] - deploy_m)
                            / (state[POSITION + 2] - next[POSITION + 2]))
                            .clamp(0.0, 1.0);
                        let deploy =
                            interpolated_event(EventKind::MainDeploy, t, dt, state, next, frac);
                        events.push(deploy);
                        state = interpolate_state(state, next, frac)?;
                        state[POSITION + 2] = deploy_m;
                        t = deploy.t;
                        steps += 1;
                        history.push(sample(t, state, phase));
                        continue;
                    }
                }
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

/// Outcome of one [`StagedStepper::step`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepStatus {
    /// The integrator advanced; the flight is still in progress.
    Running,
    /// The integrator reached a terminal condition (landing, the time cap, or
    /// a motor that never lifts). No further stepping changes the result.
    Finished,
}

/// Effective mass carried above stage `k` (everything not yet separated).
fn carried_above(stages: &[SixDofStage], k: usize) -> f64 {
    stages[k + 1..]
        .iter()
        .map(|s| s.dry_mass_kg + s.motor.mass_at(0.0))
        .sum::<f64>()
}

/// The point-mass `Rocket` flown while stage `k` burns (its dry mass plus all
/// mass carried above it).
fn effective_rocket(
    stages: &[SixDofStage],
    recovery: &Option<crate::rocket::Recovery>,
    k: usize,
) -> Rocket {
    Rocket {
        name: String::new(),
        dry_mass_kg: stages[k].dry_mass_kg + carried_above(stages, k),
        drag: stages[k].drag.clone(),
        recovery: recovery.clone(),
    }
}

/// Re-entrant driver for a staged 6-DOF flight. This owns the exact
/// integration state of the canonical `simulate_sixdof_staged` loop; one
/// [`StagedStepper::step`] call executes one iteration of that loop, so a
/// batch run and a stepped run share a single integrator and identical
/// numerical ordering. `simulate_sixdof_staged` drives it to completion;
/// `RunSession` drives it in fixed-count chunks.
pub(crate) struct StagedStepper {
    // Immutable inputs, owned so the stepper is self-contained across calls.
    stages: Vec<SixDofStage>,
    recovery: Option<crate::rocket::Recovery>,
    wind: Wind3DProfile,
    launch: SixDofLaunch,
    env: Environment,
    config: SimConfig,
    // Immutable derived setup.
    engines: Vec<SixDofEngine>,
    last: usize,
    dt: f64,
    rail_axis: [f64; 3],
    launch_attitude: [f64; 4],
    // Mutable integration state (mirrors the canonical loop's locals).
    active: usize,
    rocket: Rocket,
    state: State,
    t: f64,
    tau: f64, // active-stage motor clock
    burnout_emitted: bool,
    phase: FlightPhase,
    history: Vec<SixDofSample>,
    events: Vec<Event>,
    steps: u64,
    max_velocity: f64,
    rail_exit_time: Option<f64>,
    weathercock_pitch_deg: Option<f64>,
    done: bool,
}

impl StagedStepper {
    /// Validate inputs and prime the integration state (identical setup to the
    /// canonical loop, including the initial history sample at t = 0).
    pub(crate) fn new(
        stages: &[SixDofStage],
        recovery: Option<crate::rocket::Recovery>,
        wind: &Wind3DProfile,
        launch: &SixDofLaunch,
        env: &Environment,
        config: &SimConfig,
    ) -> Result<Self, String> {
        if stages.is_empty() {
            return Err("at least one stage required".into());
        }
        let last = stages.len() - 1;

        // Per-stage engines re-run the input validation on each vehicle.
        let engines: Vec<SixDofEngine> = stages
            .iter()
            .map(|s| SixDofEngine::new(s.vehicle.clone(), wind.clone(), launch.clone()))
            .collect::<Result<_, _>>()?;

        let stages = stages.to_vec();
        let rocket = effective_rocket(&stages, &recovery, 0);
        validate_run_inputs(&rocket, env, config)?;

        let dt = config.dt_s;
        let rail_axis = launch_axis(launch);
        let launch_attitude = launch_quaternion(launch);
        let mut state = [0.0; STATE_LEN];
        state[ATTITUDE..ATTITUDE + 4].copy_from_slice(&launch_attitude);
        let phase = FlightPhase::Pad;
        let history = vec![sample(0.0, state, phase)];

        Ok(Self {
            stages,
            recovery,
            wind: wind.clone(),
            launch: launch.clone(),
            env: env.clone(),
            config: config.clone(),
            engines,
            last,
            dt,
            rail_axis,
            launch_attitude,
            active: 0,
            rocket,
            state,
            t: 0.0,
            tau: 0.0,
            burnout_emitted: false,
            phase,
            history,
            events: Vec::new(),
            steps: 0,
            max_velocity: 0.0,
            rail_exit_time: None,
            weathercock_pitch_deg: None,
            done: false,
        })
    }

    /// Execute exactly one iteration of the canonical staged loop. `Running`
    /// mirrors the loop reaching the next iteration (whether by a full step, a
    /// snapped event crossing, or a held-on-pad tick); `Finished` mirrors the
    /// loop's `break`/guard exits. The body is a line-for-line move of the
    /// canonical loop, with `continue` -> `Ok(Running)` and `break` ->
    /// `self.done = true; Ok(Finished)`.
    pub(crate) fn step(&mut self) -> Result<StepStatus, String> {
        if self.done {
            return Ok(StepStatus::Finished);
        }
        if self.t >= self.config.max_time_s {
            self.done = true;
            return Ok(StepStatus::Finished);
        }

        let dt = self.dt;
        let burn_time = self.stages[self.active].motor.burn_time();

        if self.phase == FlightPhase::Pad {
            let motor = &self.stages[self.active].motor;
            let mass_now = self.rocket.dry_mass_kg + motor.mass_at(self.tau);
            let mass_next = self.rocket.dry_mass_kg + motor.mass_at(self.tau + dt);
            let opposing_gravity = self.env.gravity_ms2 * self.rail_axis[2];
            let held = motor.thrust_at(self.tau) <= mass_now * opposing_gravity
                && motor.thrust_at(self.tau + dt) <= mass_next * opposing_gravity;
            if held {
                if self.tau >= burn_time {
                    self.done = true;
                    return Ok(StepStatus::Finished);
                }
                self.t += dt;
                self.tau += dt;
                self.steps += 1;
                return Ok(StepStatus::Running);
            }
            self.events.push(Event {
                kind: EventKind::Liftoff,
                t: self.t,
                altitude_m: 0.0,
                velocity_ms: 0.0,
            });
            self.phase = FlightPhase::Rail;
        }

        let sep_local = burn_time + self.stages[self.active].separation_delay_s;
        let step = if self.active < self.last && self.tau + dt > sep_local {
            sep_local - self.tau
        } else {
            dt
        };

        if step > 1e-12 {
            let mut next = {
                let motor = &self.stages[self.active].motor;
                let vehicle = &self.engines[self.active].vehicle;
                rk4_step(
                    self.tau,
                    self.state,
                    step,
                    self.phase,
                    &self.rocket,
                    motor,
                    &self.env,
                    vehicle,
                    &self.wind,
                    self.rail_axis,
                )?
            };
            normalize_state_quaternion(&mut next)?;

            if self.phase == FlightPhase::Rail
                && dot3(vector3(self.state, POSITION), self.rail_axis) <= 0.0
                && dot3(vector3(self.state, VELOCITY), self.rail_axis) <= 0.0
                && self.tau < burn_time
            {
                constrain_rail_state(&mut next, self.rail_axis, self.launch_attitude, true);
            }

            let from_rail_distance = dot3(vector3(self.state, POSITION), self.rail_axis);
            let to_rail_distance = dot3(vector3(next, POSITION), self.rail_axis);
            if self.phase == FlightPhase::Rail
                && from_rail_distance > 0.0
                && to_rail_distance <= 0.0
            {
                let frac =
                    (from_rail_distance / (from_rail_distance - to_rail_distance)).clamp(0.0, 1.0);
                self.state = interpolate_state(self.state, next, frac)?;
                constrain_rail_state(&mut self.state, self.rail_axis, self.launch_attitude, true);
                self.state[POSITION..POSITION + 3].fill(0.0);
                self.state[VELOCITY..VELOCITY + 3].fill(0.0);
                self.t += frac * step;
                self.tau += frac * step;
                self.phase = FlightPhase::Pad;
                self.steps += 1;
                self.history.push(sample(self.t, self.state, self.phase));
                return Ok(StepStatus::Running);
            }
            if self.phase == FlightPhase::Rail
                && from_rail_distance < self.env.rail_length_m
                && to_rail_distance >= self.env.rail_length_m
            {
                let frac = ((self.env.rail_length_m - from_rail_distance)
                    / (to_rail_distance - from_rail_distance))
                    .clamp(0.0, 1.0);
                let event =
                    interpolated_event(EventKind::RailExit, self.t, step, self.state, next, frac);
                self.rail_exit_time = Some(event.t);
                self.events.push(event);
                self.phase = FlightPhase::Ascent;
            }

            if !self.burnout_emitted && self.tau + step >= burn_time - 1e-9 {
                let frac = if step > 0.0 {
                    ((burn_time - self.tau) / step).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                self.events.push(interpolated_event(
                    EventKind::Burnout,
                    self.t,
                    step,
                    self.state,
                    next,
                    frac,
                ));
                self.burnout_emitted = true;
            }

            if self.active == self.last
                && self.phase == FlightPhase::Ascent
                && self.state[VELOCITY + 2] > 0.0
                && next[VELOCITY + 2] <= 0.0
            {
                let frac = (self.state[VELOCITY + 2]
                    / (self.state[VELOCITY + 2] - next[VELOCITY + 2]))
                    .clamp(0.0, 1.0);
                let apogee =
                    interpolated_event(EventKind::Apogee, self.t, step, self.state, next, frac);
                self.events.push(apogee);
                if let Some(recovery) = &self.rocket.recovery {
                    self.events.push(Event {
                        kind: EventKind::RecoveryDeploy,
                        ..apogee
                    });
                    if recovery
                        .main_deploy_altitude_m
                        .is_some_and(|deploy_m| apogee.altitude_m <= deploy_m)
                    {
                        self.events.push(Event {
                            kind: EventKind::MainDeploy,
                            ..apogee
                        });
                    }
                }
                self.state = interpolate_state(self.state, next, frac)?;
                self.state[VELOCITY + 2] = 0.0;
                self.state[ANGULAR_RATE..ANGULAR_RATE + 3].fill(0.0);
                self.tau += apogee.t - self.t;
                self.t = apogee.t;
                self.phase = FlightPhase::Descent;
                self.steps += 1;
                self.history.push(sample(self.t, self.state, self.phase));
                return Ok(StepStatus::Running);
            }

            // Dual-deploy main: step restarts from the crossing so the
            // main's drag applies from exactly the deploy altitude.
            if self.phase == FlightPhase::Descent
                && self
                    .events
                    .iter()
                    .all(|event| event.kind != EventKind::MainDeploy)
            {
                if let Some(deploy_m) = self
                    .rocket
                    .recovery
                    .as_ref()
                    .and_then(|recovery| recovery.main_deploy_altitude_m)
                {
                    if self.state[POSITION + 2] > deploy_m && next[POSITION + 2] <= deploy_m {
                        let frac = ((self.state[POSITION + 2] - deploy_m)
                            / (self.state[POSITION + 2] - next[POSITION + 2]))
                            .clamp(0.0, 1.0);
                        let deploy = interpolated_event(
                            EventKind::MainDeploy,
                            self.t,
                            step,
                            self.state,
                            next,
                            frac,
                        );
                        self.events.push(deploy);
                        self.state = interpolate_state(self.state, next, frac)?;
                        self.state[POSITION + 2] = deploy_m;
                        self.tau += deploy.t - self.t;
                        self.t = deploy.t;
                        self.steps += 1;
                        self.history.push(sample(self.t, self.state, self.phase));
                        return Ok(StepStatus::Running);
                    }
                }
            }

            if self.phase == FlightPhase::Descent
                && self.state[POSITION + 2] > 0.0
                && next[POSITION + 2] <= 0.0
            {
                let frac = (self.state[POSITION + 2]
                    / (self.state[POSITION + 2] - next[POSITION + 2]))
                    .clamp(0.0, 1.0);
                let landing =
                    interpolated_event(EventKind::Landing, self.t, step, self.state, next, frac);
                self.state = interpolate_state(self.state, next, frac)?;
                self.state[POSITION + 2] = 0.0;
                self.t = landing.t;
                self.events.push(landing);
                self.steps += 1;
                self.history
                    .push(sample(self.t, self.state, FlightPhase::Grounded));
                self.done = true;
                return Ok(StepStatus::Finished);
            }

            self.state = next;
            self.t += step;
            self.tau += step;
            self.steps += 1;
            self.max_velocity = self.max_velocity.max(norm3(vector3(self.state, VELOCITY)));
            self.history.push(sample(self.t, self.state, self.phase));

            if self.phase == FlightPhase::Ascent
                && self.weathercock_pitch_deg.is_none()
                && self.rail_exit_time.is_some_and(|exit| self.t >= exit + 0.5)
            {
                self.weathercock_pitch_deg =
                    Some(pitch_from_quaternion(vector4(self.state, ATTITUDE)));
            }
        }

        if self.active < self.last && self.tau >= sep_local - 1e-12 {
            let t = self.t;
            let altitude_m = self.state[POSITION + 2];
            let velocity_ms = norm3(vector3(self.state, VELOCITY));
            let mark = |kind| Event {
                kind,
                t,
                altitude_m,
                velocity_ms,
            };
            if !self.burnout_emitted {
                self.events.push(mark(EventKind::Burnout));
            }
            self.events.push(mark(EventKind::StageSeparation));
            self.active += 1;
            self.rocket = effective_rocket(&self.stages, &self.recovery, self.active);
            self.tau = 0.0;
            self.burnout_emitted = false;
            self.events.push(mark(EventKind::StageIgnition));
            if self.phase == FlightPhase::Rail {
                self.phase = FlightPhase::Ascent;
            }
        }

        Ok(StepStatus::Running)
    }

    /// Number of core steps executed so far (mirrors the canonical loop's
    /// `steps` counter).
    pub(crate) fn step_cursor(&self) -> u64 {
        self.steps
    }

    /// Index of the currently burning/active stage in burn order.
    pub(crate) fn stage_index(&self) -> usize {
        self.active
    }

    /// Instantaneous propulsion magnitude (newtons) of the active stage at the
    /// current motor clock. Zero between burns — the authoritative burn model
    /// owns this, so a client must not re-derive it.
    pub(crate) fn thrust_n(&self) -> f64 {
        self.stages[self.active].motor.thrust_at(self.tau)
    }

    /// The most recent recorded flight sample (seeded at t = 0, so always
    /// present).
    pub(crate) fn latest_sample(&self) -> &SixDofSample {
        self.history.last().expect("history is seeded at t = 0")
    }

    /// The ordered flight events derived so far.
    pub(crate) fn events(&self) -> &[Event] {
        &self.events
    }

    /// Assemble the result from the current state. Identical to the canonical
    /// loop's tail; safe to call at any point (before completion it yields the
    /// partial history assembled so far).
    pub(crate) fn finalize(&self) -> SixDofResult {
        let event = |kind| self.events.iter().find(|event| event.kind == kind).copied();
        let apogee = event(EventKind::Apogee);
        let burnout = event(EventKind::Burnout);
        let rail_exit = event(EventKind::RailExit);
        let landing = event(EventKind::Landing);
        let final_motor = &self.stages[self.last].motor;
        let sim_result = SimResult {
            samples: Vec::new(),
            events: self.events.clone(),
            apogee_m: apogee.map_or(self.state[POSITION + 2], |event| event.altitude_m),
            apogee_time_s: apogee.map_or(self.t, |event| event.t),
            burnout_time_s: burnout.map_or(final_motor.burn_time(), |event| event.t),
            burnout_velocity_ms: burnout.map_or(0.0, |event| event.velocity_ms),
            burnout_altitude_m: burnout.map_or(0.0, |event| event.altitude_m),
            max_velocity_ms: self.max_velocity,
            rail_exit_velocity_ms: rail_exit.map_or(f64::NAN, |event| event.velocity_ms),
            landing_time_s: landing.map_or(f64::NAN, |event| event.t),
            landing_velocity_ms: landing.map_or(f64::NAN, |event| event.velocity_ms),
            steps: self.steps,
        };
        let mut summary = SimSummary::from_result(
            &sim_result,
            &self.rocket,
            final_motor,
            &self.env,
            &self.config,
        );
        // Hash the full staged input set, not just the final configuration.
        let canonical = serde_json::json!({
            "stages": self.stages,
            "recovery": self.recovery,
            "environment": self.env,
            "config": self.config,
            "wind": self.wind,
            "launch": self.launch,
        });
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_vec(&canonical).expect("staged six-DOF inputs serialize"));
        summary.input_hash = format!("{:x}", hasher.finalize());

        SixDofResult {
            summary,
            history: self.history.clone(),
            landing_position_m: vector3(self.state, POSITION),
            weathercock_pitch_deg: self.weathercock_pitch_deg.unwrap_or(0.0),
        }
    }
}

/// Staged 6-DOF flight with variable-mass handoff: same phase logic as
/// `run_detailed`, a stage-local motor clock, steps snapped to the exact
/// separation instant, and `StageSeparation`/`StageIgnition` in the event
/// timeline. Apogee → descent arms only on the final stage.
///
/// This is the canonical batch entry point: it drives [`StagedStepper`] — the
/// same re-entrant integrator that [`crate::run_session::RunSession`] steps —
/// to completion, so batch and stepped runs are bit-for-bit identical.
pub fn simulate_sixdof_staged(
    stages: &[SixDofStage],
    recovery: Option<crate::rocket::Recovery>,
    wind: &Wind3DProfile,
    launch: &SixDofLaunch,
    env: &Environment,
    config: &SimConfig,
) -> Result<SixDofResult, String> {
    let mut stepper = StagedStepper::new(stages, recovery, wind, launch, env, config)?;
    while matches!(stepper.step()?, StepStatus::Running) {}
    Ok(stepper.finalize())
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
                .map(|recovery| recovery.descent_cda(position[2].max(0.0)))
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
        let mut checks = vec![
            ("recovery drag coefficient", recovery.chute_cd),
            ("recovery area", recovery.chute_area_m2),
        ];
        if let Some(drogue) = &recovery.drogue {
            checks.push(("drogue drag coefficient", drogue.cd));
            checks.push(("drogue area", drogue.area_m2));
        }
        if let Some(deploy_m) = recovery.main_deploy_altitude_m {
            checks.push(("main deploy altitude", deploy_m));
        }
        for (name, value) in checks {
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
