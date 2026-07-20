use ascent_domain::evidence::ClockDomain;
use ascent_review::alignment::{AlignmentArtifact, AlignmentMethod, AlignmentObservation};

fn hash(byte: u8) -> String {
    format!("{byte:02x}").repeat(32)
}

#[test]
fn constrained_fit_recovers_known_offset_and_drift_deterministically() {
    let observations = (0..20)
        .map(|index| {
            let source_s = 5.0 + index as f64 * 2.0;
            AlignmentObservation {
                source_s,
                target_s: source_s * 0.999_8 - 1.25,
                weight: 1.0,
                evidence_hash: hash(1),
            }
        })
        .collect::<Vec<_>>();
    let first = AlignmentArtifact::fit_constrained(
        ClockDomain("vehicle".into()),
        ClockDomain("simulation".into()),
        &observations,
        500.0,
    )
    .unwrap();
    let second = AlignmentArtifact::fit_constrained(
        ClockDomain("vehicle".into()),
        ClockDomain("simulation".into()),
        &observations,
        500.0,
    )
    .unwrap();

    assert!((first.scale - 0.999_8).abs() < 1e-12);
    assert!((first.offset_s + 1.25).abs() < 1e-12);
    assert_eq!(first.method, AlignmentMethod::ConstrainedOffsetDrift);
    assert_eq!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
}

#[test]
fn weak_or_discontinuous_alignment_evidence_fails_visibly() {
    let one = [AlignmentObservation {
        source_s: 1.0,
        target_s: 2.0,
        weight: 1.0,
        evidence_hash: hash(1),
    }];
    assert!(AlignmentArtifact::fit_constrained(
        ClockDomain("source".into()),
        ClockDomain("target".into()),
        &one,
        100.0,
    )
    .unwrap_err()
    .contains("at least two"));

    let excessive = [
        AlignmentObservation {
            source_s: 0.0,
            target_s: 0.0,
            weight: 1.0,
            evidence_hash: hash(1),
        },
        AlignmentObservation {
            source_s: 10.0,
            target_s: 20.0,
            weight: 1.0,
            evidence_hash: hash(1),
        },
    ];
    assert!(AlignmentArtifact::fit_constrained(
        ClockDomain("source".into()),
        ClockDomain("target".into()),
        &excessive,
        100.0,
    )
    .unwrap_err()
    .contains("drift"));
}

#[test]
fn manual_and_event_modes_remain_distinct_auditable_candidates() {
    let manual = AlignmentArtifact::manual(
        ClockDomain("sensor".into()),
        ClockDomain("simulation".into()),
        -2.0,
        1.0,
        [2.0, 30.0],
        "operator matched ignition markers",
        vec![hash(2)],
    )
    .unwrap();
    let event = AlignmentArtifact::from_event_pairs(
        ClockDomain("sensor".into()),
        ClockDomain("simulation".into()),
        &[
            AlignmentObservation {
                source_s: 2.0,
                target_s: 0.0,
                weight: 1.0,
                evidence_hash: hash(2),
            },
            AlignmentObservation {
                source_s: 12.0,
                target_s: 10.0,
                weight: 1.0,
                evidence_hash: hash(3),
            },
        ],
    )
    .unwrap();
    assert_eq!(manual.method, AlignmentMethod::Manual);
    assert_eq!(event.method, AlignmentMethod::EventCorrelation);
    assert_eq!(manual.map(12.0).unwrap(), 10.0);
    assert_eq!(event.map(12.0).unwrap(), 10.0);
}
