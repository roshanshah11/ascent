use ascent_app::{CampaignCinema, Command, DecisionMetadata, Document, StoryBeat};
use serde_json::json;

fn decision(intent: &str) -> DecisionMetadata {
    DecisionMetadata {
        author: "gnc-agent".into(),
        source: "external-agent-command-seam".into(),
        intent: intent.into(),
        affected_requirements: vec!["GNC-TRAJ-004".into()],
        evidence_hashes: vec!["11".repeat(32)],
    }
}

#[test]
fn campaign_checkpoints_equal_prefix_replay_and_final_ordinary_replay() {
    let mut doc = Document::default();
    doc.dispatch_with_metadata(
        Command::SetSimParam {
            param: "cd".into(),
            value: json!(0.65),
        },
        decision("update drag from reconciliation"),
    )
    .unwrap();
    doc.dispatch_with_metadata(
        Command::Batch {
            commands: vec![
                Command::SetSimParam {
                    param: "rail_length_m".into(),
                    value: json!(1.5),
                },
                Command::SelectMotor {
                    designation: "B6".into(),
                },
            ],
        },
        decision("approve counterfactual batch"),
    )
    .unwrap();
    doc.undo();
    doc.redo();
    let journal = doc.journal_jsonl();
    let cinema = CampaignCinema::from_journal(&journal).unwrap();

    assert_eq!(
        cinema.final_document_bytes,
        Document::replay(&journal).unwrap().canonical_bytes()
    );
    for checkpoint in &cinema.checkpoints {
        assert_eq!(
            checkpoint.document_bytes,
            Document::replay(&checkpoint.journal_prefix)
                .unwrap()
                .canonical_bytes()
        );
    }
    assert_eq!(cinema.checkpoints[0].author, "gnc-agent");
    assert_eq!(
        cinema.checkpoints[0].affected_requirements,
        vec!["GNC-TRAJ-004"]
    );
    assert!(cinema.checkpoints[1]
        .canonical_command
        .starts_with("batch "));
}

#[test]
fn cinema_is_read_only_branchable_and_story_tracks_are_presentation_only() {
    let mut doc = Document::default();
    doc.dispatch_with_metadata(
        Command::SetSimParam {
            param: "cd".into(),
            value: json!(0.7),
        },
        decision("first"),
    )
    .unwrap();
    let journal = doc.journal_jsonl();
    let mut cinema = CampaignCinema::from_journal(&journal).unwrap();
    cinema.story.push(StoryBeat {
        review_time_s: 1.0,
        camera_preset: "powered-ascent-residual".into(),
        briefing: "Inspect divergence".into(),
    });
    assert_eq!(
        journal,
        doc.journal_jsonl(),
        "cinema creation and story edits never append source journal"
    );

    let branch = cinema
        .branch_from(
            0,
            Command::SetSimParam {
                param: "cd".into(),
                value: json!(0.6),
            },
            decision("branch correction"),
        )
        .unwrap();
    assert!(
        branch.journal_jsonl().lines().count()
            > cinema.checkpoints[0].journal_prefix.lines().count()
    );
    assert_eq!(journal, doc.journal_jsonl());
}

#[test]
fn corrupt_journal_stops_at_exact_line() {
    let error = CampaignCinema::from_journal(
        "{\"journal_version\":1}\n{\"op\":\"dispatch\",\"command\":{\"cmd\":\"warp\"}}\n",
    )
    .unwrap_err();
    assert!(error.contains("line 2"), "{error}");
}
