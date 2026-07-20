use ascent_app::{MissionReviewBundle, ReviewBundleInput, StoryBeat};

#[test]
fn review_bundle_is_deterministic_hash_verified_and_reopens_offline() {
    let document = ascent_app::Document::default();
    let input = ReviewBundleInput {
        journal: document.journal_jsonl(),
        alignment: None,
        reconciliation: None,
        story: vec![StoryBeat {
            review_time_s: 0.0,
            camera_preset: "vehicle-overview".into(),
            briefing: "Predicted configuration".into(),
        }],
        qualifications: vec!["prediction only; no measured 6-DOF telemetry selected".into()],
    };
    let first = MissionReviewBundle::build(&document, input.clone()).unwrap();
    let second = MissionReviewBundle::build(&document, input).unwrap();

    assert_eq!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
    assert!(first.files.contains_key("manifest.json"));
    assert!(first.files.contains_key("traces/predicted.json"));
    assert!(first.files.contains_key("report/index.html"));
    assert!(first.files.contains_key("report/normalized.pdf"));
    assert!(first.files.contains_key("geometry/fin-template.svg"));
    assert!(first.files.contains_key("geometry/longitudinal-layout.svg"));
    let pdf = &first.files["report/normalized.pdf"];
    assert!(pdf.starts_with(b"%PDF-1.4\n"));
    let text = std::str::from_utf8(pdf).unwrap();
    let startxref = text
        .split("startxref\n")
        .nth(1)
        .and_then(|tail| tail.lines().next())
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert!(text[startxref..].starts_with("xref\n"));

    // When Poppler is available, validate with an independent PDF reader.
    if std::process::Command::new("pdfinfo")
        .arg("-v")
        .output()
        .is_ok()
    {
        let path = std::env::temp_dir().join(format!(
            "ascent-review-{}-{}.pdf",
            std::process::id(),
            pdf.len()
        ));
        std::fs::write(&path, pdf).unwrap();
        let output = std::process::Command::new("pdfinfo")
            .arg(&path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(path);
        assert!(
            output.status.success(),
            "pdfinfo rejected normalized PDF: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("Pages:"));
    }

    let reopened = MissionReviewBundle::reopen(&first.canonical_bytes().unwrap()).unwrap();
    assert_eq!(
        reopened.document.canonical_bytes(),
        document.canonical_bytes()
    );
    assert_eq!(reopened.review_state.story.len(), 1);
}

#[test]
fn modified_bundle_member_is_rejected() {
    let document = ascent_app::Document::default();
    let mut bundle = MissionReviewBundle::build(
        &document,
        ReviewBundleInput {
            journal: document.journal_jsonl(),
            alignment: None,
            reconciliation: None,
            story: vec![],
            qualifications: vec!["unvalidated prediction".into()],
        },
    )
    .unwrap();
    bundle
        .files
        .insert("report/model.json".into(), b"{}".to_vec());
    assert!(MissionReviewBundle::reopen(&bundle.canonical_bytes().unwrap()).is_err());
}
