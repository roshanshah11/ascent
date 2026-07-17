use ascent_domain::Motor;
use serde::{Deserialize, Serialize};

use crate::events::{Event, EventKind};
use crate::rocket::{Environment, Rocket};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    /// Fixed RK4 timestep, seconds.
    pub dt_s: f64,
    /// Hard cap on simulated time, seconds.
    pub max_time_s: f64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            dt_s: 0.005,
            max_time_s: 600.0,
        }
    }
}

/// One recorded trajectory sample.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sample {
    pub t: f64,
    pub altitude_m: f64,
    pub velocity_ms: f64,
    pub mass_kg: f64,
    pub thrust_n: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimResult {
    pub samples: Vec<Sample>,
    pub events: Vec<Event>,
    pub apogee_m: f64,
    pub apogee_time_s: f64,
    pub burnout_time_s: f64,
    pub burnout_velocity_ms: f64,
    pub burnout_altitude_m: f64,
    pub max_velocity_ms: f64,
    pub rail_exit_velocity_ms: f64,
    pub landing_time_s: f64,
    pub landing_velocity_ms: f64,
    pub steps: u64,
}

impl SimResult {
    pub fn event(&self, kind: EventKind) -> Option<&Event> {
        self.events.iter().find(|e| e.kind == kind)
    }
}

/// State vector for vertical flight: (altitude m, velocity m/s).
type State = (f64, f64);

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Pad,
    Ascent,
    Descent,
}

fn derivative(
    t: f64,
    state: State,
    phase: Phase,
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
) -> State {
    let (h, v) = state;
    let mass = rocket.dry_mass_kg + motor.mass_at(t);
    let thrust = motor.thrust_at(t);
    let rho = env.atmosphere.density_at(h.max(0.0));
    let mut drag_cda = match &rocket.drag {
        Some(d) => d.cd * d.reference_area_m2,
        None => 0.0,
    };
    if phase == Phase::Descent {
        if let Some(r) = &rocket.recovery {
            drag_cda += r.chute_cd * r.chute_area_m2;
        }
    }
    let drag = 0.5 * rho * drag_cda * v * v * v.signum();
    let accel = (thrust - drag) / mass - env.gravity_ms2;
    (v, accel)
}

fn rk4_step(
    t: f64,
    state: State,
    dt: f64,
    phase: Phase,
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
) -> State {
    let f = |t: f64, s: State| derivative(t, s, phase, rocket, motor, env);
    let k1 = f(t, state);
    let k2 = f(
        t + dt / 2.0,
        (state.0 + dt / 2.0 * k1.0, state.1 + dt / 2.0 * k1.1),
    );
    let k3 = f(
        t + dt / 2.0,
        (state.0 + dt / 2.0 * k2.0, state.1 + dt / 2.0 * k2.1),
    );
    let k4 = f(t + dt, (state.0 + dt * k3.0, state.1 + dt * k3.1));
    (
        state.0 + dt / 6.0 * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0),
        state.1 + dt / 6.0 * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1),
    )
}

fn lerp_event(kind: EventKind, t: f64, dt: f64, from: State, to: State, frac: f64) -> Event {
    let frac = frac.clamp(0.0, 1.0);
    Event {
        kind,
        t: t + frac * dt,
        altitude_m: from.0 + frac * (to.0 - from.0),
        velocity_ms: from.1 + frac * (to.1 - from.1),
    }
}

/// Simulate a full vertical flight — pad, ascent, apogee, recovery descent,
/// landing — with fixed-step RK4 and interpolated event crossings.
pub fn simulate_vertical(
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    config: &SimConfig,
) -> SimResult {
    let dt = config.dt_s;
    let burn_time = motor.burn_time();
    let mut t = 0.0;
    let mut state: State = (0.0, 0.0);
    let mut phase = Phase::Pad;
    let mut samples = Vec::new();
    let mut events: Vec<Event> = Vec::new();
    let mut steps: u64 = 0;
    let mut max_velocity: f64 = 0.0;

    let record = |t: f64, s: State| Sample {
        t,
        altitude_m: s.0,
        velocity_ms: s.1,
        mass_kg: rocket.dry_mass_kg + motor.mass_at(t),
        thrust_n: motor.thrust_at(t),
    };
    samples.push(record(t, state));

    while t < config.max_time_s {
        if phase == Phase::Pad {
            let weight_now = (rocket.dry_mass_kg + motor.mass_at(t)) * env.gravity_ms2;
            let weight_next = (rocket.dry_mass_kg + motor.mass_at(t + dt)) * env.gravity_ms2;
            let held = motor.thrust_at(t) <= weight_now && motor.thrust_at(t + dt) <= weight_next;
            if held {
                if t >= burn_time {
                    // Motor never lifts this rocket.
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
            phase = Phase::Ascent;
        }

        let mut next = rk4_step(t, state, dt, phase, rocket, motor, env);
        // The pad cannot pull the rocket below ground during liftoff.
        if state.0 <= 0.0 && state.1 <= 0.0 && phase == Phase::Ascent && t < burn_time {
            next.0 = next.0.max(0.0);
            next.1 = next.1.max(0.0);
        }

        // Rail exit: altitude crosses the rail length on the way up.
        if phase == Phase::Ascent
            && events.iter().all(|e| e.kind != EventKind::RailExit)
            && state.0 < env.rail_length_m
            && next.0 >= env.rail_length_m
        {
            let frac = (env.rail_length_m - state.0) / (next.0 - state.0);
            events.push(lerp_event(EventKind::RailExit, t, dt, state, next, frac));
        }

        // Burnout: the burn-time boundary.
        if events.iter().all(|e| e.kind != EventKind::Burnout) && t + dt >= burn_time {
            let frac = (burn_time - t) / dt;
            events.push(lerp_event(EventKind::Burnout, t, dt, state, next, frac));
        }

        // Apogee: velocity zero-crossing from above.
        if phase == Phase::Ascent && state.1 > 0.0 && next.1 <= 0.0 {
            let frac = state.1 / (state.1 - next.1);
            let apogee = lerp_event(EventKind::Apogee, t, dt, state, next, frac);
            events.push(apogee);
            if rocket.recovery.is_some() {
                events.push(Event {
                    kind: EventKind::RecoveryDeploy,
                    ..apogee
                });
            }
            phase = Phase::Descent;
            // Restart the step from apogee so descent drag applies from the
            // true apogee point, not the step boundary.
            state = (apogee.altitude_m, 0.0);
            t = apogee.t;
            steps += 1;
            samples.push(record(t, state));
            continue;
        }

        // Landing: altitude crosses zero on the way down.
        if phase == Phase::Descent && state.0 > 0.0 && next.0 <= 0.0 {
            let frac = state.0 / (state.0 - next.0);
            let landing = lerp_event(EventKind::Landing, t, dt, state, next, frac);
            events.push(landing);
            samples.push(record(landing.t, (0.0, landing.velocity_ms)));
            steps += 1;
            break;
        }

        state = next;
        t += dt;
        steps += 1;
        max_velocity = max_velocity.max(state.1);
        samples.push(record(t, state));
    }

    let get = |kind: EventKind| events.iter().find(|e| e.kind == kind).copied();
    let apogee = get(EventKind::Apogee);
    let burnout = get(EventKind::Burnout);
    let rail_exit = get(EventKind::RailExit);
    let landing = get(EventKind::Landing);

    SimResult {
        apogee_m: apogee.map(|e| e.altitude_m).unwrap_or(state.0),
        apogee_time_s: apogee.map(|e| e.t).unwrap_or(t),
        burnout_time_s: burnout.map(|e| e.t).unwrap_or(burn_time),
        burnout_velocity_ms: burnout.map(|e| e.velocity_ms).unwrap_or(0.0),
        burnout_altitude_m: burnout.map(|e| e.altitude_m).unwrap_or(0.0),
        max_velocity_ms: max_velocity,
        rail_exit_velocity_ms: rail_exit.map(|e| e.velocity_ms).unwrap_or(f64::NAN),
        landing_time_s: landing.map(|e| e.t).unwrap_or(f64::NAN),
        landing_velocity_ms: landing.map(|e| e.velocity_ms).unwrap_or(f64::NAN),
        samples,
        events,
        steps,
    }
}

/// Timestep-convergence report: the same flight at dt, dt/2, dt/4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceReport {
    pub dt_s: f64,
    pub apogee_m: [f64; 3],
    /// |apogee(dt) − apogee(dt/2)| — the exit-gate number.
    pub apogee_delta_m: f64,
    /// |apogee(dt/2) − apogee(dt/4)| — must shrink relative to the above.
    pub apogee_delta_fine_m: f64,
    pub converged: bool,
}

/// A run "converges" when halving the step moves apogee by less than
/// max(0.5 m, 0.1% of apogee).
pub fn convergence_report(
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    config: &SimConfig,
) -> ConvergenceReport {
    let run = |dt: f64| {
        simulate_vertical(
            rocket,
            motor,
            env,
            &SimConfig {
                dt_s: dt,
                max_time_s: config.max_time_s,
            },
        )
        .apogee_m
    };
    let a = [
        run(config.dt_s),
        run(config.dt_s / 2.0),
        run(config.dt_s / 4.0),
    ];
    let delta = (a[0] - a[1]).abs();
    let delta_fine = (a[1] - a[2]).abs();
    let tolerance = (a[2].abs() * 0.001).max(0.5);
    ConvergenceReport {
        dt_s: config.dt_s,
        apogee_m: a,
        apogee_delta_m: delta,
        apogee_delta_fine_m: delta_fine,
        converged: delta < tolerance,
    }
}
