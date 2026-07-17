use ascent_domain::eng_import::{parse_eng, EngImportError};
use serde_json::json;

const MANIFEST: &str = include_str!("../../../data/eng-samples/MANIFEST.json");

const B6: &str = include_str!("../../../data/eng-samples/estes_b6_cert.eng");
const C6: &str = include_str!("../../../data/eng-samples/estes_c6_cert.eng");
const D10: &str = include_str!("../../../data/eng-samples/openrocket_d10.eng");
const MULTI: &str = include_str!("../../../data/eng-samples/estes_multi_entry.eng");
const D12_MISSING_ZERO: &str =
    include_str!("../../../data/eng-samples/estes_d12_missing_terminal_zero.eng");

/// The MANIFEST enumerates the sample corpus; this pins that we're reading
/// the same directory the manifest describes (so a future rename of a
/// fixture without updating the manifest fails loudly here).
#[test]
fn manifest_lists_all_five_samples() {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let samples = manifest["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 5);
    let files: Vec<&str> = samples.iter().map(|s| s["file"].as_str().unwrap()).collect();
    for expected in [
        "estes_b6_cert.eng",
        "estes_c6_cert.eng",
        "openrocket_d10.eng",
        "estes_multi_entry.eng",
        "estes_d12_missing_terminal_zero.eng",
    ] {
        assert!(files.contains(&expected), "manifest missing {expected}");
    }
}

#[test]
fn parses_b6_cert_with_expected_impulse() {
    let motors = parse_eng(B6, &json!({"source": "test"})).expect("b6 should parse");
    assert_eq!(motors.len(), 1);
    let m = &motors[0];
    // This upstream cert file bakes the delay into the common_name token
    // itself ("B6-0" rather than "B6" + delay "0"); RASP allows either.
    assert_eq!(m.designation, "B6-0");
    assert_eq!(m.manufacturer, "E");
    assert!((m.total_mass_kg - 0.0156).abs() < 1e-9);
    assert!((m.propellant_mass_kg - 0.0056).abs() < 1e-9);
    // Total impulse for a B motor is nominally 2.5-5.0 N*s.
    assert!(
        m.expected_total_impulse_ns > 2.0 && m.expected_total_impulse_ns < 5.5,
        "B6 impulse out of range: {}",
        m.expected_total_impulse_ns
    );
    assert!(m.provenance.get("source").is_some());
    m.validate().expect("parsed B6 must validate cleanly");
}

#[test]
fn parses_c6_cert_leading_dot_kilograms_and_hyphenated_delays() {
    let motors = parse_eng(C6, &json!({"source": "test"})).expect("c6 should parse");
    assert_eq!(motors.len(), 1);
    let m = &motors[0];
    assert_eq!(m.designation, "C6");
    assert_eq!(m.manufacturer, "E");
    assert!((m.total_mass_kg - 0.0231).abs() < 1e-9);
    assert!((m.propellant_mass_kg - 0.0108).abs() < 1e-9);
    assert!(
        m.expected_total_impulse_ns > 8.0 && m.expected_total_impulse_ns < 10.0,
        "C6 impulse out of range: {}",
        m.expected_total_impulse_ns
    );
    m.validate().expect("parsed C6 must validate cleanly");
}

#[test]
fn parses_openrocket_d10_no_comments_manufacturer_at() {
    let motors = parse_eng(D10, &json!({"source": "test"})).expect("d10 should parse");
    assert_eq!(motors.len(), 1);
    let m = &motors[0];
    assert_eq!(m.designation, "D10");
    assert_eq!(m.manufacturer, "AT");
    assert!((m.total_mass_kg - 0.0259).abs() < 1e-9);
    m.validate().expect("parsed D10 must validate cleanly");
}

#[test]
fn parses_multi_entry_file_into_two_motors() {
    let motors = parse_eng(MULTI, &json!({"source": "test"})).expect("multi-entry should parse");
    assert_eq!(motors.len(), 2);
    assert_eq!(motors[0].designation, "C6");
    assert_eq!(motors[1].designation, "A10T");
    for m in &motors {
        m.validate().expect("every entry in a multi-motor file must validate");
    }
}

#[test]
fn missing_terminal_zero_is_rejected_with_line_number() {
    let err = parse_eng(D12_MISSING_ZERO, &json!({"source": "test"}))
        .expect_err("truncated entry without terminal zero must fail");
    match err {
        EngImportError::Malformed { line, message } => {
            assert!(line > 0, "error must carry a real line number");
            assert!(
                message.to_lowercase().contains("zero"),
                "error should explain the missing terminal zero, got: {message}"
            );
        }
    }
}

#[test]
fn truncated_header_reports_line_number() {
    let bad = "; comment\nC6 18 70 0-3-5-7 .0108 .0231\n0.031 0.946\n0.139 0.0\n";
    let err = parse_eng(bad, &json!({})).expect_err("5-field header must fail");
    match err {
        EngImportError::Malformed { line, message } => {
            assert_eq!(line, 2, "header is on line 2 of this fixture");
            assert!(message.contains("7"), "message should mention the expected field count: {message}");
        }
    }
}

#[test]
fn non_numeric_thrust_sample_reports_line_number() {
    let bad = "C6 18 70 0-3-5-7 .0108 .0231 E\n0.031 oops\n0.139 0.0\n";
    let err = parse_eng(bad, &json!({})).expect_err("non-numeric thrust must fail");
    match err {
        EngImportError::Malformed { line, .. } => {
            assert_eq!(line, 2, "bad data is on line 2 of this fixture");
        }
    }
}

#[test]
fn empty_source_is_rejected() {
    let err = parse_eng("", &json!({})).expect_err("empty source has no motor entries");
    match err {
        EngImportError::Malformed { message, .. } => {
            assert!(message.to_lowercase().contains("no motor"));
        }
    }
}
