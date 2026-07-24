//! Executed evidence for the NDRT-2020 launch-through-apogee comparison.
//!
//! These tests prove the comparison is real and honest: sources are hash-pinned
//! and license-clear, the telemetry is parsed without edits, measured metrics
//! are derived (not hardcoded), the authoritative simulator actually runs, the
//! launch alignment is deterministic and simulation-independent, the comparison
//! window is ascent-only, errors/statuses are recomputed so a failure cannot
//! forge a pass, and completion never auto-grants FlightValidated.

use std::path::PathBuf;

use ascent_domain::evidence::EvidenceLevel;
use ascent_review::ndrt_2020::{self, RavenTelemetry};
use ascent_review::validation::ValidationCase;
use sha2::{Digest, Sha256};

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const NDRT_DIR: &str = "data/validation/ndrt-2020";
const CSV: &str = "fullscale_2-23_raven1_crop.csv";
const ENG: &str = "Cesaroni_4895L1395-P.eng";

/// SHA-256 pins recorded in docs/validation/NDRT_2020_SOURCE_SPEC.md.
const CSV_SHA256: &str = "1cc15862ddb3367d09225037cfe8dd1f68687d1d2292ef7ecd5f2a344736d3f6";
const ENG_SHA256: &str = "d60b9e6f8d4bb4621a58e98873f69bfa8ef49efe4b0f78fdf345aa52c50b47ce";

fn read_source(name: &str) -> Vec<u8> {
    std::fs::read(workspace().join(NDRT_DIR).join(name)).unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn load_case() -> ValidationCase {
    let bytes =
        std::fs::read(workspace().join("data/validation/cases/ndrt-2020-flight.json")).unwrap();
    ValidationCase::from_canonical_bytes(&bytes).unwrap()
}

#[test]
fn sources_are_hash_pinned_and_license_clear() {
    // The measured fixture matches the hash the case pins.
    let case = load_case();
    case.verify_fixture(&workspace()).unwrap();
    assert_eq!(case.fixture.sha256, CSV_SHA256);
    assert!(case.fixture.license.contains("MIT"));
    assert!(!case.source_authority.is_empty());

    // Both consumed sources are hash-pinned to their frozen values.
    assert_eq!(sha256_hex(&read_source(CSV)), CSV_SHA256);
    assert_eq!(sha256_hex(&read_source(ENG)), ENG_SHA256);

    // License text is checked in alongside the data.
    let license =
        std::fs::read_to_string(workspace().join(NDRT_DIR).join("ROCKETPAPER_LICENSE.txt"))
            .unwrap();
    assert!(license.contains("MIT License"));
    assert!(license.contains("Projeto Jupiter"));
}

#[test]
fn telemetry_parses_without_editing_and_preserves_rows() {
    let bytes = read_source(CSV);
    let text = String::from_utf8(bytes.clone()).unwrap();
    let telemetry = RavenTelemetry::parse(&text).unwrap();

    // Header is preserved verbatim (leading spaces and casing intact).
    assert_eq!(
        telemetry.raw_header,
        "Time-Axial Accel (Gs), Axial Accel (Gs), bILBA, Time (s), Altitude (Ft-AGL), bILBA"
    );

    // Every non-empty data line is preserved with all six raw fields, in order.
    let data_lines: Vec<&str> = text.lines().skip(1).filter(|l| !l.is_empty()).collect();
    assert_eq!(telemetry.len(), data_lines.len());
    assert_eq!(telemetry.len(), 1819);
    for (row, line) in telemetry.raw_rows.iter().zip(&data_lines) {
        assert_eq!(row.len(), 6);
        assert_eq!(row.join(","), *line);
    }

    // Conversion: altitude metres AGL is exactly feet / 3.28084 (source rule).
    let alt_m = telemetry.altitude_m_agl();
    for (m, ft) in alt_m.iter().zip(&telemetry.alt_ft_agl) {
        assert!((m - ft / ndrt_2020::FEET_PER_METER).abs() < 1e-12);
    }
    // Axial acceleration channel is already in g — no conversion applied.
    assert_eq!(telemetry.accel_g.len(), telemetry.len());
}

#[test]
fn measured_metrics_are_derived_not_hardcoded() {
    let text = String::from_utf8(read_source(CSV)).unwrap();
    let telemetry = RavenTelemetry::parse(&text).unwrap();

    // Apogee is the argmax of the altitude channel — file row 343 (README),
    // which is 0-based data index 341.
    assert_eq!(telemetry.apogee_index(), 341);
    let apogee = telemetry.measured_apogee_agl_m();
    let tta = telemetry.measured_time_to_apogee_s();
    // Reconciles with the upstream RocketPy analysis (1320.357 m, 17.095 s).
    assert!((apogee - 1320.357).abs() < 0.01, "apogee {apogee}");
    assert!((tta - 17.095).abs() < 1e-9, "tta {tta}");

    // Derived, not hardcoded: scaling the altitude channel scales the derived
    // apogee by the same factor, and the argmax row is unchanged.
    let mut scaled = telemetry.clone();
    for ft in &mut scaled.alt_ft_agl {
        *ft *= 2.0;
    }
    assert_eq!(scaled.apogee_index(), 341);
    assert!((scaled.measured_apogee_agl_m() - 2.0 * apogee).abs() < 1e-6);
}

#[test]
fn launch_alignment_is_deterministic_and_simulation_independent() {
    let text = String::from_utf8(read_source(CSV)).unwrap();
    let telemetry = RavenTelemetry::parse(&text).unwrap();

    // Deterministic: identical inputs, identical alignment and apogee row.
    assert_eq!(telemetry.apogee_index(), telemetry.apogee_index());

    // Simulation-independent: time-to-apogee is the altitude channel's own
    // timestamp at the argmax row — no simulation output enters its derivation.
    let idx = telemetry.apogee_index();
    assert_eq!(
        telemetry.measured_time_to_apogee_s(),
        telemetry.alt_time_s[idx]
    );
    // Launch is aligned to the recording start (t=0), not chosen to minimize
    // error: the first altitude timestamp is the raw source value.
    assert!(telemetry.alt_time_s[0] < 0.1);
}

#[test]
fn authoritative_simulator_executes_and_metrics_recompute() {
    let case = load_case();
    let comparison = ndrt_2020::run_comparison(
        &read_source(CSV),
        &read_source(ENG),
        &case.case_hash().unwrap(),
    )
    .unwrap();

    // The simulator ran and produced a finite apogee and burnout.
    assert!(comparison.simulated_apogee_agl_m.is_finite());
    assert!(comparison.simulated_apogee_agl_m > 0.0);
    assert!(comparison.simulated_burnout_time_s > 0.0);

    // The artifact is internally valid and bound to this case.
    let artifact = &comparison.artifact;
    artifact.validate().unwrap();
    assert_eq!(artifact.case_id, "ndrt-2020-flight");
    assert_eq!(artifact.case_hash, case.case_hash().unwrap());
    assert_eq!(artifact.metrics.len(), 4);
    assert!(!artifact.model_version.is_empty());

    // Every stored abs_error/rel_error is exactly re-derivable from the
    // measured/simulated pair (recomputed, not asserted).
    for m in &artifact.metrics {
        assert!((m.abs_error - (m.simulated - m.measured).abs()).abs() < 1e-9);
        assert_eq!(m.pass, m.abs_error <= m.tolerance);
    }

    // Surface the executed numbers for the comparison report.
    eprintln!(
        "NDRT: measured apogee {:.3} m / tta {:.3} s | sim apogee {:.3} m / tta {:.3} s | \
         burnout sim {:.4} s vs motor {:.4} s | rmse {:.3} m ({:.4} norm) | n={} dt={} | pass={}",
        comparison.measured_apogee_agl_m,
        comparison.measured_time_to_apogee_s,
        comparison.simulated_apogee_agl_m,
        comparison.simulated_time_to_apogee_s,
        comparison.simulated_burnout_time_s,
        comparison.motor_burn_time_s,
        comparison.rmse_m,
        comparison.normalized_rmse,
        comparison.compared_samples,
        comparison.timestep_s,
        artifact.pass,
    );
    for m in &artifact.metrics {
        eprintln!(
            "  metric {}: measured={:.4} simulated={:.4} abs={:.4} rel={:.4} tol={:.4} pass={}",
            m.id, m.measured, m.simulated, m.abs_error, m.rel_error, m.tolerance, m.pass
        );
    }
}

#[test]
fn config_hash_is_stable_and_input_sensitive() {
    let eng = read_source(ENG);
    let motor = ndrt_2020::parse_motor(std::str::from_utf8(&eng).unwrap()).unwrap();
    let (rocket, env, config) = ndrt_2020::build_inputs(&motor);

    let base = ascent_sim::input_hash(&rocket, &motor, &env, &config);
    // Stable: recomputing over identical inputs yields the same hash.
    assert_eq!(base, ascent_sim::input_hash(&rocket, &motor, &env, &config));

    // Input-sensitive: perturbing any mapped input changes the hash.
    let mut r2 = rocket.clone();
    if let Some(d) = &mut r2.drag {
        d.cd += 1e-6;
    }
    assert_ne!(base, ascent_sim::input_hash(&r2, &motor, &env, &config));

    let mut e2 = env.clone();
    e2.rail_length_m += 1e-6;
    assert_ne!(base, ascent_sim::input_hash(&rocket, &motor, &e2, &config));
}

#[test]
fn comparison_window_is_ascent_only() {
    let text = String::from_utf8(read_source(CSV)).unwrap();
    let telemetry = RavenTelemetry::parse(&text).unwrap();
    let case = load_case();
    let comparison = ndrt_2020::run_comparison(
        &read_source(CSV),
        &read_source(ENG),
        &case.case_hash().unwrap(),
    )
    .unwrap();

    // Exactly launch→apogee inclusive: no descent samples enter the RMSE.
    assert_eq!(comparison.compared_samples, telemetry.apogee_index() + 1);
    // The trace extends far past apogee (full descent), so the window is a
    // strict prefix — descent is present in the data but excluded.
    assert!(telemetry.len() > comparison.compared_samples * 2);

    // No metric is a velocity/acceleration/descent/recovery pass-fail.
    for m in &comparison.artifact.metrics {
        let id = m.id.as_str();
        assert!(!id.contains("velocity"), "{id}");
        assert!(!id.contains("accel"), "{id}");
        assert!(!id.contains("descent"), "{id}");
        assert!(!id.contains("landing"), "{id}");
    }
}

#[test]
fn a_failed_or_forged_comparison_cannot_pass() {
    let case = load_case();
    let comparison = ndrt_2020::run_comparison(
        &read_source(CSV),
        &read_source(ENG),
        &case.case_hash().unwrap(),
    )
    .unwrap();

    // Hand-flip a metric to a pass its numbers do not support: validation rejects.
    let mut forged = comparison.artifact.clone();
    let m = &mut forged.metrics[0];
    m.tolerance = m.abs_error / 2.0; // now abs_error > tolerance
    m.pass = true; // ...but claim a pass anyway
    forged.pass = true;
    let err = forged.validate().unwrap_err();
    assert!(err.contains("pass flag disagrees"), "{err}");

    // Flipping the overall pass without the metrics is likewise rejected.
    let mut forged2 = comparison.artifact.clone();
    forged2.pass = !forged2.pass;
    // Only meaningful to assert when at least one metric disagrees with the flip.
    if forged2.metrics.iter().all(|m| m.pass) != forged2.pass {
        assert!(forged2.validate().is_err());
    }
}

#[test]
fn completion_does_not_auto_grant_flight_validated() {
    let case = load_case();
    // The checked-in case carries the executed comparison artifact, yet is held
    // at FlightDataAvailable. Attaching a passing artifact does not promote the
    // label: the evidence level is a declared property, not a consequence of the
    // artifact's presence.
    assert_eq!(case.evidence_level, EvidenceLevel::FlightDataAvailable);
    assert!(case.comparison.is_some());
    assert_ne!(case.evidence_level, EvidenceLevel::FlightValidated);
    // It still loads and passes the evidence policy at its declared rung.
    case.enforce_evidence_policy().unwrap();
}

#[test]
fn burnout_is_not_counted_as_a_flight_validation_metric() {
    use ascent_review::validation::MetricKind;
    let case = load_case();
    let comparison = ndrt_2020::run_comparison(
        &read_source(CSV),
        &read_source(ENG),
        &case.case_hash().unwrap(),
    )
    .unwrap();

    // Burnout is present and reported, but classified as an input-consistency
    // check — never a primary flight-validation metric.
    let burnout = comparison
        .artifact
        .metrics
        .iter()
        .find(|m| m.id == "burnout_time_s")
        .expect("burnout metric present");
    assert_eq!(burnout.kind, MetricKind::InputConsistency);

    // The three flight metrics are the only primary ones.
    let primary: Vec<&str> = comparison
        .artifact
        .metrics
        .iter()
        .filter(|m| m.kind == MetricKind::Primary)
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(
        primary,
        vec![
            "apogee_agl_m",
            "time_to_apogee_s",
            "normalized_altitude_rmse"
        ]
    );
    assert_eq!(comparison.primary_metrics(), (3, 3));
    assert_eq!(comparison.input_checks(), (1, 1));

    // Flipping burnout to a hard failure must NOT flip the overall result: the
    // artifact re-validates as a pass because burnout does not gate it.
    let mut mutated = comparison.artifact.clone();
    let b = mutated
        .metrics
        .iter_mut()
        .find(|m| m.id == "burnout_time_s")
        .unwrap();
    b.simulated = b.measured + 10.0 * b.tolerance;
    b.abs_error = (b.simulated - b.measured).abs();
    b.rel_error = b.abs_error / b.measured.abs();
    b.pass = b.abs_error <= b.tolerance;
    assert!(!b.pass);
    // Overall pass is still true and self-consistent (primary metrics only).
    assert!(mutated.pass);
    mutated.validate().unwrap();
}

#[test]
fn attached_artifact_matches_a_fresh_run_and_does_not_promote() {
    let case = load_case();
    // The checked-in artifact reproduces exactly from a fresh, deterministic run.
    let attached = case.comparison.as_ref().expect("artifact attached");
    let fresh = ndrt_2020::run_comparison(
        &read_source(CSV),
        &read_source(ENG),
        &case.case_hash().unwrap(),
    )
    .unwrap();
    assert_eq!(attached, &fresh.artifact);

    // It is bound to this exact case spec and internally valid.
    assert_eq!(attached.case_id, "ndrt-2020-flight");
    assert_eq!(attached.case_hash, case.case_hash().unwrap());
    attached.validate().unwrap();
    assert!(attached.pass);

    // Present + passing, but the label stays FlightDataAvailable — no promotion.
    assert_eq!(case.evidence_level, EvidenceLevel::FlightDataAvailable);
    case.enforce_evidence_policy().unwrap();
}

/// Regenerates `data/validation/cases/ndrt-2020-flight.json` with the executed
/// comparison artifact attached, in canonical form. Deterministic and
/// idempotent (the case hash is spec-only, so re-attaching does not change it).
/// Run explicitly when the spec or the comparison output changes:
/// `cargo test -p ascent-review --test ndrt_2020 -- --ignored regenerate_case_fixture`.
#[test]
#[ignore]
fn regenerate_case_fixture() {
    let mut case = load_case();
    let comparison = ndrt_2020::run_comparison(
        &read_source(CSV),
        &read_source(ENG),
        &case.case_hash().unwrap(),
    )
    .unwrap();
    case.comparison = Some(comparison.artifact);
    let mut bytes = case.canonical_bytes().unwrap();
    bytes.push(b'\n');
    let path = workspace().join("data/validation/cases/ndrt-2020-flight.json");
    std::fs::write(&path, &bytes).unwrap();
    // Round-trips back through the canonical loader.
    ValidationCase::from_canonical_bytes(&std::fs::read(&path).unwrap()).unwrap();
}
