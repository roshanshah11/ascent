use ascent_domain::evidence::{
    Channel, ChannelKind, ClockDomain, CoordinateFrame, FlightTrace, FrameKind, Sample, TimeBase,
    TimeSystem, TrackKind, TypedEvent, Uncertainty, Unit, EVIDENCE_SCHEMA_VERSION,
};
use ascent_review::alignment::AlignmentArtifact;
use ascent_review::reconciliation::{ChannelPair, DiagnosticCandidate, PhaseWindow, Reconciler};

fn hash(byte: u8) -> String {
    format!("{byte:02x}").repeat(32)
}

fn trace(id: &str, domain: &str, times: &[f64], altitude: &[f64], yaw_rad: &[f64]) -> FlightTrace {
    FlightTrace {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        trace_id: id.into(),
        time_bases: vec![TimeBase {
            id: domain.into(),
            system: TimeSystem::DeviceElapsed,
            epoch: None,
        }],
        frames: vec![CoordinateFrame {
            id: "launch-enu".into(),
            kind: FrameKind::EastNorthUp,
            parent_id: None,
        }],
        channels: vec![
            Channel {
                id: "state.altitude".into(),
                kind: ChannelKind::Scalar,
                track: TrackKind::Estimated,
                time_base_id: domain.into(),
                frame_id: Some("launch-enu".into()),
                unit: Unit::Meter,
                samples: times
                    .iter()
                    .zip(altitude)
                    .map(|(&time, &value)| Sample {
                        time,
                        values: vec![value],
                        valid: true,
                    })
                    .collect(),
                uncertainty: Some(Uncertainty {
                    representation: "one_sigma".into(),
                    values: vec![2.0],
                    unit: Unit::Meter,
                }),
                parent_hashes: vec![hash(1)],
            },
            Channel {
                id: "state.attitude".into(),
                kind: ChannelKind::AttitudeQuaternionWxyz,
                track: TrackKind::Estimated,
                time_base_id: domain.into(),
                frame_id: Some("launch-enu".into()),
                unit: Unit::Dimensionless,
                samples: times
                    .iter()
                    .zip(yaw_rad)
                    .map(|(&time, &angle)| Sample {
                        time,
                        values: vec![(angle / 2.0).cos(), 0.0, 0.0, (angle / 2.0).sin()],
                        valid: true,
                    })
                    .collect(),
                uncertainty: None,
                parent_hashes: vec![hash(1)],
            },
        ],
        events: vec![TypedEvent {
            id: format!("{id}-burnout"),
            event_type: "burnout".into(),
            time_base_id: domain.into(),
            time: times[1],
            detector_id: "fixture".into(),
            detector_version: "1".into(),
            confidence: 1.0,
            source_sample_ranges: vec!["1".into()],
            evidence_hashes: vec![hash(1)],
        }],
    }
}

#[test]
fn phase_reconciliation_pins_residual_metrics_attitude_and_event_delta() {
    let predicted = trace(
        "predicted",
        "simulation",
        &[0.0, 1.0, 2.0],
        &[0.0, 10.0, 20.0],
        &[0.0, 0.0, 0.0],
    );
    let measured = trace(
        "measured",
        "vehicle",
        &[2.0, 3.0, 4.0],
        &[0.0, 9.0, 18.0],
        &[0.0, 0.1, 0.2],
    );
    let alignment = AlignmentArtifact::manual(
        ClockDomain("vehicle".into()),
        ClockDomain("simulation".into()),
        -2.0,
        1.0,
        [2.0, 4.0],
        "fixture events",
        vec![hash(1)],
    )
    .unwrap();
    let result = Reconciler::compare(
        &predicted,
        &measured,
        &alignment,
        &[
            ChannelPair::new("state.altitude", "state.altitude"),
            ChannelPair::new("state.attitude", "state.attitude"),
        ],
        &[
            PhaseWindow::new("powered", 0.0, 1.5).unwrap(),
            PhaseWindow::new("coast", 1.5, 2.1).unwrap(),
        ],
        &[],
    )
    .unwrap();

    let altitude = result
        .channels
        .iter()
        .find(|item| item.quantity == "state.altitude")
        .unwrap();
    assert_eq!(
        altitude
            .residuals
            .iter()
            .map(|sample| sample.value)
            .collect::<Vec<_>>(),
        vec![0.0, 1.0, 2.0]
    );
    assert!((altitude.whole_flight.rmse - (5.0_f64 / 3.0).sqrt()).abs() < 1e-12);
    assert_eq!(altitude.whole_flight.bias, 1.0);
    assert_eq!(altitude.whole_flight.max_absolute, 2.0);
    assert_eq!(
        altitude.normalization_denominator,
        "measured one_sigma = 2 meter"
    );
    let attitude = result
        .channels
        .iter()
        .find(|item| item.quantity == "state.attitude")
        .unwrap();
    assert!((attitude.residuals[1].value - 0.1).abs() < 1e-12);
    assert_eq!(result.event_deltas[0].event_type, "burnout");
    assert_eq!(result.event_deltas[0].delta_s, 0.0);
}

#[test]
fn diagnostics_are_deterministic_ranked_hypotheses_with_evidence() {
    let predicted = trace(
        "predicted",
        "simulation",
        &[0.0, 1.0, 2.0],
        &[0.0, 10.0, 20.0],
        &[0.0; 3],
    );
    let measured = trace(
        "measured",
        "vehicle",
        &[2.0, 3.0, 4.0],
        &[0.0, 9.0, 18.0],
        &[0.0; 3],
    );
    let alignment = AlignmentArtifact::manual(
        ClockDomain("vehicle".into()),
        ClockDomain("simulation".into()),
        -2.0,
        1.0,
        [2.0, 4.0],
        "fixture",
        vec![hash(1)],
    )
    .unwrap();
    let candidates = [
        DiagnosticCandidate {
            name: "drag model bias".into(),
            residual_signature: vec![0.0, 1.0, 2.0],
            evidence_hash: hash(2),
        },
        DiagnosticCandidate {
            name: "clock edge".into(),
            residual_signature: vec![2.0, 1.0, 0.0],
            evidence_hash: hash(3),
        },
    ];
    let result = Reconciler::compare(
        &predicted,
        &measured,
        &alignment,
        &[ChannelPair::new("state.altitude", "state.altitude")],
        &[PhaseWindow::new("flight", 0.0, 2.1).unwrap()],
        &candidates,
    )
    .unwrap();
    assert_eq!(result.hypotheses[0].name, "drag model bias");
    assert!(result.hypotheses[0].label.contains("hypothesis"));
    assert_eq!(result.hypotheses[0].evidence_hash, hash(2));
    assert_eq!(
        result.canonical_bytes().unwrap(),
        result.canonical_bytes().unwrap()
    );
}
