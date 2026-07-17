#![cfg(feature = "bridge-rocketpy")]

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

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

#[test]
#[ignore = "requires RocketPy 1.12.1; run with PYTHONPATH=/private/tmp/ascent-rocketpy-fixture"]
fn rocketpy_adapter_reference_input_matches_frozen_contract() {
    let input_path = repository_file("bridges/rocketpy/reference_input.json");
    let output_path = repository_file("bridges/rocketpy/reference_output.json");
    let script_path = repository_file("bridges/rocketpy/run_ascent.py");
    let input = fs::read(&input_path).expect("reference input must be readable");
    let expected: serde_json::Value =
        serde_json::from_slice(&fs::read(&output_path).expect("frozen output must be readable"))
            .expect("frozen output must parse");
    let python = std::env::var_os("ASCENT_ROCKETPY_PYTHON").unwrap_or_else(|| "python3".into());
    let mut child = Command::new(python)
        .arg(script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("fixture Python must start");
    child
        .stdin
        .take()
        .expect("fixture stdin")
        .write_all(&input)
        .expect("fixture input write");
    let output = child
        .wait_with_output()
        .expect("fixture process must finish");
    assert!(
        output.status.success(),
        "adapter stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("adapter output must be JSON");

    assert_eq!(actual["schema_version"], expected["schema_version"]);
    assert_eq!(actual["rocketpy_version"], "1.12.1");
    assert_eq!(actual["rocketpy_version"], expected["rocketpy_version"]);
    let actual_summary = &actual["summary"];
    let expected_summary = &expected["summary"];
    let apogee_delta_m = (actual_summary["apogee_m"].as_f64().expect("actual apogee")
        - expected_summary["apogee_m"]
            .as_f64()
            .expect("expected apogee"))
    .abs();
    assert!(
        apogee_delta_m <= 0.5,
        "apogee drift {apogee_delta_m} m exceeds documented 0.5 m tolerance"
    );
    for field in [
        "apogee_time_s",
        "burnout_time_s",
        "burnout_velocity_ms",
        "max_velocity_ms",
        "rail_exit_velocity_ms",
        "landing_time_s",
        "landing_velocity_ms",
        "events",
        "timestep_s",
        "steps",
    ] {
        assert_eq!(
            actual_summary[field], expected_summary[field],
            "{field} drifted"
        );
    }
}

#[test]
fn cross_validation_docs_name_every_fixed_rocketpy_placeholder() {
    let docs = include_str!("../../../docs/CROSS_VALIDATION.md");
    for required in [
        "chamber_radius=0.01 m",
        "chamber_height=0.07 m",
        "chamber_position=0.035 m",
        "nozzle_radius=0.003 m",
        "dry_inertia=(0.0, 0.0, 0.0) kg m2",
        "Rocket inertia=(1e-6, 1e-6, 1e-6) kg m2",
        "center_of_mass_without_motor=0.0 m",
        "rocket.add_motor(..., position=0.0 m)",
    ] {
        assert!(docs.contains(required), "docs omit {required}");
    }
}
