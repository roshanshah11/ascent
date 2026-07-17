use ascent_domain::eng_import::{parse_eng, EngImportError};
use serde_json::json;
use std::{collections::BTreeSet, fs};

const MANIFEST: &str = include_str!("../../../data/eng-samples/MANIFEST.json");

const B6: &str = include_str!("../../../data/eng-samples/estes_b6_cert.eng");
const C6: &str = include_str!("../../../data/eng-samples/estes_c6_cert.eng");
const D10: &str = include_str!("../../../data/eng-samples/openrocket_d10.eng");
const MULTI: &str = include_str!("../../../data/eng-samples/estes_multi_entry.eng");
const D12_MISSING_ZERO: &str =
    include_str!("../../../data/eng-samples/estes_d12_missing_terminal_zero.eng");

fn fixture_contents(file: &str) -> &'static str {
    match file {
        "estes_b6_cert.eng" => B6,
        "estes_c6_cert.eng" => C6,
        "openrocket_d10.eng" => D10,
        "estes_multi_entry.eng" => MULTI,
        "estes_d12_missing_terminal_zero.eng" => D12_MISSING_ZERO,
        other => panic!("manifest names an unknown corpus fixture: {other}"),
    }
}

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
fn manifest_and_physical_eng_corpus_have_exactly_the_same_files() {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let manifest_files: BTreeSet<String> = manifest["samples"]
        .as_array()
        .expect("manifest samples must be an array")
        .iter()
        .map(|sample| {
            sample["file"]
                .as_str()
                .expect("manifest sample must name a file")
                .to_owned()
        })
        .collect();
    let corpus_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("data/eng-samples");
    let physical_files: BTreeSet<String> = fs::read_dir(&corpus_dir)
        .expect("the corpus directory must be readable")
        .map(|entry| entry.expect("corpus directory entries must be readable").path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("eng"))
        .map(|path| {
            path.file_name()
                .expect(".eng file must have a name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    assert_eq!(
        physical_files, manifest_files,
        "MANIFEST.json must name every and only physical .eng corpus fixture"
    );
}

#[test]
fn manifest_drives_exact_impulse_and_rejection_expectations_for_full_corpus() {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let samples = manifest["samples"].as_array().expect("manifest samples must be an array");

    for sample in samples {
        let file = sample["file"].as_str().expect("sample must name its fixture");
        let expected = sample["expected"]
            .as_object()
            .unwrap_or_else(|| panic!("manifest sample {file} must declare an expectation"));
        let contents = fixture_contents(file);

        match expected["kind"].as_str() {
            Some("valid") => {
                let expected_motors = expected["motors"]
                    .as_array()
                    .unwrap_or_else(|| panic!("valid sample {file} must declare motors"));
                let motors = parse_eng(contents, &json!({"fixture": file}))
                    .unwrap_or_else(|err| panic!("valid sample {file} did not parse: {err}"));
                assert_eq!(motors.len(), expected_motors.len(), "motor count for {file}");

                for (motor, expected_motor) in motors.iter().zip(expected_motors) {
                    assert_eq!(
                        motor.designation,
                        expected_motor["designation"]
                            .as_str()
                            .expect("motor expectation must name its designation"),
                        "designation for {file}"
                    );
                    assert_eq!(
                        motor.expected_total_impulse_ns,
                        expected_motor["total_impulse_ns"]
                            .as_f64()
                            .expect("motor expectation must declare exact total impulse"),
                        "total impulse for {} in {file}",
                        motor.designation
                    );
                }
            }
            Some("invalid") => {
                let expected_error = expected["error"]
                    .as_object()
                    .unwrap_or_else(|| panic!("invalid sample {file} must declare an error"));
                let err = parse_eng(contents, &json!({"fixture": file}))
                    .expect_err("manifest-invalid fixture must be rejected");
                match err {
                    EngImportError::Malformed { line, message } => {
                        assert_eq!(
                            line,
                            expected_error["line"]
                                .as_u64()
                                .expect("error expectation must declare a line") as usize,
                            "error line for {file}"
                        );
                        match expected_error["class"].as_str() {
                            Some("missing_terminal_zero") => assert!(
                                message.contains("terminal zero"),
                                "missing-terminal-zero error for {file}: {message}"
                            ),
                            Some(class) => panic!("unsupported manifest error class {class}"),
                            None => panic!("error expectation for {file} must declare a class"),
                        }
                    }
                }
            }
            Some(kind) => panic!("unsupported expectation kind {kind} for {file}"),
            None => panic!("manifest sample {file} must declare an expectation kind"),
        }
    }
}

#[test]
fn malformed_sample_times_and_thrusts_report_their_source_lines() {
    let cases = [
        (
            "non-monotonic time",
            "C6 18 70 0-3-5-7 .0108 .0231 E\n0.1 5\n0.1 4\n0.2 0\n",
            3,
            "strictly increasing",
        ),
        (
            "non-finite time",
            "C6 18 70 0-3-5-7 .0108 .0231 E\nNaN 5\n0.2 0\n",
            2,
            "finite time_s",
        ),
        (
            "non-finite thrust",
            "C6 18 70 0-3-5-7 .0108 .0231 E\n0.1 NaN\n0.2 0\n",
            2,
            "finite thrust_n",
        ),
        (
            "explicit initial sample",
            "C6 18 70 0-3-5-7 .0108 .0231 E\n0 5\n0.2 0\n",
            2,
            "implicit",
        ),
        (
            "data after terminal zero",
            "C6 18 70 0-3-5-7 .0108 .0231 E\n0.1 5\n0.2 0\n0.3 1\n",
            4,
            "terminal zero",
        ),
        (
            "adjacent entry without comment separator",
            "C6 18 70 0-3-5-7 .0108 .0231 E\n0.1 5\n0.2 0\nB6 18 70 0 .0056 .0156 E\n0.1 4\n0.2 0\n",
            4,
            "separating comment",
        ),
    ];

    for (name, source, expected_line, expected_message) in cases {
        let err = parse_eng(source, &json!({})).expect_err(name);
        match err {
            EngImportError::Malformed { line, message } => {
                assert_eq!(line, expected_line, "{name} line");
                assert!(
                    message.contains(expected_message),
                    "{name} message should contain '{expected_message}', got: {message}"
                );
            }
        }
    }
}

#[test]
fn malformed_header_numbers_and_negative_thrust_report_source_lines() {
    let cases = [
        (
            "non-numeric diameter",
            "C6 not-a-number 70 0-3-5-7 .0108 .0231 E\n0.1 5\n0.2 0\n",
            1,
            "invalid diameter_mm",
        ),
        (
            "non-finite diameter",
            "C6 NaN 70 0-3-5-7 .0108 .0231 E\n0.1 5\n0.2 0\n",
            1,
            "finite diameter_mm",
        ),
        (
            "non-finite length",
            "C6 18 NaN 0-3-5-7 .0108 .0231 E\n0.1 5\n0.2 0\n",
            1,
            "finite length_mm",
        ),
        (
            "non-finite propellant mass",
            "C6 18 70 0-3-5-7 NaN .0231 E\n0.1 5\n0.2 0\n",
            1,
            "finite propellant_kg",
        ),
        (
            "non-finite loaded mass",
            "C6 18 70 0-3-5-7 .0108 NaN E\n0.1 5\n0.2 0\n",
            1,
            "finite loaded_mass_kg",
        ),
        (
            "nonpositive diameter",
            "C6 0 70 0-3-5-7 .0108 .0231 E\n0.1 5\n0.2 0\n",
            1,
            "positive diameter_mm",
        ),
        (
            "nonpositive length",
            "C6 18 -70 0-3-5-7 .0108 .0231 E\n0.1 5\n0.2 0\n",
            1,
            "positive length_mm",
        ),
        (
            "nonpositive propellant mass",
            "C6 18 70 0-3-5-7 0 .0231 E\n0.1 5\n0.2 0\n",
            1,
            "positive propellant_kg",
        ),
        (
            "nonpositive loaded mass",
            "C6 18 70 0-3-5-7 .0108 0 E\n0.1 5\n0.2 0\n",
            1,
            "positive loaded_mass_kg",
        ),
        (
            "propellant exceeding loaded mass",
            "C6 18 70 0-3-5-7 .03 .0231 E\n0.1 5\n0.2 0\n",
            1,
            "exceeds loaded_mass_kg",
        ),
        (
            "negative thrust",
            "C6 18 70 0-3-5-7 .0108 .0231 E\n0.1 -1\n0.2 0\n",
            2,
            "negative thrust_n",
        ),
    ];

    for (name, source, expected_line, expected_message) in cases {
        let err = parse_eng(source, &json!({})).expect_err(name);
        match err {
            EngImportError::Malformed { line, message } => {
                assert_eq!(line, expected_line, "{name} line");
                assert!(
                    message.contains(expected_message),
                    "{name} message should contain '{expected_message}', got: {message}"
                );
            }
        }
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
