//! Design DTO (what the UI edits) and its mapping onto the sim crates.

use std::sync::{Mutex, OnceLock};

use ascent_domain::{Motor, MotorRegistry};
#[cfg(feature = "bridge-rocketpy")]
use ascent_sim::RocketPyEngine;
use ascent_sim::{
    simulate_vertical, AtmosphereModel, AtmosphereProfile, DragModel, Environment, NativeEngine,
    Recovery, Rocket, SimConfig, SimEngine, SimSummary,
};
use serde::{Deserialize, Serialize};

/// The app's motor catalog for this process: starts from the bundled
/// C6/B6/D12 stock set (Codex task A4) and grows as `.eng` files are
/// imported through the `import_motor_file` IPC command. Later
/// `run_simulation`/`flight_review` calls in the same session can find
/// anything imported earlier.
fn session_registry() -> &'static Mutex<MotorRegistry> {
    static REGISTRY: OnceLock<Mutex<MotorRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(MotorRegistry::bundled()))
}

/// Lock the session registry, recovering from poisoning. All mutations go
/// through `register_eng`, which validates every motor before inserting any,
/// so a panic elsewhere while the lock was held cannot leave the map in a
/// half-written state — recovering beats bricking every IPC command for the
/// rest of the process.
fn lock_registry() -> std::sync::MutexGuard<'static, MotorRegistry> {
    session_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChuteSpec {
    pub enabled: bool,
    pub diameter_cm: f64,
    pub cd: f64,
    /// Dual-deploy: the main opens descending through this altitude (m
    /// AGL) instead of at apogee. `None` = single-deploy (main at apogee),
    /// the pre-v0.5 behavior. Skipped when absent so existing designs
    /// serialize byte-identically and their study hashes never drift.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_deploy_altitude_m: Option<f64>,
    /// Drogue canopy diameter (cm) for dual-deploy; the drogue opens at
    /// apogee and rides down with the main. `None` = no drogue (free
    /// ballistic descent to the main-deploy altitude).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drogue_diameter_cm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drogue_cd: Option<f64>,
}

/// Everything the inspector can edit, in UI-friendly units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
                main_deploy_altitude_m: None,
                drogue_diameter_cm: None,
                drogue_cd: None,
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

fn motor_info(m: &Motor) -> MotorInfo {
    MotorInfo {
        designation: m.designation.clone(),
        manufacturer: m.manufacturer.clone(),
        total_impulse_ns: m.total_impulse(),
        burn_time_s: m.burn_time(),
        total_mass_g: m.total_mass_kg * 1000.0,
    }
}

pub fn bundled_motors() -> Vec<MotorInfo> {
    let registry = lock_registry();
    registry.list().into_iter().map(motor_info).collect()
}

/// All motors currently known to the session registry (bundled plus any
/// imported this session), cloned out for callers like `review_ipc` that
/// need to search by designation.
pub(crate) fn session_motors() -> Vec<Motor> {
    let registry = lock_registry();
    registry.list().into_iter().cloned().collect()
}

pub(crate) fn find_motor(designation: &str) -> Result<Motor, String> {
    let registry = lock_registry();
    registry
        .get(designation)
        .cloned()
        .ok_or_else(|| format!("unknown motor: {designation}"))
}

/// Result of importing a `.eng` file over IPC: the lead motor plus any
/// designations the import overwrote (bundled or previously imported), so
/// the UI can warn instead of silently swapping a reference motor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedMotor {
    pub motor: MotorInfo,
    pub replaced: Vec<String>,
}

/// Parse and register a RASP `.eng` file's motor(s) into the session
/// registry, returning info for the first motor parsed. Subsequent
/// `run_simulation`/`flight_review` calls can reference it by designation.
pub fn import_motor_file(contents: &str) -> Result<ImportedMotor, String> {
    let provenance = serde_json::json!({
        "source": "user-imported RASP .eng file",
        "format": "RASP thrust-curve text",
    });
    let mut registry = lock_registry();
    let outcome = registry.register_eng(contents, provenance)?;
    let motor = registry
        .get(&outcome.first_designation)
        .expect("just-registered motor must be present");
    Ok(ImportedMotor {
        motor: motor_info(motor),
        replaced: outcome.replaced,
    })
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

/// A cross-validation result. `native` is always present; `rocketpy` is an
/// explicit comparison entry and can be unavailable without invalidating the
/// native run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadResult {
    pub native: SimSummary,
    pub rocketpy: RocketPySpread,
    pub apogee_spread_m: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocketPySpread {
    pub available: bool,
    pub engine_id: String,
    pub engine_version: String,
    pub summary: Option<SimSummary>,
    pub reason: Option<String>,
}

/// Cap playback samples sent over IPC; full fidelity stays in the summary.
const MAX_PLAYBACK_SAMPLES: usize = 2_000;

/// Validate a Design and map it onto the sim-layer types. Shared by the
/// run command and the evidence command so they can never disagree.
pub fn build_flight(design: &Design) -> Result<(Rocket, Motor, Environment), String> {
    if design.dry_mass_g <= 0.0 {
        return Err("dry mass must be positive".into());
    }
    if design.diameter_mm <= 0.0 {
        return Err("diameter must be positive".into());
    }
    if design.chute.enabled {
        if let Some(deploy_m) = design.chute.main_deploy_altitude_m {
            if !deploy_m.is_finite() || deploy_m < 0.0 {
                return Err("main deploy altitude must be finite and non-negative".into());
            }
        }
        match (design.chute.drogue_diameter_cm, design.chute.drogue_cd) {
            (None, None) => {}
            (Some(diameter_cm), Some(cd)) => {
                if design.chute.main_deploy_altitude_m.is_none() {
                    return Err("a drogue requires a main deploy altitude".into());
                }
                if !diameter_cm.is_finite() || diameter_cm <= 0.0 {
                    return Err("drogue diameter must be finite and positive".into());
                }
                if !cd.is_finite() || cd <= 0.0 {
                    return Err("drogue Cd must be finite and positive".into());
                }
            }
            _ => return Err("drogue diameter and Cd must be set together".into()),
        }
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
            // Drogue rides on the dual-deploy path only: it needs both a
            // diameter and a Cd, and it's meaningless without a main-deploy
            // altitude (otherwise the main is already out at apogee).
            let drogue = match (
                design.chute.main_deploy_altitude_m,
                design.chute.drogue_diameter_cm,
                design.chute.drogue_cd,
            ) {
                (Some(_), Some(d_cm), Some(cd)) => {
                    let r = d_cm / 100.0 / 2.0;
                    Some(ascent_sim::Drogue {
                        cd,
                        area_m2: std::f64::consts::PI * r * r,
                    })
                }
                _ => None,
            };
            Some(Recovery {
                chute_cd: design.chute.cd,
                chute_area_m2: std::f64::consts::PI * chute_r * chute_r,
                drogue,
                main_deploy_altitude_m: design.chute.main_deploy_altitude_m,
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
    Ok((rocket, motor, env))
}

pub fn run_design(design: &Design) -> Result<RunRecord, String> {
    run_design_with_atmosphere(design, None)
}

pub fn run_design_with_atmosphere(
    design: &Design,
    atmosphere: Option<&AtmosphereProfile>,
) -> Result<RunRecord, String> {
    let (rocket, motor, mut env) = build_flight(design)?;
    if let Some(model) = atmosphere.and_then(AtmosphereProfile::atmosphere_model) {
        env.atmosphere = model;
    }
    let config = SimConfig::default();
    // The recorded summary comes through the SimEngine seam — the same path
    // future bridge engines use — while playback samples/events come from the
    // raw solver result. NativeEngine::run is simulate_vertical + from_result,
    // so the two stay byte-identical (proven in ascent-sim's engine tests).
    let engine: &dyn SimEngine = &NativeEngine;
    let summary = engine.run(&rocket, &motor, &env, &config)?;
    let result = simulate_vertical(&rocket, &motor, &env, &config);

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

/// Run native first, then optionally ask the developer-gated RocketPy bridge
/// for a side-by-side comparison. The comparison never substitutes for or
/// averages into native output.
pub fn run_spread(design: &Design) -> Result<SpreadResult, String> {
    let (rocket, motor, env) = build_flight(design)?;
    let config = SimConfig::default();
    let native = NativeEngine.run(&rocket, &motor, &env, &config)?;

    #[cfg(feature = "bridge-rocketpy")]
    let rocketpy = {
        let engine = RocketPyEngine::from_environment();
        match engine.run(&rocket, &motor, &env, &config) {
            Ok(summary) => RocketPySpread {
                available: true,
                engine_id: engine.id().into(),
                engine_version: summary.sim_version.clone(),
                summary: Some(summary),
                reason: None,
            },
            Err(reason) => RocketPySpread {
                available: false,
                engine_id: engine.id().into(),
                engine_version: engine.version().into(),
                summary: None,
                reason: Some(reason),
            },
        }
    };
    #[cfg(not(feature = "bridge-rocketpy"))]
    let rocketpy = RocketPySpread {
        available: false,
        engine_id: "rocketpy-bridge".into(),
        engine_version: "not-compiled".into(),
        summary: None,
        reason: Some("RocketPy bridge is not compiled in this build".into()),
    };
    let apogee_spread_m = rocketpy
        .summary
        .as_ref()
        .map(|summary| (native.apogee_m - summary.apogee_m).abs());
    Ok(SpreadResult {
        native,
        rocketpy,
        apogee_spread_m,
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
        assert!(
            last.altitude_m.abs() < 1e-6,
            "playback must end on the ground"
        );
    }

    #[test]
    fn imported_density_profile_changes_the_primary_run_and_its_hash() {
        let standard = run_design(&Design::reference()).unwrap();
        let profile = ascent_sim::AtmosphereProfile {
            name: "thin-air-test".into(),
            layers: vec![
                ascent_sim::ProfileLayer {
                    altitude_m: 0.0,
                    wind_speed_ms: 0.0,
                    wind_direction_deg: 0.0,
                    density_kg_m3: Some(0.2),
                },
                ascent_sim::ProfileLayer {
                    altitude_m: 2_000.0,
                    wind_speed_ms: 0.0,
                    wind_direction_deg: 0.0,
                    density_kg_m3: Some(0.2),
                },
            ],
        };

        let profiled = run_design_with_atmosphere(&Design::reference(), Some(&profile)).unwrap();

        assert!(profiled.summary.apogee_m > standard.summary.apogee_m);
        assert_ne!(profiled.summary.input_hash, standard.summary.input_hash);
    }

    #[test]
    fn single_deploy_chute_serializes_without_the_v05_keys() {
        // A pre-v0.5-shaped design must round-trip byte-identically, so its
        // study-input hash never drifts when v0.5 lands.
        let json = serde_json::to_string(&Design::reference().chute).unwrap();
        assert!(!json.contains("main_deploy_altitude_m"));
        assert!(!json.contains("drogue_diameter_cm"));
        assert!(!json.contains("drogue_cd"));
        let old = r#"{"enabled":true,"diameter_cm":30.0,"cd":0.75}"#;
        let spec: ChuteSpec = serde_json::from_str(old).unwrap();
        assert_eq!(spec, Design::reference().chute);
    }

    #[test]
    fn dual_deploy_design_builds_drogue_and_main_deploy_altitude() {
        let mut design = Design::reference();
        design.chute.main_deploy_altitude_m = Some(75.0);
        design.chute.drogue_diameter_cm = Some(8.0);
        design.chute.drogue_cd = Some(0.8);
        let (rocket, _, _) = build_flight(&design).unwrap();
        let recovery = rocket.recovery.expect("chute enabled");
        assert_eq!(recovery.main_deploy_altitude_m, Some(75.0));
        let drogue = recovery.drogue.expect("drogue configured");
        let expected_area = std::f64::consts::PI * 0.04 * 0.04;
        assert!((drogue.area_m2 - expected_area).abs() < 1e-12);
        assert_eq!(drogue.cd, 0.8);
    }

    #[test]
    fn main_deploy_altitude_without_drogue_is_ballistic_then_main() {
        // A main-deploy altitude but no drogue: free descent to the deploy
        // altitude, then the main. build_flight must not fabricate a drogue.
        let mut design = Design::reference();
        design.chute.main_deploy_altitude_m = Some(100.0);
        let (rocket, _, _) = build_flight(&design).unwrap();
        let recovery = rocket.recovery.unwrap();
        assert_eq!(recovery.main_deploy_altitude_m, Some(100.0));
        assert!(recovery.drogue.is_none());
    }

    #[test]
    fn invalid_dual_deploy_configuration_is_rejected_at_the_design_boundary() {
        let mut partial = Design::reference();
        partial.chute.main_deploy_altitude_m = Some(60.0);
        partial.chute.drogue_diameter_cm = Some(8.0);
        assert!(build_flight(&partial)
            .unwrap_err()
            .contains("drogue diameter and Cd must be set together"));

        let mut negative_altitude = Design::reference();
        negative_altitude.chute.main_deploy_altitude_m = Some(-1.0);
        assert!(build_flight(&negative_altitude)
            .unwrap_err()
            .contains("main deploy altitude"));

        let mut drogue_without_dual = Design::reference();
        drogue_without_dual.chute.drogue_diameter_cm = Some(8.0);
        drogue_without_dual.chute.drogue_cd = Some(0.8);
        assert!(build_flight(&drogue_without_dual)
            .unwrap_err()
            .contains("requires a main deploy altitude"));
    }

    #[test]
    fn dual_deploy_lands_faster_than_single_deploy() {
        let single = run_design(&Design::reference()).unwrap();
        let mut design = Design::reference();
        design.chute.main_deploy_altitude_m = Some(60.0);
        design.chute.drogue_diameter_cm = Some(8.0);
        design.chute.drogue_cd = Some(0.8);
        let dual = run_design(&design).unwrap();
        assert!(
            dual.summary.landing_time_s < single.summary.landing_time_s,
            "dual-deploy {} s must land sooner than single {} s",
            dual.summary.landing_time_s,
            single.summary.landing_time_s
        );
        assert!(dual.events.iter().any(|e| e.kind == "MainDeploy"));
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
    #[cfg(not(feature = "bridge-rocketpy"))]
    fn spread_keeps_the_native_result_when_rocketpy_is_not_compiled() {
        let spread = run_spread(&Design::reference()).expect("native spread result");

        assert!(spread.native.apogee_m > 350.0);
        assert!(!spread.rocketpy.available);
        assert!(spread.rocketpy.summary.is_none());
        assert!(spread
            .rocketpy
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("not compiled"));
        assert!(spread.apogee_spread_m.is_none());
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
    fn motor_list_contains_all_bundled_motors() {
        let motors = bundled_motors();
        for designation in ["C6", "B6", "D12"] {
            assert!(
                motors.iter().any(|m| m.designation == designation),
                "missing bundled motor {designation}"
            );
        }
        // No exact-length assertion: the session registry is process-global,
        // so other tests in this binary may have imported motors already.
    }

    #[test]
    fn imported_eng_motor_flies_end_to_end_with_provenance() {
        // The full IPC path: import a .eng file, fly it, audit the evidence.
        let d10 = include_str!("../../../data/eng-samples/openrocket_d10.eng");
        let imported = import_motor_file(d10).unwrap();
        assert_eq!(imported.motor.designation, "D10");
        assert!(imported.motor.total_impulse_ns > 0.0);
        assert!(
            imported.replaced.is_empty(),
            "D10 is not bundled, so nothing should be replaced"
        );

        let mut design = Design::reference();
        design.motor_designation = "D10".into();
        let record = run_design(&design).unwrap();
        assert!(
            record.summary.apogee_m > 50.0,
            "imported D10 should lift off"
        );
        assert_eq!(record.summary.input_hash.len(), 64);

        let ev = crate::evidence::evidence_for(&design).unwrap();
        assert_eq!(ev.motor.designation, "D10");
        let prov = serde_json::to_string(&ev.motor.provenance).unwrap();
        assert!(
            prov.contains("user-imported"),
            "evidence must carry the import provenance, got: {prov}"
        );
    }

    #[test]
    fn every_bundled_motor_flies_the_reference_airframe() {
        for designation in ["B6", "C6", "D12"] {
            let mut d = Design::reference();
            d.motor_designation = designation.into();
            let record = run_design(&d).unwrap();
            assert!(
                record.summary.apogee_m > 50.0,
                "{designation} should lift off"
            );
        }
    }
}
