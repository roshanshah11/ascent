use std::collections::{BTreeMap, BTreeSet};

use ascent_domain::evidence::{
    AffineClockTransform, ArtifactEvidence, Channel, ChannelKind, ClockDomain, CoordinateFrame,
    EvidenceLevel, EvidenceManifest, FlightTrace, FrameKind, ReviewClock, Sample, TimeBase,
    TimeSystem, TrackKind, TypedEvent, Uncertainty, Unit, EVIDENCE_SCHEMA_VERSION,
};

fn hash(byte: u8) -> String {
    format!("{byte:02x}").repeat(32)
}

fn trace() -> FlightTrace {
    FlightTrace {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        trace_id: "predicted-sixdof".into(),
        time_bases: vec![TimeBase {
            id: "sim".into(),
            system: TimeSystem::SimulationElapsed,
            epoch: None,
        }],
        frames: vec![CoordinateFrame {
            id: "launch-enu".into(),
            kind: FrameKind::EastNorthUp,
            parent_id: None,
        }],
        channels: vec![Channel {
            id: "truth.attitude".into(),
            kind: ChannelKind::AttitudeQuaternionWxyz,
            track: TrackKind::Simulated,
            time_base_id: "sim".into(),
            frame_id: Some("launch-enu".into()),
            unit: Unit::Dimensionless,
            samples: vec![
                Sample {
                    time: 0.0,
                    values: vec![1.0, 0.0, 0.0, 0.0],
                    valid: true,
                },
                Sample {
                    time: 0.1,
                    values: vec![0.999_687_516_3, 0.0, 0.024_997_395_9, 0.0],
                    valid: true,
                },
            ],
            uncertainty: Some(Uncertainty {
                representation: "one_sigma".into(),
                values: vec![0.001, 0.001, 0.001],
                unit: Unit::Radian,
            }),
            parent_hashes: vec![hash(1)],
        }],
        events: vec![TypedEvent {
            id: "rail-exit".into(),
            event_type: "rail_exit".into(),
            time_base_id: "sim".into(),
            time: 0.1,
            detector_id: "ascent.rail-crossing".into(),
            detector_version: "1".into(),
            confidence: 1.0,
            source_sample_ranges: vec!["truth.attitude:0..=1".into()],
            evidence_hashes: vec![hash(1)],
        }],
    }
}

#[test]
fn trace_canonical_bytes_round_trip_and_validate() {
    let trace = trace();
    trace.validate().unwrap();
    let bytes = trace.canonical_bytes().unwrap();
    let decoded = FlightTrace::from_canonical_bytes(&bytes).unwrap();
    assert_eq!(bytes, decoded.canonical_bytes().unwrap());
}

#[test]
fn trace_rejects_unknown_versions_non_finite_values_bad_quaternions_and_clock_reversal() {
    let mut candidate = trace();
    candidate.schema_version += 1;
    assert!(candidate.validate().unwrap_err().contains("schema version"));

    let mut candidate = trace();
    candidate.channels[0].samples[0].values[0] = f64::NAN;
    assert!(candidate.validate().unwrap_err().contains("finite"));

    let mut candidate = trace();
    candidate.channels[0].samples[0].values = vec![2.0, 0.0, 0.0, 0.0];
    assert!(candidate
        .validate()
        .unwrap_err()
        .contains("unit quaternion"));

    let mut candidate = trace();
    candidate.channels[0].samples[1].time = -0.1;
    assert!(candidate.validate().unwrap_err().contains("monotonic"));
}

#[test]
fn evidence_manifest_rejects_broken_parent_hashes() {
    let manifest = EvidenceManifest {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        manifest_id: "review-001".into(),
        intended_use: "six degree of freedom mission review".into(),
        validity_domain: vec!["subsonic".into()],
        evidence_level: EvidenceLevel::CrossValidated,
        input_pedigree: vec!["licensed fixture".into()],
        frozen_thresholds: BTreeMap::from([("attitude_rmse_deg".into(), 1.0)]),
        caveats: vec!["no transonic validation".into()],
        artifacts: vec![ArtifactEvidence {
            artifact_id: "residuals".into(),
            sha256: hash(2),
            parent_hashes: vec![hash(99)],
            transformation: Some("phase reconciliation v1".into()),
        }],
        root_hashes: BTreeSet::from([hash(1)]),
    };
    assert!(manifest.validate().unwrap_err().contains("parent hash"));
}

#[test]
fn review_clock_maps_domains_without_rewriting_source_time() {
    let measured = ClockDomain("vehicle-mcu".into());
    let simulation = ClockDomain("simulation".into());
    let clock = ReviewClock::new(
        simulation.clone(),
        BTreeMap::from([(
            measured.clone(),
            AffineClockTransform {
                target_domain: simulation.clone(),
                offset_s: -2.0,
                scale: 0.999,
                valid_source_interval_s: Some([2.0, 102.0]),
            },
        )]),
    )
    .unwrap();

    let original = 12.0;
    let mapped = clock.map_to_review(&measured, original).unwrap();
    assert!((mapped - 9.988).abs() < 1e-12);
    assert_eq!(original, 12.0, "mapping must not mutate source timestamps");
    assert_eq!(clock.map_to_review(&simulation, 12.0).unwrap(), 12.0);
}
