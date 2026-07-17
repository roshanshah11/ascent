use ascent_domain::Motor;
use serde::{Deserialize, Serialize};

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
            max_time_s: 120.0,
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
    pub apogee_m: f64,
    pub apogee_time_s: f64,
    pub burnout_time_s: f64,
    pub burnout_velocity_ms: f64,
    pub burnout_altitude_m: f64,
    pub max_velocity_ms: f64,
    pub liftoff_time_s: f64,
    pub steps: u64,
}

/// State vector for vertical flight: (altitude m, velocity m/s).
type State = (f64, f64);

fn derivative(t: f64, state: State, rocket: &Rocket, motor: &Motor, env: &Environment) -> State {
    let (_h, v) = state;
    let mass = rocket.dry_mass_kg + motor.mass_at(t);
    let thrust = motor.thrust_at(t);
    let drag = match &rocket.drag {
        Some(d) => 0.5 * env.air_density_kgm3 * d.cd * d.reference_area_m2 * v * v * v.signum(),
        None => 0.0,
    };
    let accel = (thrust - drag) / mass - env.gravity_ms2;
    (v, accel)
}

fn rk4_step(
    t: f64,
    state: State,
    dt: f64,
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
) -> State {
    let k1 = derivative(t, state, rocket, motor, env);
    let k2 = derivative(
        t + dt / 2.0,
        (state.0 + dt / 2.0 * k1.0, state.1 + dt / 2.0 * k1.1),
        rocket,
        motor,
        env,
    );
    let k3 = derivative(
        t + dt / 2.0,
        (state.0 + dt / 2.0 * k2.0, state.1 + dt / 2.0 * k2.1),
        rocket,
        motor,
        env,
    );
    let k4 = derivative(
        t + dt,
        (state.0 + dt * k3.0, state.1 + dt * k3.1),
        rocket,
        motor,
        env,
    );
    (
        state.0 + dt / 6.0 * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0),
        state.1 + dt / 6.0 * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1),
    )
}

/// Simulate vertical flight from ignition to apogee with fixed-step RK4.
///
/// The rocket is held on the pad until thrust exceeds weight. Apogee is
/// located by linear interpolation of the velocity zero-crossing between
/// the bracketing steps.
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
    let mut samples = Vec::new();
    let mut steps: u64 = 0;
    let mut max_velocity: f64 = 0.0;
    let mut liftoff_time = f64::NAN;
    let mut burnout_state: Option<(f64, State)> = None;

    let record = |t: f64, s: State, motor: &Motor, rocket: &Rocket| Sample {
        t,
        altitude_m: s.0,
        velocity_ms: s.1,
        mass_kg: rocket.dry_mass_kg + motor.mass_at(t),
        thrust_n: motor.thrust_at(t),
    };
    samples.push(record(t, state, motor, rocket));

    while t < config.max_time_s {
        let on_pad = state.0 <= 0.0 && state.1 <= 0.0;
        if on_pad {
            let weight_now = (rocket.dry_mass_kg + motor.mass_at(t)) * env.gravity_ms2;
            let weight_next = (rocket.dry_mass_kg + motor.mass_at(t + dt)) * env.gravity_ms2;
            let held = motor.thrust_at(t) <= weight_now && motor.thrust_at(t + dt) <= weight_next;
            if held {
                // Held by the pad for the whole step; advance time without integrating.
                if t >= burn_time {
                    // Motor never lifts this rocket.
                    break;
                }
                t += dt;
                steps += 1;
                continue;
            }
            if liftoff_time.is_nan() {
                liftoff_time = t;
            }
        }

        let mut next = rk4_step(t, state, dt, rocket, motor, env);
        let t_next = t + dt;
        if on_pad {
            // The pad cannot pull the rocket below ground during the
            // liftoff transition step.
            next.0 = next.0.max(0.0);
            next.1 = next.1.max(0.0);
        }

        // Burnout crossing: capture state at the burn-time boundary.
        if burnout_state.is_none() && t_next >= burn_time {
            let frac = ((burn_time - t) / dt).clamp(0.0, 1.0);
            let bh = state.0 + frac * (next.0 - state.0);
            let bv = state.1 + frac * (next.1 - state.1);
            burnout_state = Some((burn_time, (bh, bv)));
        }

        // Apogee: velocity crosses zero from above after liftoff.
        if !liftoff_time.is_nan() && state.1 > 0.0 && next.1 <= 0.0 {
            let frac = state.1 / (state.1 - next.1);
            let apogee_t = t + frac * dt;
            let apogee_h = state.0 + frac * (next.0 - state.0);
            samples.push(record(t_next, next, motor, rocket));
            let (bt, (bh, bv)) = burnout_state.unwrap_or((apogee_t, (apogee_h, 0.0)));
            return SimResult {
                samples,
                apogee_m: apogee_h,
                apogee_time_s: apogee_t,
                burnout_time_s: bt,
                burnout_velocity_ms: bv,
                burnout_altitude_m: bh,
                max_velocity_ms: max_velocity,
                liftoff_time_s: liftoff_time,
                steps: steps + 1,
            };
        }

        state = next;
        t = t_next;
        steps += 1;
        max_velocity = max_velocity.max(state.1);
        samples.push(record(t, state, motor, rocket));
    }

    // Never reached apogee (or never lifted off) within max_time.
    let (bt, (bh, bv)) = burnout_state.unwrap_or((burn_time, (state.0, state.1)));
    SimResult {
        samples,
        apogee_m: state.0,
        apogee_time_s: t,
        burnout_time_s: bt,
        burnout_velocity_ms: bv,
        burnout_altitude_m: bh,
        max_velocity_ms: max_velocity,
        liftoff_time_s: liftoff_time,
        steps,
    }
}
