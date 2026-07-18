//! Monte Carlo dispersion (v0.2 Step 6): N planar flights with seeded
//! parameter perturbations — the "where might it actually land" answer
//! OpenRocket never gives.
//!
//! Determinism is sacred: the RNG is a self-contained SplitMix64 stream
//! seeded only by `Dispersion.seed` ("ChaCha8 or similar" per the plan; a
//! non-crypto simulation stream needs statistical quality and replayability,
//! not unpredictability). No OS entropy is ever consulted, so the same seed
//! and inputs produce a byte-identical `DispersionSummary` (tested).
//!
//! Planar limitation, stated honestly: flights live in the x–z plane, so
//! the landing "ellipse" degenerates to its downrange axis — `a_m` spans
//! ±2σ of landing range, `b_m` is 0 until the solver grows a crossrange
//! dimension, and the bearing is 0° (downwind) by construction.

use ascent_domain::Motor;
use serde::{Deserialize, Serialize};

use crate::planar::{simulate_planar, PlanarVehicle, WindLayer, WindProfile};
use crate::rocket::{Environment, Rocket};
use crate::sim::SimConfig;

/// Which input a variation perturbs. Sigmas are one standard deviation in
/// the unit named by the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaryParam {
    /// Thrust-curve scale factor, sigma in percent.
    ThrustPct,
    /// Drag coefficient scale factor, sigma in percent.
    CdPct,
    /// Additive wind-speed offset applied to every layer, sigma in m/s.
    WindSpeedMs,
    /// Rail tilt from vertical, sigma in degrees.
    LaunchAngleDeg,
    /// Additive dry-mass offset, sigma in grams.
    MassG,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variation {
    pub param: VaryParam,
    pub sigma: f64,
}

/// A dispersion study request: everything needed to reproduce it exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispersion {
    pub seed: u64,
    pub samples: u32,
    pub vary: Vec<Variation>,
}

/// One member flight, compacted for scatter plots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactRun {
    pub apogee_m: f64,
    pub landing_range_m: f64,
    pub max_aoa_deg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandingEllipse {
    /// Downrange semi-axis: 2σ of landing range, meters.
    pub a_m: f64,
    /// Crossrange semi-axis: 0 for the planar solver (no crossrange yet).
    pub b_m: f64,
    /// Ellipse major-axis bearing; 0° = the wind/flight plane's +x.
    pub bearing_deg: f64,
}

/// The study result. Echoes the request (seed + distributions) so the
/// summary is its own evidence of what was varied and how.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispersionSummary {
    pub seed: u64,
    pub samples: u32,
    pub vary: Vec<Variation>,
    pub apogee_p5_m: f64,
    pub apogee_p50_m: f64,
    pub apogee_p95_m: f64,
    pub landing_mean_m: f64,
    pub landing_ellipse: LandingEllipse,
    pub runs: Vec<CompactRun>,
}

/// SplitMix64: tiny, seedable, statistically solid for simulation use.
/// (Vigna 2015 — the stream PCG and xoshiro use for seeding.)
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Uniform in (0, 1] — never exactly 0, so ln() below is always finite.
    fn next_unit(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 1.0) / (1u64 << 53) as f64
    }

    /// Standard normal via Box–Muller.
    fn next_normal(&mut self) -> f64 {
        let u1 = self.next_unit();
        let u2 = self.next_unit();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// Linear-interpolation percentile (the numpy default), p in [0, 100].
/// `values` need not be sorted.
pub fn percentile(values: &[f64], p: f64) -> f64 {
    assert!(!values.is_empty(), "percentile of empty slice");
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let rank = p / 100.0 * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    sorted[lo] + frac * (sorted[hi] - sorted[lo])
}

fn perturbed_inputs(
    rng: &mut SplitMix64,
    base_rocket: &Rocket,
    base_motor: &Motor,
    base_vehicle: &PlanarVehicle,
    base_wind: &WindProfile,
    vary: &[Variation],
) -> (Rocket, Motor, PlanarVehicle, WindProfile) {
    let mut rocket = base_rocket.clone();
    let mut motor = base_motor.clone();
    let mut vehicle = base_vehicle.clone();
    let mut wind = base_wind.clone();

    for v in vary {
        // One draw per variation, in declaration order — the stream layout
        // is part of the reproducibility contract.
        let n = rng.next_normal();
        match v.param {
            VaryParam::ThrustPct => {
                let factor = (1.0 + n * v.sigma / 100.0).max(0.1);
                for (_, thrust) in &mut motor.thrust_curve {
                    *thrust *= factor;
                }
                motor.expected_total_impulse_ns *= factor;
            }
            VaryParam::CdPct => {
                if let Some(d) = &mut rocket.drag {
                    d.cd = (d.cd * (1.0 + n * v.sigma / 100.0)).max(0.0);
                }
            }
            VaryParam::WindSpeedMs => {
                let delta = n * v.sigma;
                if wind.layers.is_empty() {
                    wind.layers.push(WindLayer {
                        altitude_m: 0.0,
                        speed_ms: delta,
                        direction_deg: 0.0,
                    });
                } else {
                    for layer in &mut wind.layers {
                        layer.speed_ms += delta;
                    }
                }
            }
            VaryParam::LaunchAngleDeg => {
                vehicle.launch_angle_rad += (n * v.sigma).to_radians();
            }
            VaryParam::MassG => {
                rocket.dry_mass_kg = (rocket.dry_mass_kg + n * v.sigma / 1000.0).max(0.001);
            }
        }
    }
    (rocket, motor, vehicle, wind)
}

/// Run the study: `samples` perturbed planar flights, deterministic in
/// `dispersion.seed`.
pub fn run_dispersion(
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    vehicle: &PlanarVehicle,
    wind: &WindProfile,
    config: &SimConfig,
    dispersion: &Dispersion,
) -> Result<DispersionSummary, String> {
    run_dispersion_observed(rocket, motor, env, vehicle, wind, config, dispersion, |_, _| true)
        .map(|outcome| outcome.expect("uncancellable run cannot be cancelled"))
}

/// Same run, but observable: `on_progress(completed, total)` is called
/// before every flight and may return false to cancel cooperatively —
/// `Ok(None)` means cancelled, nothing partial escapes. The observer
/// never touches the RNG stream, so an observed run is byte-identical
/// to a plain one with the same seed.
#[allow(clippy::too_many_arguments)]
pub fn run_dispersion_observed(
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    vehicle: &PlanarVehicle,
    wind: &WindProfile,
    config: &SimConfig,
    dispersion: &Dispersion,
    mut on_progress: impl FnMut(u32, u32) -> bool,
) -> Result<Option<DispersionSummary>, String> {
    if dispersion.samples == 0 {
        return Err("dispersion needs at least 1 sample".into());
    }
    let mut rng = SplitMix64::new(dispersion.seed);
    let mut runs = Vec::with_capacity(dispersion.samples as usize);

    for i in 0..dispersion.samples {
        if !on_progress(i, dispersion.samples) {
            return Ok(None);
        }
        let (r, m, v, w) = perturbed_inputs(&mut rng, rocket, motor, vehicle, wind, &dispersion.vary);
        let flight = simulate_planar(&r, &m, env, &v, &w, config);
        runs.push(CompactRun {
            apogee_m: flight.apogee_m,
            landing_range_m: flight.landing_range_m,
            max_aoa_deg: flight.max_aoa_deg,
        });
    }
    on_progress(dispersion.samples, dispersion.samples);

    let apogees: Vec<f64> = runs.iter().map(|r| r.apogee_m).collect();
    let ranges: Vec<f64> = runs.iter().map(|r| r.landing_range_m).collect();
    let n = ranges.len() as f64;
    let mean = ranges.iter().sum::<f64>() / n;
    let variance = ranges.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;

    Ok(Some(DispersionSummary {
        seed: dispersion.seed,
        samples: dispersion.samples,
        vary: dispersion.vary.clone(),
        apogee_p5_m: percentile(&apogees, 5.0),
        apogee_p50_m: percentile(&apogees, 50.0),
        apogee_p95_m: percentile(&apogees, 95.0),
        landing_mean_m: mean,
        landing_ellipse: LandingEllipse {
            a_m: 2.0 * variance.sqrt(),
            b_m: 0.0,
            bearing_deg: 0.0,
        },
        runs,
    }))
}
