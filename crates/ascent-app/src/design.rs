//! Design DTO (what the UI edits) and its mapping onto the sim crates.

use ascent_domain::Motor;
use ascent_sim::{
    simulate_vertical, AtmosphereModel, DragModel, Environment, Recovery, Rocket, SimConfig,
    SimSummary,
};
use serde::{Deserialize, Serialize};

const C6_JSON: &str = include_str!("../../ascent-domain/data/motors/estes_c6.json");

/// Motors bundled with the app (provenance-preserved JSON). More arrive via
/// Codex task A4; adding one = adding an include_str! entry here.
const MOTOR_SOURCES: &[&str] = &[C6_JSON];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChuteSpec {
    pub enabled: bool,
    pub diameter_cm: f64,
    pub cd: f64,
}

/// Everything the inspector can edit, in UI-friendly units.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Design {
    pub name: String,
    pub dry_mass_g: f64,
    pub diameter_mm: f64,
    pub cd: f64,
    pub chute: ChuteSpec,
    pub motor_designation: String,
    pub rail_length_m: f64,
}

impl Design {
    /// The reference rocket: Estes Alpha III on a C6 (Day 1's baseline).
    pub fn reference() -> Self {
        Design {
            name: "Estes Alpha III".into(),
            dry_mass_g: 34.0,
            diameter_mm: 25.0,
            cd: 0.60,
            chute: ChuteSpec {
                enabled: true,
                diameter_cm: 30.0,
                cd: 0.75,
            },
            motor_designation: "C6".into(),
            rail_length_m: 0.9,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorInfo {
    pub designation: String,
    pub manufacturer: String,
    pub total_impulse_ns: f64,
    pub burn_time_s: f64,
    pub total_mass_g: f64,
}

pub fn bundled_motors() -> Vec<MotorInfo> {
    MOTOR_SOURCES
        .iter()
        .filter_map(|src| Motor::from_json(src).ok())
        .map(|m| MotorInfo {
            designation: m.designation.clone(),
            manufacturer: m.manufacturer.clone(),
            total_impulse_ns: m.total_impulse(),
            burn_time_s: m.burn_time(),
            total_mass_g: m.total_mass_kg * 1000.0,
        })
        .collect()
}

fn find_motor(designation: &str) -> Result<Motor, String> {
    MOTOR_SOURCES
        .iter()
        .filter_map(|src| Motor::from_json(src).ok())
        .find(|m| m.designation == designation)
        .ok_or_else(|| format!("unknown motor: {designation}"))
}

/// One playback sample. Downsampled for the UI; the summary keeps the
/// full-resolution numbers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSample {
    pub t: f64,
    pub altitude_m: f64,
    pub velocity_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEvent {
    pub kind: String,
    pub t: f64,
    pub altitude_m: f64,
    pub velocity_ms: f64,
}

/// The complete, self-contained record of one run. Flight mode renders
/// exclusively from this — no live sim calls during playback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub design: Design,
    pub summary: SimSummary,
    pub events: Vec<RunEvent>,
    pub samples: Vec<PlaybackSample>,
}

/// Cap playback samples sent over IPC; full fidelity stays in the summary.
const MAX_PLAYBACK_SAMPLES: usize = 2_000;

pub fn run_design(design: &Design) -> Result<RunRecord, String> {
    if design.dry_mass_g <= 0.0 {
        return Err("dry mass must be positive".into());
    }
    if design.diameter_mm <= 0.0 {
        return Err("diameter must be positive".into());
    }
    let motor = find_motor(&design.motor_designation)?;
    let radius_m = design.diameter_mm / 1000.0 / 2.0;
    let rocket = Rocket {
        name: design.name.clone(),
        dry_mass_kg: design.dry_mass_g / 1000.0,
        drag: Some(DragModel {
            cd: design.cd,
            reference_area_m2: std::f64::consts::PI * radius_m * radius_m,
        }),
        recovery: if design.chute.enabled {
            let chute_r = design.chute.diameter_cm / 100.0 / 2.0;
            Some(Recovery {
                chute_cd: design.chute.cd,
                chute_area_m2: std::f64::consts::PI * chute_r * chute_r,
            })
        } else {
            None
        },
    };
    let env = Environment {
        gravity_ms2: 9.80665,
        atmosphere: AtmosphereModel::Standard,
        rail_length_m: design.rail_length_m,
    };
    let config = SimConfig::default();
    let result = simulate_vertical(&rocket, &motor, &env, &config);
    let summary = SimSummary::from_result(&result, &rocket, &motor, &env, &config);

    let stride = result.samples.len().div_ceil(MAX_PLAYBACK_SAMPLES).max(1);
    let mut samples: Vec<PlaybackSample> = result
        .samples
        .iter()
        .step_by(stride)
        .map(|s| PlaybackSample {
            t: s.t,
            altitude_m: s.altitude_m,
            velocity_ms: s.velocity_ms,
        })
        .collect();
    // Always keep the final (landing) sample so playback ends on the ground.
    if let Some(last) = result.samples.last() {
        if samples.last().map(|s| s.t) != Some(last.t) {
            samples.push(PlaybackSample {
                t: last.t,
                altitude_m: last.altitude_m,
                velocity_ms: last.velocity_ms,
            });
        }
    }

    Ok(RunRecord {
        design: design.clone(),
        summary,
        events: result
            .events
            .iter()
            .map(|e| RunEvent {
                kind: format!("{:?}", e.kind),
                t: e.t,
                altitude_m: e.altitude_m,
                velocity_ms: e.velocity_ms,
            })
            .collect(),
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_design_runs_and_matches_headless_summary() {
        let record = run_design(&Design::reference()).unwrap();
        // The reference design must reproduce the Day 1/2 headless numbers —
        // the UI layer must not perturb physics (Day 5 acceptance).
        assert!(record.summary.apogee_m > 350.0 && record.summary.apogee_m < 370.0);
        assert_eq!(record.summary.input_hash.len(), 64);
        assert!(!record.events.is_empty());
        assert!(record.samples.len() <= MAX_PLAYBACK_SAMPLES + 1);
        let last = record.samples.last().unwrap();
        assert!(last.altitude_m.abs() < 1e-6, "playback must end on the ground");
    }

    #[test]
    fn run_is_deterministic_across_calls() {
        let a = run_design(&Design::reference()).unwrap();
        let b = run_design(&Design::reference()).unwrap();
        assert_eq!(a.summary.input_hash, b.summary.input_hash);
        assert_eq!(
            serde_json::to_string(&a.summary).unwrap(),
            serde_json::to_string(&b.summary).unwrap()
        );
    }

    #[test]
    fn invalid_design_is_rejected_not_crashed() {
        let mut d = Design::reference();
        d.dry_mass_g = -5.0;
        assert!(run_design(&d).is_err());
        let mut d2 = Design::reference();
        d2.motor_designation = "Z99".into();
        assert!(run_design(&d2).is_err());
    }

    #[test]
    fn motor_list_contains_c6() {
        let motors = bundled_motors();
        assert!(motors.iter().any(|m| m.designation == "C6"));
    }
}
