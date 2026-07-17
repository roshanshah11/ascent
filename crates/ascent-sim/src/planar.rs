//! 3-DOF planar flight (v0.2 Step 5): x, z, and pitch in a vertical plane,
//! with a layered wind profile and Barrowman-derived weathercocking. The
//! vertical solver remains the validated degenerate case: calm wind and a
//! vertical rail must reproduce its apogee to 1e-9 (tested).
//!
//! Deviation from the plan doc: `WindProfile` is an explicit argument to
//! `simulate_planar`, not a field on `SimConfig` — `SimConfig` serializes
//! into every run's SHA-256 input hash, so growing it would invalidate the
//! golden regression pin for unchanged vertical runs.
//!
//! Model notes (small-angle Barrowman regime, subsonic):
//! - Pitch angle θ is measured from vertical; +θ tilts the nose toward +x.
//! - Angle of attack α = θ − γ, where γ is the air-relative velocity angle.
//! - Normal force N = ½ρv²·A_ref·CNα·α acts perpendicular to the body axis
//!   at the CP; with CP aft of CG the resulting moment is restoring, which
//!   is exactly the weathercocking mechanism (and a fin-off vehicle with CP
//!   ahead of CG diverges — tested).
//! - No pitch-damping term: undamped oscillation is conservative for
//!   max-AoA reporting. Descent is a point mass under chute drifting with
//!   the wind (no pitch dynamics while dangling).

use ascent_domain::Motor;
use serde::{Deserialize, Serialize};

use crate::rocket::{Environment, Rocket};
use crate::sim::SimConfig;

/// One wind layer: applies from `altitude_m` upward until the next layer.
/// `direction_deg` is the compass-style direction the wind blows **toward**,
/// projected onto the flight plane: 0° = +x, 180° = −x. Out-of-plane
/// components are ignored by the planar solver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindLayer {
    pub altitude_m: f64,
    pub speed_ms: f64,
    pub direction_deg: f64,
}

/// Piecewise-constant wind vs altitude. Empty = calm (the default).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindProfile {
    pub layers: Vec<WindLayer>,
}

impl WindProfile {
    pub fn calm() -> Self {
        Self::default()
    }

    /// Uniform wind at all altitudes blowing toward +x.
    pub fn constant(speed_ms: f64) -> Self {
        Self {
            layers: vec![WindLayer {
                altitude_m: 0.0,
                speed_ms,
                direction_deg: 0.0,
            }],
        }
    }

    /// In-plane wind velocity (+x component) at an altitude: the highest
    /// layer at or below `altitude_m` applies; below the first layer, calm.
    pub fn wind_x_at(&self, altitude_m: f64) -> f64 {
        self.layers
            .iter()
            .filter(|l| l.altitude_m <= altitude_m)
            .max_by(|a, b| a.altitude_m.total_cmp(&b.altitude_m))
            .map(|l| l.speed_ms * l.direction_deg.to_radians().cos())
            .unwrap_or(0.0)
    }
}

/// Rigid-body pitch inputs the planar solver needs beyond `Rocket`. The
/// caller computes CP/CNα with ascent-aero (Barrowman) and CG/inertia from
/// its mass model — this crate stays free of geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanarVehicle {
    /// Total normal-force coefficient derivative, per radian (Barrowman).
    pub cn_alpha_per_rad: f64,
    /// Center of pressure, meters aft of the nose tip.
    pub cp_from_nose_m: f64,
    /// Center of gravity, meters aft of the nose tip.
    pub cg_from_nose_m: f64,
    /// Pitch moment of inertia about the CG, kg·m².
    pub pitch_inertia_kgm2: f64,
    /// Aerodynamic reference area (body cross-section), m².
    pub reference_area_m2: f64,
}

impl PlanarVehicle {
    /// Static margin in meters: positive = CP aft of CG = stable.
    pub fn margin_m(&self) -> f64 {
        self.cp_from_nose_m - self.cg_from_nose_m
    }
}

/// Superset of the vertical summary's headline numbers plus the planar-only
/// quantities. (Full SimSummary integration arrives with the dispersion UI.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanarSummary {
    pub apogee_m: f64,
    pub apogee_time_s: f64,
    pub max_velocity_ms: f64,
    pub landing_time_s: f64,
    /// Signed downrange landing displacement, meters (+x = downwind for a
    /// 0° wind direction).
    pub landing_range_m: f64,
    /// Largest |angle of attack| seen during powered/coast ascent, degrees.
    pub max_aoa_deg: f64,
    /// Pitch angle away from vertical shortly after rail exit (the
    /// weathercock response), degrees, signed.
    pub weathercock_deg: f64,
    pub steps: u64,
}

/// State vector: (x, z, vx, vz, theta, q).
type State = [f64; 6];

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Pad,
    Rail,
    Ascent,
    Descent,
}

fn derivative(
    t: f64,
    s: State,
    phase: Phase,
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    vehicle: &PlanarVehicle,
    wind: &WindProfile,
) -> State {
    let [_x, z, vx, vz, theta, q] = s;
    let mass = rocket.dry_mass_kg + motor.mass_at(t);
    let thrust = motor.thrust_at(t);
    let rho = env.atmosphere.density_at(z.max(0.0));
    let body_cda = rocket.drag.as_ref().map(|d| d.cd * d.reference_area_m2).unwrap_or(0.0);

    match phase {
        Phase::Pad | Phase::Rail => {
            // Constrained to the (vertical) rail: 1-D dynamics, pitch held.
            let v = vz;
            let drag = 0.5 * rho * body_cda * v * v * v.signum();
            let accel = (thrust - drag) / mass - env.gravity_ms2;
            [0.0, v, 0.0, accel, 0.0, 0.0]
        }
        Phase::Ascent => {
            let wind_x = wind.wind_x_at(z);
            let (rvx, rvz) = (vx - wind_x, vz);
            let v = (rvx * rvx + rvz * rvz).sqrt();

            // Body axis (nose direction) and its in-plane perpendicular.
            let (sin_t, cos_t) = theta.sin_cos();

            // Thrust along the body axis.
            let (mut ax, mut az) = (thrust * sin_t / mass, thrust * cos_t / mass);

            if v > 1e-9 {
                // Axial drag along −v_rel.
                let drag = 0.5 * rho * body_cda * v;
                ax -= drag * rvx / mass;
                az -= drag * rvz / mass;
            }

            let mut q_dot = 0.0;
            // Normal force only applies in nose-first flight (positive axial
            // relative velocity). This also protects RK4 substages that peek
            // past apogee, where atan2 would report a spurious 180° AoA.
            let axial = rvx * sin_t + rvz * cos_t;
            if axial > 1e-3 {
                // Angle of attack from body-frame components of v_rel.
                let crossflow = rvx * cos_t - rvz * sin_t;
                let alpha = (-crossflow).atan2(axial);
                // Normal force (linear in α, Barrowman regime) along the
                // body-perpendicular (cosθ, −sinθ); positive α pushes the
                // vehicle downwind while the moment turns the nose upwind.
                let n_force = 0.5 * rho * v * v * vehicle.reference_area_m2
                    * vehicle.cn_alpha_per_rad
                    * alpha;
                ax += n_force * cos_t / mass;
                az += n_force * (-sin_t) / mass;
                // Restoring (CP aft of CG) or divergent (CP ahead) moment.
                q_dot = -vehicle.margin_m() * n_force / vehicle.pitch_inertia_kgm2;
            }

            az -= env.gravity_ms2;
            [vx, vz, ax, az, q, q_dot]
        }
        Phase::Descent => {
            // Point mass under chute: body + chute drag on the air-relative
            // velocity; the canopy drifts with the wind. Pitch frozen.
            let wind_x = wind.wind_x_at(z);
            let (rvx, rvz) = (vx - wind_x, vz);
            let v = (rvx * rvx + rvz * rvz).sqrt();
            let chute_cda = rocket
                .recovery
                .as_ref()
                .map(|r| r.chute_cd * r.chute_area_m2)
                .unwrap_or(0.0);
            let cda = body_cda + chute_cda;
            let (mut ax, mut az) = (0.0, -env.gravity_ms2);
            if v > 1e-9 {
                let drag = 0.5 * rho * cda * v;
                ax -= drag * rvx / mass;
                az -= drag * rvz / mass;
            }
            [vx, vz, ax, az, 0.0, 0.0]
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rk4_step(
    t: f64,
    s: State,
    dt: f64,
    phase: Phase,
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    vehicle: &PlanarVehicle,
    wind: &WindProfile,
) -> State {
    let f = |t: f64, s: State| derivative(t, s, phase, rocket, motor, env, vehicle, wind);
    let add = |a: State, b: State, k: f64| {
        let mut out = [0.0; 6];
        for i in 0..6 {
            out[i] = a[i] + k * b[i];
        }
        out
    };
    let k1 = f(t, s);
    let k2 = f(t + dt / 2.0, add(s, k1, dt / 2.0));
    let k3 = f(t + dt / 2.0, add(s, k2, dt / 2.0));
    let k4 = f(t + dt, add(s, k3, dt));
    let mut out = [0.0; 6];
    for i in 0..6 {
        out[i] = s[i] + dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
    }
    out
}

/// Simulate a planar 3-DOF flight: pad hold, rail run, weathercocking
/// ascent, apogee, chute descent with wind drift, landing.
pub fn simulate_planar(
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    vehicle: &PlanarVehicle,
    wind: &WindProfile,
    config: &SimConfig,
) -> PlanarSummary {
    let dt = config.dt_s;
    let burn_time = motor.burn_time();
    let mut t = 0.0;
    let mut s: State = [0.0; 6];
    let mut phase = Phase::Pad;
    let mut steps: u64 = 0;
    let mut max_velocity: f64 = 0.0;
    let mut max_aoa_rad: f64 = 0.0;
    let mut weathercock_rad = f64::NAN;
    let mut rail_exit_t = f64::NAN;
    let mut apogee = (f64::NAN, f64::NAN); // (time, altitude)
    let mut landing = (f64::NAN, f64::NAN); // (time, range)

    while t < config.max_time_s {
        if phase == Phase::Pad {
            let weight_now = (rocket.dry_mass_kg + motor.mass_at(t)) * env.gravity_ms2;
            let weight_next = (rocket.dry_mass_kg + motor.mass_at(t + dt)) * env.gravity_ms2;
            let held = motor.thrust_at(t) <= weight_now && motor.thrust_at(t + dt) <= weight_next;
            if held {
                if t >= burn_time {
                    break; // Motor never lifts this rocket.
                }
                t += dt;
                steps += 1;
                continue;
            }
            phase = Phase::Rail;
        }

        let mut next = rk4_step(t, s, dt, phase, rocket, motor, env, vehicle, wind);
        // The pad cannot pull the rocket below ground during liftoff.
        if phase == Phase::Rail && s[1] <= 0.0 && s[3] <= 0.0 && t < burn_time {
            next[1] = next[1].max(0.0);
            next[3] = next[3].max(0.0);
        }

        // Rail exit: free flight (and pitch dynamics) begin.
        if phase == Phase::Rail && next[1] >= env.rail_length_m {
            phase = Phase::Ascent;
            rail_exit_t = t + dt;
        }

        if phase == Phase::Ascent {
            // Track angle of attack for the summary.
            let wind_x = wind.wind_x_at(next[1]);
            let (rvx, rvz) = (next[2] - wind_x, next[3]);
            let (sin_t, cos_t) = next[4].sin_cos();
            let axial = rvx * sin_t + rvz * cos_t;
            if axial > 1e-3 {
                let crossflow = rvx * cos_t - rvz * sin_t;
                let alpha = (-crossflow).atan2(axial);
                if alpha.abs() > max_aoa_rad.abs() {
                    max_aoa_rad = alpha;
                }
            }
            // Weathercock response: pitch angle half a second after rail
            // exit (enough time for the initial turn-into-wind to develop).
            if weathercock_rad.is_nan() && !rail_exit_t.is_nan() && t + dt >= rail_exit_t + 0.5 {
                weathercock_rad = next[4];
            }
        }

        // Apogee: vertical-velocity zero-crossing from above.
        if (phase == Phase::Ascent || phase == Phase::Rail) && s[3] > 0.0 && next[3] <= 0.0 {
            let frac = (s[3] / (s[3] - next[3])).clamp(0.0, 1.0);
            let apogee_t = t + frac * dt;
            let mut ap = [0.0; 6];
            for i in 0..6 {
                ap[i] = s[i] + frac * (next[i] - s[i]);
            }
            apogee = (apogee_t, ap[1]);
            // Restart the step from apogee so chute drag applies from the
            // true apogee point (mirrors the vertical solver).
            ap[3] = 0.0;
            s = ap;
            t = apogee_t;
            phase = Phase::Descent;
            steps += 1;
            continue;
        }

        // Landing: altitude crosses zero on the way down.
        if phase == Phase::Descent && s[1] > 0.0 && next[1] <= 0.0 {
            let frac = (s[1] / (s[1] - next[1])).clamp(0.0, 1.0);
            landing = (t + frac * dt, s[0] + frac * (next[0] - s[0]));
            steps += 1;
            break;
        }

        s = next;
        t += dt;
        steps += 1;
        let speed = (s[2] * s[2] + s[3] * s[3]).sqrt();
        max_velocity = max_velocity.max(speed);

        // A divergent (unstable) vehicle tumbles; once the pitch passes 90°
        // the small-angle model is meaningless — stop integrating ascent and
        // let the summary report the blow-up honestly.
        if phase == Phase::Ascent && s[4].abs() > std::f64::consts::FRAC_PI_2 {
            max_aoa_rad = max_aoa_rad.max(s[4].abs());
            break;
        }
    }

    PlanarSummary {
        apogee_m: if apogee.1.is_nan() { s[1] } else { apogee.1 },
        apogee_time_s: if apogee.0.is_nan() { t } else { apogee.0 },
        max_velocity_ms: max_velocity,
        landing_time_s: landing.0,
        landing_range_m: if landing.1.is_nan() { s[0] } else { landing.1 },
        max_aoa_deg: max_aoa_rad.to_degrees(),
        weathercock_deg: if weathercock_rad.is_nan() {
            0.0
        } else {
            weathercock_rad.to_degrees()
        },
        steps,
    }
}

/// Timestep-convergence for the planar solver: apogee and landing range at
/// dt, dt/2, dt/4, with the same acceptance rule as the vertical report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanarConvergence {
    pub dt_s: f64,
    pub apogee_m: [f64; 3],
    pub landing_range_m: [f64; 3],
    pub apogee_delta_m: f64,
    pub converged: bool,
}

pub fn planar_convergence(
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    vehicle: &PlanarVehicle,
    wind: &WindProfile,
    config: &SimConfig,
) -> PlanarConvergence {
    let run = |dt: f64| {
        simulate_planar(
            rocket,
            motor,
            env,
            vehicle,
            wind,
            &SimConfig {
                dt_s: dt,
                max_time_s: config.max_time_s,
            },
        )
    };
    let results = [
        run(config.dt_s),
        run(config.dt_s / 2.0),
        run(config.dt_s / 4.0),
    ];
    let apogee = [results[0].apogee_m, results[1].apogee_m, results[2].apogee_m];
    let range = [
        results[0].landing_range_m,
        results[1].landing_range_m,
        results[2].landing_range_m,
    ];
    let delta = (apogee[0] - apogee[1]).abs();
    let tolerance = (apogee[2].abs() * 0.001).max(0.5);
    PlanarConvergence {
        dt_s: config.dt_s,
        apogee_m: apogee,
        landing_range_m: range,
        apogee_delta_m: delta,
        converged: delta < tolerance,
    }
}
