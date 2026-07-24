use std::collections::BTreeMap;
use std::path::PathBuf;

use ascent_domain::evidence::EvidenceLevel;
use ascent_review::validation::{ComparisonResult, ValidationCase};

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn checked_in_ladder_is_complete_provenance_bound_and_qualified() {
    let cases = [
        "analytic-constant-thrust.json",
        "openrocket-24_12.json",
        "rocketpy-cross-validation.json",
        "valkyrie-2025-flight.json",
    ]
    .map(|name| {
        let bytes = std::fs::read(workspace().join("data/validation/cases").join(name)).unwrap();
        ValidationCase::from_canonical_bytes(&bytes).unwrap()
    });

    assert_eq!(cases[0].evidence_level, EvidenceLevel::Analytic);
    assert_eq!(cases[1].evidence_level, EvidenceLevel::RegressionCompatible);
    assert_eq!(cases[2].evidence_level, EvidenceLevel::CrossValidated);
    assert_eq!(cases[3].evidence_level, EvidenceLevel::FlightDataAvailable);
    for case in &cases {
        case.verify_fixture(&workspace()).unwrap();
        assert!(!case.fixture.license.is_empty());
        assert!(!case.configuration_mapping.is_empty());
        assert!(!case.metrics.is_empty());
        assert!(!case.validity_domain.is_empty());
        assert!(!case.uncertainty.is_empty());
    }
    assert!(cases[1]
        .known_mismatches
        .iter()
        .any(|mismatch| mismatch.contains("low-fidelity")));
}

#[test]
fn fixture_or_mapping_changes_invalidate_previous_results() {
    let bytes =
        std::fs::read(workspace().join("data/validation/cases/valkyrie-2025-flight.json")).unwrap();
    let mut case = ValidationCase::from_canonical_bytes(&bytes).unwrap();
    let original = case.case_hash().unwrap();

    case.configuration_mapping
        .insert("altitude".into(), "wrong_column".into());
    assert_ne!(original, case.case_hash().unwrap());

    let mut tampered = std::fs::read(workspace().join(&case.fixture.path)).unwrap();
    tampered.push(b'\n');
    assert!(case.verify_fixture_bytes(&tampered).is_err());
}

#[test]
fn deterministic_results_keep_levels_and_caveats_visible() {
    let bytes =
        std::fs::read(workspace().join("data/validation/cases/openrocket-24_12.json")).unwrap();
    let case = ValidationCase::from_canonical_bytes(&bytes).unwrap();
    let result = ComparisonResult::evaluate(
        &case,
        BTreeMap::from([("apogee_relative_error".into(), 0.014)]),
    )
    .unwrap();

    assert_eq!(result.evidence_level, EvidenceLevel::RegressionCompatible);
    assert_eq!(result.caveats, case.caveats);
    assert_eq!(result.known_mismatches, case.known_mismatches);
    assert_eq!(
        result.canonical_bytes().unwrap(),
        result.canonical_bytes().unwrap()
    );
}
