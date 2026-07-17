#![cfg(feature = "bridge-rocketpy")]

use std::path::PathBuf;

use ascent_domain::Motor;
use ascent_sim::{
    engines, input_hash, AtmosphereModel, DragModel, Environment, Recovery, Rocket, RocketPyEngine,
    SimConfig, SimEngine,
};

const C6_JSON: &str = include_str!("../../ascent-domain/data/motors/estes_c6.json");

fn reference_flight() -> (Rocket, Motor, Environment, SimConfig) {
    let motor = Motor::from_json(C6_JSON).expect("bundled C6 must parse");
    let rocket = Rocket {
        name: "Estes Alpha III".into(),
        dry_mass_kg: 0.034,
        drag: Some(DragModel {
            cd: 0.60,
            reference_area_m2: std::f64::consts::PI * 0.0125 * 0.0125,
        }),
        recovery: Some(Recovery {
            chute_cd: 0.75,
            chute_area_m2: std::f64::consts::PI * 0.15 * 0.15,
        }),
    };
    let env = Environment {
        gravity_ms2: 9.80665,
        atmosphere: AtmosphereModel::Standard,
        rail_length_m: 0.9,
    };
    (rocket, motor, env, SimConfig::default())
}

fn fixture_script(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn repository_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn fixture_response_becomes_a_summary_with_the_native_input_hash() {
    let (rocket, motor, env, config) = reference_flight();
    let engine = RocketPyEngine::with_process(
        "cat",
        repository_file("bridges/rocketpy/reference_output.json"),
    );

    let summary = engine
        .run(&rocket, &motor, &env, &config)
        .expect("fixture response parses");

    assert_eq!(summary.rocket, rocket.name);
    assert_eq!(summary.motor, motor.designation);
    assert_eq!(
        summary.input_hash,
        input_hash(&rocket, &motor, &env, &config)
    );
    assert_eq!(summary.sim_version, "rocketpy-1.12.1");
    // Fixture provenance and the 0.5 m comparison tolerance are documented
    // in docs/CROSS_VALIDATION.md. Exact equality is stronger here because
    // the injected fixture process returns the checked-in frozen JSON.
    assert!((summary.apogee_m - 361.09734484916976).abs() <= 0.5);
    assert_eq!(summary.events.len(), 6);
    assert_eq!(summary.timestep_s, 0.005);
    assert_eq!(summary.steps, 23_116);
}

#[test]
fn missing_python_is_an_explicit_error() {
    let (rocket, motor, env, config) = reference_flight();
    let engine = RocketPyEngine::with_process(
        "/definitely/missing/ascent-rocketpy-python",
        repository_file("bridges/rocketpy/reference_output.json"),
    );

    let error = engine
        .run(&rocket, &motor, &env, &config)
        .expect_err("missing Python must fail");
    assert!(error.contains("failed to start RocketPy bridge"), "{error}");
}

#[test]
fn unsupported_bridge_schema_is_an_explicit_error() {
    let (rocket, motor, env, config) = reference_flight();
    let engine =
        RocketPyEngine::with_process("sh", fixture_script("emit_bad_rocketpy_response.sh"));

    let error = engine
        .run(&rocket, &motor, &env, &config)
        .expect_err("unsupported schema must fail");
    assert!(
        error.contains("unsupported RocketPy response schema"),
        "{error}"
    );
}

#[test]
fn request_excludes_motor_metadata_the_rocketpy_adapter_does_not_use() {
    let (rocket, motor, env, config) = reference_flight();
    let engine =
        RocketPyEngine::with_process("sh", fixture_script("assert_narrow_rocketpy_request.sh"));

    engine
        .run(&rocket, &motor, &env, &config)
        .expect("bridge request must be narrow");
}

#[test]
fn feature_enabled_factory_lists_rocketpy_after_native() {
    let ids: Vec<&str> = engines().iter().map(|engine| engine.id()).collect();
    assert_eq!(ids, ["ascent-native", "rocketpy-bridge"]);
}
