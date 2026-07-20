use ascent_app::{
    build_counterfactual_review, CampaignCinema, Command, DecisionMetadata, Document,
};
use ascent_domain::evidence::{
    Channel, ChannelKind, ClockDomain, Sample, TrackKind, TypedEvent, Unit,
};
use ascent_domain::telemetry::{builtin_schema, BuiltinSchema, TelemetryImporter};
use ascent_review::alignment::AlignmentArtifact;
use ascent_review::reconciliation::{ChannelPair, PhaseWindow, Reconciler};
use serde_json::json;

fn decision(intent: &str, evidence_hashes: Vec<String>) -> DecisionMetadata {
    DecisionMetadata {
        author: "campaign.gnc-agent".into(),
        source: "external-agent-command-seam".into(),
        intent: intent.into(),
        affected_requirements: vec!["GNC-FLIGHT-REVIEW-006".into()],
        evidence_hashes,
    }
}

#[test]
fn golden_campaign_replays_predict_import_align_reconcile_preview_approve_and_reverify() {
    let raw = b"time_s,altitude_m\n0,0\n1,10\n2,20\n3,10\n";
    let mut bundle =
        TelemetryImporter::ingest(&builtin_schema(BuiltinSchema::BlueRavenCsv), raw).unwrap();
    let raw_hash = bundle.sources[0].sha256.clone();
    bundle.trace.events.push(TypedEvent {
        id: "measured-apogee".into(),
        event_type: "apogee".into(),
        time_base_id: "device-elapsed".into(),
        time: 2.0,
        detector_id: "campaign.barometric-apogee".into(),
        detector_version: "1".into(),
        confidence: 0.95,
        source_sample_ranges: vec!["CSV lines 3-5".into()],
        evidence_hashes: vec![raw_hash.clone()],
    });
    bundle.validate().unwrap();

    let alignment = AlignmentArtifact::manual(
        ClockDomain("device-elapsed".into()),
        ClockDomain("simulation-elapsed".into()),
        0.0,
        1.0,
        [0.0, 3.0],
        "launch and apogee event agreement",
        vec![raw_hash.clone()],
    )
    .unwrap();

    let mut predicted = bundle.trace.clone();
    predicted.trace_id = "campaign-predicted-6dof-projection".into();
    predicted.time_bases[0].id = "simulation-elapsed".into();
    predicted.time_bases[0].system = ascent_domain::evidence::TimeSystem::SimulationElapsed;
    predicted.channels = vec![Channel {
        id: "truth.altitude".into(),
        kind: ChannelKind::Scalar,
        track: TrackKind::Simulated,
        time_base_id: "simulation-elapsed".into(),
        frame_id: Some("launch-enu".into()),
        unit: Unit::Meter,
        samples: [0.0, 9.0, 19.0, 9.5]
            .into_iter()
            .enumerate()
            .map(|(time, value)| Sample {
                time: time as f64,
                values: vec![value],
                valid: true,
            })
            .collect(),
        uncertainty: None,
        parent_hashes: vec!["22".repeat(32)],
    }];
    predicted.events = vec![TypedEvent {
        id: "predicted-apogee".into(),
        event_type: "apogee".into(),
        time_base_id: "simulation-elapsed".into(),
        time: 2.0,
        detector_id: "campaign.sim-apogee".into(),
        detector_version: "1".into(),
        confidence: 1.0,
        source_sample_ranges: vec!["simulation samples 2-4".into()],
        evidence_hashes: vec!["22".repeat(32)],
    }];
    predicted.validate().unwrap();
    let phases = vec![
        PhaseWindow::new("powered ascent", 0.0, 1.5).unwrap(),
        PhaseWindow::new("coast and descent", 1.5, 3.1).unwrap(),
    ];
    let reconciliation = Reconciler::compare(
        &predicted,
        &bundle.trace,
        &alignment,
        &[ChannelPair::new("truth.altitude", "barometry.altitude")],
        &phases,
        &[],
    )
    .unwrap();

    let mut document = Document::default();
    document
        .dispatch_with_metadata(
            Command::SetSimParam {
                param: "cd".into(),
                value: json!(0.61),
            },
            decision(
                "freeze predicted 6-DOF configuration",
                vec!["22".repeat(32)],
            ),
        )
        .unwrap();
    document
        .dispatch_with_metadata(
            Command::SetTelemetry {
                bundles: vec![bundle],
            },
            decision(
                "import immutable measured avionics evidence",
                vec![raw_hash.clone()],
            ),
        )
        .unwrap();
    document
        .dispatch_with_metadata(
            Command::SetAlignment {
                alignment: Some(alignment),
            },
            decision(
                "approve measured-to-simulation clock alignment",
                vec![raw_hash.clone()],
            ),
        )
        .unwrap();
    document
        .dispatch_with_metadata(
            Command::SetReconciliation {
                reconciliation: Some(reconciliation.clone()),
            },
            decision(
                "reconcile predicted and measured flight by phase",
                vec![raw_hash.clone()],
            ),
        )
        .unwrap();

    let journal_before_preview = document.journal_jsonl();
    let proposal = vec![Command::SetSimParam {
        param: "cd".into(),
        value: json!(0.58),
    }];
    let universe = build_counterfactual_review(&document, &proposal).unwrap();
    assert_eq!(document.journal_jsonl(), journal_before_preview);
    assert_ne!(
        universe.baseline_document_hash,
        universe.proposed_document_hash
    );
    assert!(universe
        .proposed_trace
        .channels
        .iter()
        .any(|channel| channel.id == "truth.attitude"));

    document
        .dispatch_with_metadata(
            Command::Batch { commands: proposal },
            decision(
                "approve counterfactual correction as one batch",
                vec![raw_hash.clone()],
            ),
        )
        .unwrap();
    document
        .dispatch_with_metadata(
            Command::SetReconciliation {
                reconciliation: Some(reconciliation),
            },
            decision(
                "re-verify accepted configuration with frozen criteria",
                vec![raw_hash],
            ),
        )
        .unwrap();

    let journal = document.journal_jsonl();
    let cinema = CampaignCinema::from_journal(&journal).unwrap();
    assert_eq!(cinema.checkpoints.len(), 6);
    assert_eq!(cinema.final_document_bytes, document.canonical_bytes());
    assert_eq!(
        Document::replay(&journal).unwrap().canonical_bytes(),
        document.canonical_bytes()
    );
    let intents = cinema
        .checkpoints
        .iter()
        .map(|checkpoint| checkpoint.intent.as_str())
        .collect::<Vec<_>>();
    for required in [
        "predicted",
        "import",
        "alignment",
        "reconcile",
        "counterfactual",
        "re-verify",
    ] {
        assert!(
            intents.iter().any(|intent| intent.contains(required)),
            "missing campaign beat {required}: {intents:?}"
        );
    }
}
