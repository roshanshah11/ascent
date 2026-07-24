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

/// Executed evidence for the `FlightDataAvailable` claim (see
/// `docs/VALKYRIE_VALIDATION.md`). The measured Valkyrie trace is real,
/// hash-pinned, and contains **exactly** a time/altitude signal — no vehicle,
/// motor, or event channel from which a simulation *input* could be built. This
/// test documents both what the dataset can supply (measured apogee and
/// time-to-apogee) and, by asserting the two-column shape, what it cannot. It is
/// characterization of the measured data, not a model-versus-flight comparison;
/// the case therefore cannot be `FlightValidated`.
#[test]
fn valkyrie_measured_signal_is_characterized() {
    let case_bytes =
        std::fs::read(workspace().join("data/validation/cases/valkyrie-2025-flight.json")).unwrap();
    let case = ValidationCase::from_canonical_bytes(&case_bytes).unwrap();
    assert_eq!(case.evidence_level, EvidenceLevel::FlightDataAvailable);

    let csv = std::fs::read(workspace().join(&case.fixture.path)).unwrap();
    // The characterized bytes are exactly the hash-pinned fixture.
    case.verify_fixture_bytes(&csv).unwrap();

    let text = String::from_utf8(csv).unwrap();
    let mut lines = text.lines();
    // Two columns only: the executed proof that no input channel is present.
    assert_eq!(lines.next().unwrap().trim(), "time,altitude");

    let mut rows: Vec<(f64, f64)> = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let mut fields = line.split(',');
        let t: f64 = fields.next().unwrap().parse().unwrap();
        let altitude: f64 = fields.next().unwrap().parse().unwrap();
        assert!(fields.next().is_none(), "unexpected third column");
        rows.push((t, altitude));
    }
    assert_eq!(rows.len(), 12_578);

    let (apogee_t, apogee_alt) =
        rows.iter().copied().fold(
            (0.0_f64, f64::MIN),
            |acc, (t, a)| if a > acc.1 { (t, a) } else { acc },
        );
    // The only outputs this dataset can supply. Pinned tight because the fixture
    // is frozen and hash-verified above.
    assert!((apogee_alt - 2059.0).abs() < 1.0, "apogee {apogee_alt}");
    assert!((apogee_t - 20.84).abs() < 0.05, "time-to-apogee {apogee_t}");

    // Complete ascent and descent: a long trace that returns near the ground.
    let duration = rows.last().unwrap().0 - rows.first().unwrap().0;
    assert!(duration > 120.0, "duration {duration}");
    assert!(rows.last().unwrap().1 < 10.0, "final altitude");
}
