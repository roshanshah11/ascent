//! Evidence for the one-command NDRT 2020 comparison workflow.
//!
//! These tests prove the exported directory is real evidence, not a rendering:
//! the authoritative simulator actually runs, every consumed source is verified
//! against its pin (and a corrupted source fails), the recomputed artifact is
//! cross-checked against the canonical attached one (and a mismatch fails), the
//! output is deterministic apart from the single allowed timestamp, the exported
//! JSON re-validates, the Markdown numbers come from the artifact, primary
//! metrics stay separated from input-consistency checks, and the evidence label
//! and credibility caveats appear without ever being promoted.

use std::collections::BTreeMap;

use ascent_domain::evidence::EvidenceLevel;
use ascent_review::ndrt_2020;
use ascent_review::ndrt_2020_export::{
    self as export, ExportManifest, FlightComparisonExport, ARTIFACT_FILE, CASE_FILE,
    MANIFEST_FILE, SUMMARY_FILE,
};
use ascent_review::validation::{ComparisonArtifact, MetricKind, ValidationCase};
use sha2::{Digest, Sha256};

const GENERATED_AT: u64 = 1_700_000_000;

fn run() -> FlightComparisonExport {
    export::run_canonical_export(GENERATED_AT).expect("canonical export succeeds")
}

fn text(exported: &FlightComparisonExport, name: &str) -> String {
    String::from_utf8(exported.file(name).expect("file present").bytes.clone()).unwrap()
}

fn json(exported: &FlightComparisonExport, name: &str) -> serde_json::Value {
    serde_json::from_slice(&exported.file(name).expect("file present").bytes).unwrap()
}

fn canonical_case() -> ValidationCase {
    ValidationCase::from_canonical_bytes(export::CANONICAL_CASE_JSON).unwrap()
}

/// Re-canonicalize a case whose attached artifact has been tampered with, so it
/// still loads cleanly and the *only* thing wrong is the artifact mismatch.
fn case_bytes_with_artifact(artifact: ComparisonArtifact) -> Vec<u8> {
    let mut case = canonical_case();
    case.comparison = Some(artifact);
    case.canonical_bytes().unwrap()
}

#[test]
fn export_runs_the_authoritative_simulator() {
    let exported = run();

    // The model identity in the export is the authoritative vertical engine, and
    // it is the same identity ndrt_2020 stamps — not a re-labelled second path.
    assert_eq!(exported.manifest.model_version, ndrt_2020::MODEL_VERSION);
    assert!(exported
        .manifest
        .model_version
        .contains("simulate_vertical"));

    // The simulator actually produced numbers.
    assert!(exported.comparison.simulated_apogee_agl_m.is_finite());
    assert!(exported.comparison.simulated_apogee_agl_m > 0.0);
    assert!(exported.comparison.simulated_time_to_apogee_s > 0.0);
    assert!(exported.comparison.compared_samples > 0);

    // And the exported artifact is exactly what a direct harness run produces:
    // the command re-executes the harness rather than replaying a stored result.
    let direct = ndrt_2020::run_comparison(
        export::CANONICAL_TELEMETRY_CSV,
        export::CANONICAL_MOTOR_ENG,
        &canonical_case().case_hash().unwrap(),
    )
    .unwrap();
    assert_eq!(exported.comparison.artifact, direct.artifact);

    // The config/input hash is the simulator's own canonical input hash.
    let motor =
        ndrt_2020::parse_motor(std::str::from_utf8(export::CANONICAL_MOTOR_ENG).unwrap()).unwrap();
    let (rocket, env, config) = ndrt_2020::build_inputs(&motor);
    assert_eq!(
        exported.manifest.input_hash,
        ascent_sim::input_hash(&rocket, &motor, &env, &config)
    );
}

#[test]
fn source_integrity_failure_is_an_error() {
    // Corrupted telemetry: one altered byte breaks the case's pinned fixture hash.
    let mut csv = export::CANONICAL_TELEMETRY_CSV.to_vec();
    let last = csv.len() - 1;
    csv[last] = if csv[last] == b'0' { b'1' } else { b'0' };
    let err = export::run_export(
        export::CANONICAL_CASE_JSON,
        &csv,
        export::CANONICAL_MOTOR_ENG,
        GENERATED_AT,
    )
    .unwrap_err();
    assert!(err.contains("source integrity failure"), "{err}");
    assert!(err.contains("hash mismatch"), "{err}");

    // Corrupted motor source: caught by its own pin.
    let mut eng = export::CANONICAL_MOTOR_ENG.to_vec();
    eng.extend_from_slice(b"\n");
    let err = export::run_export(
        export::CANONICAL_CASE_JSON,
        export::CANONICAL_TELEMETRY_CSV,
        &eng,
        GENERATED_AT,
    )
    .unwrap_err();
    assert!(err.contains("source integrity failure"), "{err}");
    assert!(err.contains(export::MOTOR_ENG_PATH), "{err}");
}

#[test]
fn artifact_mismatch_is_an_error_and_names_the_difference() {
    let mut attached = canonical_case().comparison.unwrap();
    attached.model_version = "some-other-engine/9.9.9".into();
    let err = export::run_export(
        &case_bytes_with_artifact(attached),
        export::CANONICAL_TELEMETRY_CSV,
        export::CANONICAL_MOTOR_ENG,
        GENERATED_AT,
    )
    .unwrap_err();
    assert!(err.contains("artifact validation failure"), "{err}");
    assert!(err.contains("model_version"), "{err}");
    assert!(err.contains("some-other-engine/9.9.9"), "{err}");

    // A tampered metric is likewise caught, and the metric is named.
    let mut attached = canonical_case().comparison.unwrap();
    let metric = &mut attached.metrics[0];
    metric.simulated = metric.measured; // internally consistent, but not what we compute
    metric.abs_error = 0.0;
    metric.rel_error = 0.0;
    metric.pass = true;
    let err = export::run_export(
        &case_bytes_with_artifact(attached),
        export::CANONICAL_TELEMETRY_CSV,
        export::CANONICAL_MOTOR_ENG,
        GENERATED_AT,
    )
    .unwrap_err();
    assert!(err.contains("artifact validation failure"), "{err}");
    assert!(err.contains("apogee_agl_m"), "{err}");

    // A case with no attached artifact at all cannot be silently exported.
    let mut case = canonical_case();
    case.comparison = None;
    let err = export::run_export(
        &case.canonical_bytes().unwrap(),
        export::CANONICAL_TELEMETRY_CSV,
        export::CANONICAL_MOTOR_ENG,
        GENERATED_AT,
    )
    .unwrap_err();
    assert!(err.contains("no attached comparison artifact"), "{err}");
}

#[test]
fn output_is_deterministic_apart_from_the_allowed_timestamp() {
    let a = run();
    let b = run();
    // Identical timestamp ⇒ byte-identical everywhere.
    assert_eq!(a.files, b.files);
    assert_eq!(a.terminal_summary, b.terminal_summary);

    // Different timestamp ⇒ every file except manifest.json is still identical,
    // and manifest.json differs in exactly the one allowed field.
    let later = export::run_canonical_export(GENERATED_AT + 86_400).unwrap();
    for file in &a.files {
        let other = later.file(file.name).unwrap();
        if file.name == MANIFEST_FILE {
            assert_ne!(file.bytes, other.bytes, "the timestamp must be recorded");
        } else {
            assert_eq!(
                file.bytes, other.bytes,
                "{} must be deterministic",
                file.name
            );
        }
    }
    assert_eq!(a.terminal_summary, later.terminal_summary);

    let mut early: serde_json::Value = json(&a, MANIFEST_FILE);
    let late: serde_json::Value = json(&later, MANIFEST_FILE);
    assert_eq!(early["generated_at_unix_s"], GENERATED_AT);
    assert_eq!(late["generated_at_unix_s"], GENERATED_AT + 86_400);
    early["generated_at_unix_s"] = late["generated_at_unix_s"].clone();
    assert_eq!(early, late, "generated_at_unix_s is the only variation");
}

#[test]
fn exported_json_validates() {
    let exported = run();

    // The artifact file round-trips and re-validates: its stored pass/fail and
    // errors are re-derivable from its own measured/simulated values.
    let artifact: ComparisonArtifact =
        serde_json::from_slice(&exported.file(ARTIFACT_FILE).unwrap().bytes).unwrap();
    artifact.validate().unwrap();
    assert_eq!(artifact, exported.comparison.artifact);
    assert_eq!(artifact.case_id, ndrt_2020::CASE_ID);
    assert_eq!(artifact.case_hash, canonical_case().case_hash().unwrap());

    // The embedded case copy round-trips through the canonical loader, evidence
    // policy included.
    let case = ValidationCase::from_canonical_bytes(&exported.file(CASE_FILE).unwrap().bytes)
        .expect("exported case is canonical and policy-clean");
    assert_eq!(case, canonical_case());

    // The manifest round-trips into the same struct that produced it.
    let manifest: ExportManifest = serde_json::from_value(json(&exported, MANIFEST_FILE)).unwrap();
    assert_eq!(manifest, exported.manifest);

    // Manifest hashes agree with the bytes actually consumed.
    let sha = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let by_path: BTreeMap<&str, &str> = exported
        .manifest
        .sources
        .iter()
        .map(|s| (s.path.as_str(), s.sha256.as_str()))
        .collect();
    assert_eq!(
        by_path[case.fixture.path.as_str()],
        sha(export::CANONICAL_TELEMETRY_CSV)
    );
    assert_eq!(
        by_path[export::MOTOR_ENG_PATH],
        sha(export::CANONICAL_MOTOR_ENG)
    );
    assert_eq!(by_path[export::CASE_PATH], sha(export::CANONICAL_CASE_JSON));
    assert_eq!(
        exported.manifest.case_file_sha256,
        sha(export::CANONICAL_CASE_JSON)
    );
    assert!(exported.manifest.artifact_matches_canonical);
}

#[test]
fn markdown_summary_matches_the_artifact_values() {
    let exported = run();
    let summary = text(&exported, SUMMARY_FILE);

    for metric in &exported.comparison.artifact.metrics {
        // Every metric appears with the artifact's own numbers, formatted the
        // one way the renderer formats them.
        assert!(
            summary.contains(&format!("`{}`", metric.id)),
            "{}",
            metric.id
        );
        for value in [
            metric.measured,
            metric.simulated,
            metric.abs_error,
            metric.tolerance,
        ] {
            let rendered = format!("{value:.6}");
            assert!(
                summary.contains(&rendered),
                "summary is missing {} value {rendered}",
                metric.id
            );
        }
        // A relative error is shown only where there is a baseline to be
        // relative to. A zero-baseline metric is already a normalized ratio, so
        // its cell reads `N/A` instead of a misleading 0.000 %.
        if metric.measured != 0.0 {
            assert!(
                summary.contains(&format!("{:.3} %", metric.rel_error * 100.0)),
                "summary is missing {} rel_error",
                metric.id
            );
        }
    }

    // Identity and hashes are the artifact's, verbatim.
    let artifact = &exported.comparison.artifact;
    assert!(summary.contains(&artifact.input_hash));
    assert!(summary.contains(&artifact.case_hash));
    assert!(summary.contains(&artifact.model_version));
    for hash in artifact.source_hashes.values() {
        assert!(summary.contains(hash), "summary is missing source hash");
    }

    // Measured/simulated diagnostics agree with the executed comparison.
    let c = &exported.comparison;
    for value in [
        c.measured_apogee_agl_m,
        c.simulated_apogee_agl_m,
        c.measured_time_to_apogee_s,
        c.simulated_time_to_apogee_s,
        c.rmse_m,
    ] {
        assert!(summary.contains(&format!("{value:.6}")), "missing {value}");
    }

    // The overall verdict in the prose is the artifact's, not an independent claim.
    let verdict = if artifact.pass { "PASS" } else { "FAIL" };
    assert!(
        summary.contains(&format!("**Overall: {verdict}**")),
        "{summary}"
    );
}

#[test]
fn a_zero_baseline_metric_reports_no_relative_error() {
    let exported = run();
    let summary = text(&exported, SUMMARY_FILE);

    // `normalized_altitude_rmse` is already a dimensionless normalized ratio,
    // measured against a zero baseline. The stored artifact value is untouched…
    let metric = exported
        .comparison
        .artifact
        .metrics
        .iter()
        .find(|m| m.id == "normalized_altitude_rmse")
        .expect("normalized RMSE metric");
    assert_eq!(metric.measured, 0.0);
    assert!(metric.pass);

    // …but no relative error is displayed against that zero baseline.
    let record = exported
        .manifest
        .metrics
        .iter()
        .find(|m| m.id == "normalized_altitude_rmse")
        .expect("normalized RMSE record");
    assert_eq!(record.rel_error, None, "no baseline ⇒ no relative error");
    assert_eq!(
        json(&exported, MANIFEST_FILE)["metrics"][2]["rel_error"],
        serde_json::Value::Null
    );

    // Every metric that does have a baseline still reports one.
    for record in &exported.manifest.metrics {
        if record.id != "normalized_altitude_rmse" {
            assert!(record.rel_error.is_some(), "{}", record.id);
        }
    }

    // The report states the ratio and its threshold, and marks the relative
    // error absent rather than zero.
    assert!(summary.contains("- normalized RMSE: 3.790 %"), "{summary}");
    assert!(summary.contains("- threshold: 10.000 %"), "{summary}");
    assert!(summary.contains("- relative error: N/A"), "{summary}");
    assert!(
        summary.contains("| N/A |"),
        "table cell reads N/A:\n{summary}"
    );
}

#[test]
fn the_pinned_packet_is_distinguished_from_the_inputs_actually_verified() {
    let exported = run();
    let manifest = &exported.manifest;

    // Six files are pinned upstream; three were consumed and re-hashed here.
    assert_eq!(manifest.source_packet_files_pinned, 6);
    assert_eq!(manifest.source_packet.len(), 6);
    assert_eq!(manifest.simulation_inputs_verified, 3);
    assert_eq!(manifest.sources.len(), 3);

    // Exactly the two packet members the comparison consumes are marked verified;
    // the rest are declared pins this run did not re-check.
    let verified: Vec<&str> = manifest
        .source_packet
        .iter()
        .filter(|entry| entry.consumed_and_verified)
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(
        verified,
        vec![
            "data/validation/ndrt-2020/fullscale_2-23_raven1_crop.csv",
            export::MOTOR_ENG_PATH
        ]
    );
    // The third verified source is Ascent's own case spec, not a packet member.
    assert!(!manifest
        .source_packet
        .iter()
        .any(|entry| entry.path == export::CASE_PATH));

    // Terminal and report make the same distinction, in the same words.
    for rendered in [&exported.terminal_summary, &text(&exported, SUMMARY_FILE)] {
        assert!(
            rendered.contains("full source packet pinned")
                || rendered.contains("Full source packet pinned"),
            "{rendered}"
        );
        assert!(
            rendered.contains("6 files"),
            "packet count missing:\n{rendered}"
        );
        assert!(
            rendered.contains("simulation inputs consumed and verified")
                || rendered.contains("Simulation inputs consumed and verified"),
            "{rendered}"
        );
        assert!(
            rendered.contains("3 files"),
            "verified count missing:\n{rendered}"
        );
    }
    // The report says plainly that the unconsumed pins were not re-verified.
    assert!(text(&exported, SUMMARY_FILE).contains("not** re-verified here"));
}

#[test]
fn primary_metrics_and_input_checks_stay_separated() {
    let exported = run();
    let artifact = &exported.comparison.artifact;

    // The harness's classification survives into the export untouched.
    let primary: Vec<&str> = artifact
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
    let checks: Vec<&str> = artifact
        .metrics
        .iter()
        .filter(|m| m.kind == MetricKind::InputConsistency)
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(checks, vec!["burnout_time_s"]);

    // Counts are reported separately, and the overall result is the primary count.
    let m = &exported.manifest;
    assert_eq!((m.primary_metrics_passed, m.primary_metrics_total), (3, 3));
    assert_eq!((m.input_checks_passed, m.input_checks_total), (1, 1));
    assert_eq!(m.overall_pass, artifact.pass);

    // Roles are explicit in the manifest, one per metric.
    for record in &m.metrics {
        let expected = if primary.contains(&record.id.as_str()) {
            "primary_flight_metric"
        } else {
            "input_consistency_check"
        };
        assert_eq!(record.role, expected, "{}", record.id);
    }

    // The Markdown keeps them in separate, labelled sections, and says plainly
    // that the checks do not gate the result.
    let summary = text(&exported, SUMMARY_FILE);
    let primary_at = summary.find("## Primary flight metrics").expect("section");
    let checks_at = summary
        .find("## Input-consistency checks")
        .expect("section");
    assert!(primary_at < checks_at);
    let burnout_at = summary.find("`burnout_time_s`").expect("burnout listed");
    assert!(
        burnout_at > checks_at,
        "burnout must appear only under input-consistency"
    );
    let apogee_at = summary.find("`apogee_agl_m`").expect("apogee listed");
    assert!(apogee_at > primary_at && apogee_at < checks_at);
    assert!(summary.contains("**excluded**"));

    // The terminal summary separates them too.
    let terminal = &exported.terminal_summary;
    assert!(terminal.contains("primary flight metrics — 3/3 pass"));
    assert!(terminal.contains("input-consistency checks — 1/1 pass"));
    assert!(terminal.contains("never gating"));
}

#[test]
fn evidence_label_and_credibility_caveats_appear_without_promotion() {
    let exported = run();
    let case = canonical_case();

    // The label is the case's declared rung — still FlightDataAvailable.
    assert_eq!(case.evidence_level, EvidenceLevel::FlightDataAvailable);
    assert_eq!(exported.manifest.evidence_level, "flight_data_available");

    let summary = text(&exported, SUMMARY_FILE);
    assert!(summary.contains("`flight_data_available`"));
    assert!(!summary.contains("**`flight_validated`**"));
    assert!(summary.contains("does not promote it"));
    assert!(exported
        .terminal_summary
        .contains("flight_data_available (not promoted by this run)"));

    // Every credibility caveat from the case reaches both the manifest and the
    // report, verbatim.
    assert!(!case.caveats.is_empty());
    assert_eq!(exported.manifest.credibility_caveats, case.caveats);
    for caveat in &case.caveats {
        assert!(
            summary.contains(caveat.as_str()),
            "missing caveat: {caveat}"
        );
        assert!(
            exported.terminal_summary.contains(caveat.as_str()),
            "terminal is missing caveat: {caveat}"
        );
    }

    // Assumptions, limitations, and known mismatches are carried too.
    assert_eq!(
        exported.manifest.assumptions_and_limitations,
        exported.comparison.artifact.known_limitations
    );
    assert_eq!(exported.manifest.known_mismatches, case.known_mismatches);
    for item in case
        .known_mismatches
        .iter()
        .chain(&case.validity_domain)
        .chain(&case.uncertainty)
        .chain(&exported.comparison.artifact.known_limitations)
    {
        assert!(summary.contains(item.as_str()), "missing: {item}");
    }

    // Exporting does not touch the checked-in case's rung.
    let reloaded = ValidationCase::from_canonical_bytes(export::CANONICAL_CASE_JSON).unwrap();
    assert_eq!(reloaded.evidence_level, EvidenceLevel::FlightDataAvailable);
}

#[test]
fn export_directory_is_self_contained_and_complete() {
    let exported = run();
    let names: Vec<&str> = exported.files.iter().map(|f| f.name).collect();
    assert_eq!(
        names,
        vec![ARTIFACT_FILE, SUMMARY_FILE, MANIFEST_FILE, CASE_FILE]
    );
    for file in &exported.files {
        assert!(!file.bytes.is_empty(), "{} is empty", file.name);
    }
}
