//! The `ascent-cli compare-flight` workflow end to end, on a real directory.
//!
//! `ascent_review::ndrt_2020_export` owns (and proves) the comparison itself;
//! these tests cover what the CLI layer adds: the selector, the filesystem
//! write, and the exit-code contract — including that rerunning into a clean
//! directory produces equivalent evidence.

use std::path::{Path, PathBuf};

use ascent_app::cli::compare_flight;
use ascent_review::ndrt_2020_export::{
    ARTIFACT_FILE, CASE_FILE, CASE_SELECTOR, MANIFEST_FILE, SUMMARY_FILE,
};
use ascent_review::validation::{ComparisonArtifact, ValidationCase};

const GENERATED_AT: u64 = 1_700_000_000;

/// A fresh, empty directory under the target dir. Removed first so every run
/// starts clean, which is exactly the property one of these tests asserts.
fn clean_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn read(dir: &Path, name: &str) -> Vec<u8> {
    std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("reading {name}: {e}"))
}

#[test]
fn compare_flight_writes_a_self_contained_directory() {
    let dir = clean_dir("compare-flight-basic");
    let report = compare_flight(CASE_SELECTOR, &dir, GENERATED_AT).expect("comparison passes");

    // Every documented file landed on disk.
    for name in [ARTIFACT_FILE, SUMMARY_FILE, MANIFEST_FILE, CASE_FILE] {
        assert!(!read(&dir, name).is_empty(), "{name} is missing or empty");
    }

    // The artifact on disk re-validates on its own terms.
    let artifact: ComparisonArtifact = serde_json::from_slice(&read(&dir, ARTIFACT_FILE)).unwrap();
    artifact.validate().unwrap();
    assert!(artifact.pass);

    // The case copy on disk still loads under the evidence policy, at its
    // declared rung.
    let case = ValidationCase::from_canonical_bytes(&read(&dir, CASE_FILE)).unwrap();
    assert_eq!(case.case_id, "ndrt-2020-flight");

    // The terminal report is concise, states the result, names the label, and
    // says where the evidence went.
    assert!(report.contains("ndrt-2020-flight — executed comparison: PASS"));
    assert!(report.contains("flight_data_available (not promoted by this run)"));
    assert!(report.contains("primary flight metrics — 3/3 pass"));
    assert!(report.contains("input-consistency checks — 1/1 pass"));
    assert!(report.contains("credibility caveats"));
    // The pinned packet and the inputs this run actually verified are separate
    // claims, and the terminal says so.
    assert!(report.contains("full source packet pinned: 6 files"));
    assert!(report.contains("simulation inputs consumed and verified: 3 files"));
    assert!(report.contains(&format!("wrote 4 files to {}", dir.display())));
    assert!(
        report.lines().count() < 30,
        "terminal summary should stay on one screen:\n{report}"
    );
}

#[test]
fn rerunning_into_a_clean_directory_produces_equivalent_evidence() {
    let first = clean_dir("compare-flight-rerun-a");
    let second = clean_dir("compare-flight-rerun-b");
    compare_flight(CASE_SELECTOR, &first, GENERATED_AT).unwrap();
    // A later run, at a different time, into a directory that never held output.
    compare_flight(CASE_SELECTOR, &second, GENERATED_AT + 3_600).unwrap();

    // The evidence itself is byte-identical.
    for name in [ARTIFACT_FILE, SUMMARY_FILE, CASE_FILE] {
        assert_eq!(read(&first, name), read(&second, name), "{name} differs");
    }

    // The manifest differs in exactly the recorded timestamp, and nothing else.
    let mut a: serde_json::Value = serde_json::from_slice(&read(&first, MANIFEST_FILE)).unwrap();
    let b: serde_json::Value = serde_json::from_slice(&read(&second, MANIFEST_FILE)).unwrap();
    assert_eq!(a["generated_at_unix_s"], GENERATED_AT);
    assert_eq!(b["generated_at_unix_s"], GENERATED_AT + 3_600);
    a["generated_at_unix_s"] = b["generated_at_unix_s"].clone();
    assert_eq!(a, b);
}

#[test]
fn rerunning_over_an_existing_directory_refreshes_it_in_place() {
    let dir = clean_dir("compare-flight-overwrite");
    compare_flight(CASE_SELECTOR, &dir, GENERATED_AT).unwrap();
    let summary = read(&dir, SUMMARY_FILE);

    // Corrupt an output, then rerun: the command rewrites it rather than
    // trusting what it finds.
    std::fs::write(dir.join(SUMMARY_FILE), b"stale").unwrap();
    compare_flight(CASE_SELECTOR, &dir, GENERATED_AT).unwrap();
    assert_eq!(read(&dir, SUMMARY_FILE), summary);
}

#[test]
fn an_unknown_selector_is_an_error_that_lists_what_exists() {
    let dir = clean_dir("compare-flight-unknown");
    let err = compare_flight("no-such-flight", &dir, GENERATED_AT).unwrap_err();
    assert!(
        err.contains("unknown flight comparison 'no-such-flight'"),
        "{err}"
    );
    assert!(err.contains(CASE_SELECTOR), "{err}");
    // Nothing was written for a request that was never valid.
    assert!(!dir.exists());
}

#[test]
fn usage_documents_the_command() {
    let usage = ascent_app::cli::USAGE;
    assert!(usage.contains("compare-flight ndrt-2020 --output <directory>"));
    assert!(usage.contains("comparison artifact"));
}
