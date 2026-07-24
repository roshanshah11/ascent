//! Executed, non-calibrated comparison of Ascent's authoritative vertical
//! engine against the Notre Dame Rocketry Team (NDRT) 2020 full-scale flight.
//!
//! The measured trace and motor come from the RocketPy team's `RocketPaper`
//! repository (MIT, © 2020 Projeto Jupiter), pinned by commit and SHA-256 in
//! `docs/validation/NDRT_2020_SOURCE_SPEC.md`. Ascent performed no fitting or
//! tuning against the measured flight: the vehicle mass, drag coefficient,
//! reference area, motor thrust curve, standard atmosphere, and launch
//! alignment are all fixed from the sources *before* any result is seen. The
//! frozen pass/fail tolerances live on [`Tolerances`] and are the acceptance
//! bar, not knobs.
//!
//! This is **not** a blind, independent, or preflight validation: the
//! independence of the externally supplied NDRT parameters — especially the
//! drag coefficient `Cd = 0.44` — from the measured flight has not been
//! established. The values are reused as published; whether they were
//! themselves derived against this flight is unknown.
//!
//! Scope: launch through **measured apogee** only. Descent, recovery, velocity,
//! and acceleration are never used as pass/fail metrics. The overall result is
//! derived from the three **primary** flight metrics (apogee, time to apogee,
//! normalized altitude RMSE). Burnout is an **input-consistency** check
//! (simulated burnout vs. the imported motor's burn time), reported but never
//! counted toward the flight-validation pass/fail.

use std::collections::BTreeMap;

use ascent_domain::{parse_eng, Motor};
use ascent_sim::{
    content_hash, input_hash, simulate_vertical, AtmosphereModel, DragModel, Environment, Rocket,
    SimConfig, SimResult,
};
use sha2::{Digest, Sha256};

use crate::validation::{ComparedMetric, ComparisonArtifact, MetricKind};

/// The case id and title used by the checked-in validation case.
pub const CASE_ID: &str = "ndrt-2020-flight";

/// Version identity of the model that produced the simulated values. Stable
/// across runs (pure function of the crate version + engine name), so the
/// artifact is reproducible.
pub const MODEL_VERSION: &str = concat!(
    "ascent-sim/",
    env!("CARGO_PKG_VERSION"),
    " simulate_vertical (RK4 1-DOF vertical)"
);

/// Feet→metre conversion used by the *source* notebook (`ft / 3.28084`). Kept
/// identical so the measured channel is normalized exactly as the upstream
/// analysis did — see the source spec.
pub const FEET_PER_METER: f64 = 3.28084;

/// Vehicle and environment inputs, transcribed from the goal's input block and
/// the NDRT flight-readiness values. None of these is a free parameter: each is
/// sourced and frozen before any comparison runs.
pub mod inputs {
    /// Airframe mass without the motor, kg. The motor's loaded mass is carried
    /// by the imported [`Motor`](ascent_domain::Motor), so `loaded = airframe +
    /// motor.total_mass_kg` reproduces the 23.321 kg lift-off mass.
    pub const AIRFRAME_DRY_MASS_KG: f64 = 18.998;
    /// Body radius, m (0.203 m / 2 airframe). Drives the drag reference area.
    pub const BODY_RADIUS_M: f64 = 0.1015;
    /// Constant drag coefficient (Ascent's day-2 constant-Cd model).
    pub const DRAG_CD: f64 = 0.44;
    /// Launch rail length, m (12 ft 1515 rail).
    pub const RAIL_LENGTH_M: f64 = 3.353;
    /// Standard gravity, m/s².
    pub const GRAVITY_MS2: f64 = 9.80665;
}

/// Frozen acceptance tolerances. Set from the goal's metric block **before**
/// any result is viewed; they are the pass/fail bar, not tunable knobs.
pub struct Tolerances;

impl Tolerances {
    /// Apogee: relative error ≤ 10 %.
    pub const APOGEE_REL: f64 = 0.10;
    /// Time to apogee: relative error ≤ 10 %.
    pub const TIME_TO_APOGEE_REL: f64 = 0.10;
    /// Normalized altitude RMSE (RMSE / measured apogee) ≤ 10 %.
    pub const NORMALIZED_RMSE: f64 = 0.10;
    /// Burnout difference ≤ 1 simulation timestep. Input consistency only.
    pub const BURNOUT_STEPS: f64 = 1.0;
}

/// The RAVEN altimeter CSV, parsed *without editing*: the raw header and every
/// raw field are preserved in order, and the typed channels are derived from
/// them. The file packs two independent time series column-wise — a high-rate
/// axial-acceleration channel and a lower-rate barometric-altitude channel —
/// each with its own clock. Only the altitude channel drives the comparison.
#[derive(Debug, Clone)]
pub struct RavenTelemetry {
    /// The header line, verbatim (no trimming, no re-casing).
    pub raw_header: String,
    /// Every data row's raw comma-split fields, in file order.
    pub raw_rows: Vec<Vec<String>>,
    /// Axial-acceleration channel time, seconds (column 0).
    pub accel_time_s: Vec<f64>,
    /// Axial acceleration, g (column 1) — already in g, no conversion.
    pub accel_g: Vec<f64>,
    /// Barometric-altitude channel time, seconds (column 3).
    pub alt_time_s: Vec<f64>,
    /// Altitude above ground level, feet (column 4), raw.
    pub alt_ft_agl: Vec<f64>,
}

impl RavenTelemetry {
    /// Parse the fixture bytes. Rows, columns, order, and raw values are all
    /// preserved; numeric channels are parsed alongside without discarding the
    /// raw fields. Every data row must carry the six expected columns.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let raw_header = lines.next().ok_or("telemetry is empty")?.to_string();
        let mut raw_rows = Vec::new();
        let (mut accel_time_s, mut accel_g) = (Vec::new(), Vec::new());
        let (mut alt_time_s, mut alt_ft_agl) = (Vec::new(), Vec::new());
        for (i, line) in lines.enumerate() {
            if line.is_empty() {
                continue;
            }
            let fields: Vec<String> = line.split(',').map(|f| f.to_string()).collect();
            if fields.len() != 6 {
                return Err(format!(
                    "telemetry row {} has {} fields, expected 6",
                    i + 2,
                    fields.len()
                ));
            }
            let parse = |idx: usize| -> Result<f64, String> {
                fields[idx]
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| format!("row {} column {} is not numeric", i + 2, idx))
            };
            accel_time_s.push(parse(0)?);
            accel_g.push(parse(1)?);
            alt_time_s.push(parse(3)?);
            alt_ft_agl.push(parse(4)?);
            raw_rows.push(fields);
        }
        if raw_rows.is_empty() {
            return Err("telemetry has no data rows".into());
        }
        Ok(Self {
            raw_header,
            raw_rows,
            accel_time_s,
            accel_g,
            alt_time_s,
            alt_ft_agl,
        })
    }

    /// Number of data rows.
    pub fn len(&self) -> usize {
        self.raw_rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.raw_rows.is_empty()
    }

    /// Altitude channel normalized to metres AGL, using the source's exact
    /// `ft / 3.28084` conversion.
    pub fn altitude_m_agl(&self) -> Vec<f64> {
        self.alt_ft_agl
            .iter()
            .map(|ft| ft / FEET_PER_METER)
            .collect()
    }

    /// Row index of measured apogee: the argmax of the AGL altitude channel,
    /// ties resolved to the first occurrence. Deterministic and independent of
    /// any simulation.
    pub fn apogee_index(&self) -> usize {
        let alt = self.altitude_m_agl();
        let mut best = 0usize;
        for (i, &a) in alt.iter().enumerate() {
            if a > alt[best] {
                best = i;
            }
        }
        best
    }

    /// Measured apogee altitude, metres AGL (the maximum of the channel).
    pub fn measured_apogee_agl_m(&self) -> f64 {
        self.altitude_m_agl()[self.apogee_index()]
    }

    /// Measured time to apogee, seconds. This is the altitude channel's own
    /// timestamp at the apogee row: launch is aligned to the start of the
    /// recording (no offset is applied), which is exactly how the source
    /// notebook reports time-to-apogee against the ignition-referenced
    /// simulation clock. Deterministic and simulation-independent.
    pub fn measured_time_to_apogee_s(&self) -> f64 {
        self.alt_time_s[self.apogee_index()]
    }
}

/// Everything the executed comparison produced: the machine-readable artifact
/// plus the raw measured/simulated diagnostics used to write the comparison
/// report. `artifact.pass` is the overall result.
#[derive(Debug, Clone)]
pub struct NdrtComparison {
    pub artifact: ComparisonArtifact,
    pub measured_apogee_agl_m: f64,
    pub measured_time_to_apogee_s: f64,
    pub simulated_apogee_agl_m: f64,
    pub simulated_time_to_apogee_s: f64,
    pub simulated_burnout_time_s: f64,
    pub motor_burn_time_s: f64,
    pub normalized_rmse: f64,
    pub rmse_m: f64,
    /// Number of measured altitude samples in the launch→apogee window.
    pub compared_samples: usize,
    pub timestep_s: f64,
}

impl NdrtComparison {
    /// `(passed, total)` over the **primary** flight-validation metrics (apogee,
    /// time to apogee, normalized altitude RMSE).
    pub fn primary_metrics(&self) -> (usize, usize) {
        let primary = self
            .artifact
            .metrics
            .iter()
            .filter(|m| m.kind == MetricKind::Primary);
        (primary.clone().filter(|m| m.pass).count(), primary.count())
    }

    /// `(passed, total)` over the **input-consistency** checks (burnout).
    pub fn input_checks(&self) -> (usize, usize) {
        let checks = self
            .artifact
            .metrics
            .iter()
            .filter(|m| m.kind == MetricKind::InputConsistency);
        (checks.clone().filter(|m| m.pass).count(), checks.count())
    }
}

/// Build the Ascent inputs from the frozen source values. This is the input
/// mapping the comparison runs; the same values feed [`input_hash`], so any
/// change to them changes the config hash.
pub fn build_inputs(motor: &Motor) -> (Rocket, Environment, SimConfig) {
    let rocket = Rocket {
        name: "NDRT 2020 Launch Vehicle".to_string(),
        dry_mass_kg: inputs::AIRFRAME_DRY_MASS_KG,
        drag: Some(DragModel {
            cd: inputs::DRAG_CD,
            reference_area_m2: std::f64::consts::PI * inputs::BODY_RADIUS_M * inputs::BODY_RADIUS_M,
        }),
        // Descent is out of scope for this comparison; a ballistic post-apogee
        // path has no effect on the launch→apogee window being compared.
        recovery: None,
    };
    let env = Environment {
        gravity_ms2: inputs::GRAVITY_MS2,
        // Ascent cannot consume the source's ERA5 NetCDF sounding (`env_23.nc`);
        // it uses the 1976 US Standard Atmosphere. This is a declared
        // atmosphere limit, documented in the comparison report — not a fit.
        atmosphere: AtmosphereModel::Standard,
        rail_length_m: inputs::RAIL_LENGTH_M,
    };
    let _ = motor;
    (rocket, env, SimConfig::default())
}

/// Parse the pinned Cesaroni motor from its `.eng` bytes. The first (only)
/// entry is the flight motor.
pub fn parse_motor(eng_text: &str) -> Result<Motor, String> {
    let motors = parse_eng(
        eng_text,
        &serde_json::json!({ "source": "RocketPaper NDRT_2020" }),
    )
    .map_err(|e| e.to_string())?;
    motors
        .into_iter()
        .next()
        .ok_or_else(|| "no motor entry in .eng source".to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Linearly interpolate the simulated altitude trace onto time `t`. The samples
/// are dense (fixed dt) and monotonic in time; `t` outside the sampled range
/// clamps to the nearest endpoint.
fn interpolate_altitude(result: &SimResult, t: f64) -> f64 {
    let samples = &result.samples;
    if t <= samples[0].t {
        return samples[0].altitude_m;
    }
    let last = samples[samples.len() - 1];
    if t >= last.t {
        return last.altitude_m;
    }
    let above = samples.partition_point(|s| s.t <= t);
    let (lo, hi) = (&samples[above - 1], &samples[above]);
    let frac = (t - lo.t) / (hi.t - lo.t);
    lo.altitude_m + frac * (hi.altitude_m - lo.altitude_m)
}

/// Run the executed, non-calibrated comparison. All errors and statuses are
/// recomputed here from the measured telemetry and the freshly simulated
/// trace; nothing is read back from a stored result.
///
/// `case_hash` binds the produced artifact to the exact checked-in case
/// specification it was run against.
pub fn run_comparison(
    csv_bytes: &[u8],
    eng_bytes: &[u8],
    case_hash: &str,
) -> Result<NdrtComparison, String> {
    let csv_text =
        std::str::from_utf8(csv_bytes).map_err(|_| "telemetry is not valid UTF-8".to_string())?;
    let eng_text =
        std::str::from_utf8(eng_bytes).map_err(|_| ".eng source is not valid UTF-8".to_string())?;

    let telemetry = RavenTelemetry::parse(csv_text)?;
    let motor = parse_motor(eng_text)?;
    motor.validate().map_err(|e| e.to_string())?;

    let (rocket, env, config) = build_inputs(&motor);
    let result = simulate_vertical(&rocket, &motor, &env, &config);

    // Measured metrics, derived from the telemetry.
    let apogee_idx = telemetry.apogee_index();
    let measured_apogee = telemetry.measured_apogee_agl_m();
    let measured_tta = telemetry.measured_time_to_apogee_s();
    let alt_m = telemetry.altitude_m_agl();

    // Interpolate the simulated altitude onto every measured timestamp in the
    // launch→apogee window and accumulate the squared residual.
    let mut sq_sum = 0.0;
    let mut count = 0usize;
    for (t, measured) in telemetry.alt_time_s[..=apogee_idx]
        .iter()
        .zip(&alt_m[..=apogee_idx])
    {
        let sim_alt = interpolate_altitude(&result, *t);
        let residual = sim_alt - *measured;
        sq_sum += residual * residual;
        count += 1;
    }
    let rmse_m = (sq_sum / count as f64).sqrt();
    let normalized_rmse = if measured_apogee.abs() > 0.0 {
        rmse_m / measured_apogee.abs()
    } else {
        0.0
    };

    let sim_apogee = result.apogee_m;
    let sim_tta = result.apogee_time_s;
    let sim_burnout = result.burnout_time_s;
    let motor_burn = motor.burn_time();

    let metrics = vec![
        ComparedMetric::derive(
            "apogee_agl_m",
            "meter",
            measured_apogee,
            sim_apogee,
            Tolerances::APOGEE_REL * measured_apogee.abs(),
        ),
        ComparedMetric::derive(
            "time_to_apogee_s",
            "second",
            measured_tta,
            sim_tta,
            Tolerances::TIME_TO_APOGEE_REL * measured_tta.abs(),
        ),
        // Normalized altitude RMSE: baseline 0, "simulated" is the normalized
        // residual, tolerance is the frozen 10 %. `abs_error` therefore equals
        // the normalized RMSE and the pass follows directly.
        ComparedMetric::derive(
            "normalized_altitude_rmse",
            "ratio",
            0.0,
            normalized_rmse,
            Tolerances::NORMALIZED_RMSE,
        ),
        // Burnout input-consistency: simulated burnout vs. the imported motor
        // burn time, tolerance one simulation timestep. Marked as an
        // input-consistency check so it is verified and reported but excluded
        // from the overall flight-validation pass/fail.
        ComparedMetric::derive(
            "burnout_time_s",
            "second",
            motor_burn,
            sim_burnout,
            Tolerances::BURNOUT_STEPS * config.dt_s,
        )
        .as_input_consistency(),
    ];

    let source_hashes = BTreeMap::from([
        ("telemetry_csv".to_string(), sha256_hex(csv_bytes)),
        ("motor_eng".to_string(), sha256_hex(eng_bytes)),
        (
            "model_inputs".to_string(),
            content_hash(&serde_json::json!({
                "rocket": rocket,
                "motor": motor,
                "environment": env,
                "config": config,
            })),
        ),
    ]);

    let artifact = ComparisonArtifact::assemble(
        CASE_ID,
        case_hash,
        MODEL_VERSION,
        source_hashes,
        input_hash(&rocket, &motor, &env, &config),
        metrics,
        vec![
            "Atmosphere: Ascent uses the 1976 US Standard Atmosphere; the source \
             ERA5 sounding (env_23.nc) is not consumed."
                .to_string(),
            "Altitude channel is barometric AGL; density altitude offset from the \
             206 m launch elevation is not applied (standard atmosphere referenced \
             to sea level)."
                .to_string(),
            "Comparison window is launch→measured apogee only; descent, recovery, \
             velocity, and acceleration are excluded from pass/fail."
                .to_string(),
        ],
    )?;

    Ok(NdrtComparison {
        artifact,
        measured_apogee_agl_m: measured_apogee,
        measured_time_to_apogee_s: measured_tta,
        simulated_apogee_agl_m: sim_apogee,
        simulated_time_to_apogee_s: sim_tta,
        simulated_burnout_time_s: sim_burnout,
        motor_burn_time_s: motor_burn,
        normalized_rmse,
        rmse_m,
        compared_samples: count,
        timestep_s: config.dt_s,
    })
}
