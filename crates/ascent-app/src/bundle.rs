use crate::cinema::{CampaignCinema, StoryBeat};
use crate::design::build_flight;
use crate::document::Document;
use crate::evidence::evidence_for_with_atmosphere;
use crate::markers::vehicle_markers;
use crate::report::flight_readiness_markdown;
use ascent_review::alignment::AlignmentArtifact;
use ascent_review::reconciliation::ReconciliationResult;
use ascent_sim::{flight_trace_from_vertical, simulate_vertical, SimConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub const REVIEW_BUNDLE_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewBundleInput {
    pub journal: String,
    pub alignment: Option<AlignmentArtifact>,
    pub reconciliation: Option<ReconciliationResult>,
    #[serde(default)]
    pub story: Vec<StoryBeat>,
    #[serde(default)]
    pub qualifications: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleMember {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewBundleManifest {
    pub bundle_version: u16,
    pub app_version: String,
    pub evidence_schema_version: u16,
    pub document_sha256: String,
    pub journal_sha256: String,
    pub rendering_profile: String,
    pub pdf_normalization: String,
    pub qualifications: Vec<String>,
    pub members: Vec<BundleMember>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionReviewBundle {
    pub files: BTreeMap<String, Vec<u8>>,
}

pub struct ReopenedReview {
    pub document: Document,
    pub review_state: CampaignCinema,
    pub manifest: ReviewBundleManifest,
}

impl MissionReviewBundle {
    pub fn build(document: &Document, input: ReviewBundleInput) -> Result<Self, String> {
        if input.qualifications.is_empty()
            || input
                .qualifications
                .iter()
                .any(|item| item.trim().is_empty())
        {
            return Err("review bundle requires explicit evidence qualifications".into());
        }
        if let Some(result) = &input.reconciliation {
            let selected = input
                .alignment
                .as_ref()
                .ok_or("a reconciliation bundle must include its selected alignment artifact")?;
            if &result.alignment != selected {
                return Err("reconciliation does not reference the selected alignment".into());
            }
        }
        let replayed = Document::replay(&input.journal)?;
        if replayed.canonical_bytes() != document.canonical_bytes() {
            return Err("review journal does not reproduce the supplied document".into());
        }
        let mut cinema = CampaignCinema::from_journal(&input.journal)?;
        cinema.story = input.story.clone();

        let run = crate::design::run_design_with_atmosphere(
            &document.design,
            document.atmosphere.as_ref(),
        )?;
        let (rocket, motor, mut environment) = build_flight(&document.design)?;
        if let Some(model) = document
            .atmosphere
            .as_ref()
            .and_then(ascent_sim::AtmosphereProfile::atmosphere_model)
        {
            environment.atmosphere = model;
        }
        let simulation = simulate_vertical(&rocket, &motor, &environment, &SimConfig::default());
        let trace = flight_trace_from_vertical(&simulation, &run.summary.input_hash)?;
        let evidence =
            evidence_for_with_atmosphere(&document.design, document.atmosphere.as_ref())?;
        let review = crate::review_ipc::report_for(
            &document.vehicle,
            &document.design,
            run.summary.apogee_m,
        )
        .ok();
        let mut markdown = flight_readiness_markdown(document, review.as_ref());

        let mut files = BTreeMap::new();
        insert_json(&mut files, "state/document.json", document)?;
        files.insert(
            "state/journal.jsonl".into(),
            input.journal.as_bytes().to_vec(),
        );
        insert_json(&mut files, "state/review.json", &cinema)?;
        insert_json(&mut files, "traces/predicted.json", &trace)?;
        insert_json(&mut files, "events/events.json", &trace.events)?;
        insert_json(&mut files, "evidence/credibility.json", &evidence)?;
        let validation_cases = [
            (
                "analytic-constant-thrust",
                include_bytes!("../../../data/validation/cases/analytic-constant-thrust.json")
                    .as_slice(),
            ),
            (
                "openrocket-24_12",
                include_bytes!("../../../data/validation/cases/openrocket-24_12.json").as_slice(),
            ),
            (
                "rocketpy-cross-validation",
                include_bytes!("../../../data/validation/cases/rocketpy-cross-validation.json")
                    .as_slice(),
            ),
            (
                "valkyrie-2025-flight",
                include_bytes!("../../../data/validation/cases/valkyrie-2025-flight.json")
                    .as_slice(),
            ),
        ];
        let mut validation_summary = Vec::new();
        for (id, bytes) in validation_cases {
            let case = ascent_review::validation::ValidationCase::from_canonical_bytes(bytes)?;
            let case_hash = case.case_hash()?;
            validation_summary.push(json!({
                "case_id": case.case_id,
                "evidence_level": case.evidence_level,
                "intended_use": case.intended_use,
                "caveats": case.caveats,
                "known_mismatches": case.known_mismatches,
                "case_hash": case_hash,
            }));
            files.insert(format!("evidence/validation/{id}.json"), bytes.to_vec());
        }
        if let Some(alignment) = &input.alignment {
            alignment.validate()?;
            insert_json(&mut files, "alignment/selected.json", alignment)?;
        }
        if let Some(reconciliation) = &input.reconciliation {
            insert_json(&mut files, "reconciliation/result.json", reconciliation)?;
        }
        markdown.push_str("\n## Evidence qualifications\n\n");
        for qualification in &input.qualifications {
            let _ = writeln!(markdown, "- {qualification}");
        }
        markdown.push_str("\n## Named validation cases\n\n");
        for case in &validation_summary {
            let _ = writeln!(
                markdown,
                "- {} · level {} · case `{}`",
                case["case_id"].as_str().unwrap_or("unknown"),
                case["evidence_level"].as_str().unwrap_or("unknown"),
                case["case_hash"].as_str().unwrap_or("unknown")
            );
            for caveat in case["caveats"].as_array().into_iter().flatten() {
                if let Some(caveat) = caveat.as_str() {
                    let _ = writeln!(markdown, "  - Caveat: {caveat}");
                }
            }
            for mismatch in case["known_mismatches"].as_array().into_iter().flatten() {
                if let Some(mismatch) = mismatch.as_str() {
                    let _ = writeln!(markdown, "  - Known mismatch: {mismatch}");
                }
            }
        }
        let report_model = json!({
            "schema_version": 1,
            "document_hash": hash(document.canonical_bytes().as_bytes()),
            "run_input_hash": run.summary.input_hash,
            "trace_id": trace.trace_id,
            "alignment_algorithm": input.alignment.as_ref().map(|a| format!("{}@{}", a.algorithm, a.algorithm_version)),
            "event_detectors": trace.events.iter().map(|event| format!("{}@{}", event.detector_id, event.detector_version)).collect::<Vec<_>>(),
            "evidence_level": evidence.validation,
            "validation_cases": validation_summary,
            "qualifications": input.qualifications,
            "rendering": {"profile": "ascent-offline-review-v1", "units": "SI"},
            "markdown": markdown,
        });
        insert_json(&mut files, "report/model.json", &report_model)?;
        files.insert(
            "report/index.html".into(),
            standalone_html(&report_model).into_bytes(),
        );
        files.insert(
            "report/normalized.pdf".into(),
            normalized_pdf(&report_model),
        );
        files.insert(
            "geometry/fin-template.svg".into(),
            fin_template_svg(document)?.into_bytes(),
        );
        files.insert(
            "geometry/longitudinal-layout.svg".into(),
            longitudinal_svg(document)?.into_bytes(),
        );

        let members = files
            .iter()
            .map(|(path, bytes)| BundleMember {
                path: path.clone(),
                sha256: hash(bytes),
                bytes: bytes.len(),
                role: member_role(path).into(),
            })
            .collect();
        let qualifications = report_model["qualifications"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let manifest = ReviewBundleManifest {
            bundle_version: REVIEW_BUNDLE_VERSION,
            app_version: env!("CARGO_PKG_VERSION").into(),
            evidence_schema_version: ascent_domain::evidence::EVIDENCE_SCHEMA_VERSION,
            document_sha256: hash(document.canonical_bytes().as_bytes()),
            journal_sha256: hash(input.journal.as_bytes()),
            rendering_profile: "ascent-offline-review-v1".into(),
            pdf_normalization: "fixed producer, no timestamps, stable object order".into(),
            qualifications,
            members,
        };
        insert_json(&mut files, "manifest.json", &manifest)?;
        Ok(Self { files })
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|error| error.to_string())
    }

    pub fn reopen(bytes: &[u8]) -> Result<ReopenedReview, String> {
        let bundle: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid review bundle container: {error}"))?;
        let manifest: ReviewBundleManifest = read_json(&bundle.files, "manifest.json")?;
        if manifest.bundle_version != REVIEW_BUNDLE_VERSION {
            return Err(format!(
                "unsupported review bundle version {}",
                manifest.bundle_version
            ));
        }
        for member in &manifest.members {
            let bytes = bundle
                .files
                .get(&member.path)
                .ok_or_else(|| format!("bundle member {} is missing", member.path))?;
            if bytes.len() != member.bytes || hash(bytes) != member.sha256 {
                return Err(format!(
                    "bundle member {} failed SHA-256 verification",
                    member.path
                ));
            }
        }
        let document: Document = read_json(&bundle.files, "state/document.json")?;
        if hash(document.canonical_bytes().as_bytes()) != manifest.document_sha256 {
            return Err("reopened document does not match manifest hash".into());
        }
        let journal = std::str::from_utf8(
            bundle
                .files
                .get("state/journal.jsonl")
                .ok_or("bundle journal is missing")?,
        )
        .map_err(|error| format!("bundle journal is not UTF-8: {error}"))?;
        if hash(journal.as_bytes()) != manifest.journal_sha256
            || Document::replay(journal)?.canonical_bytes() != document.canonical_bytes()
        {
            return Err("bundle journal does not reproduce the saved document".into());
        }
        let review_state: CampaignCinema = read_json(&bundle.files, "state/review.json")?;
        if review_state.final_document_bytes != document.canonical_bytes() {
            return Err("saved review state does not target the reopened document".into());
        }
        Ok(ReopenedReview {
            document,
            review_state,
            manifest,
        })
    }
}

fn insert_json<T: Serialize>(
    files: &mut BTreeMap<String, Vec<u8>>,
    path: &str,
    value: &T,
) -> Result<(), String> {
    files.insert(
        path.into(),
        serde_json::to_vec(value).map_err(|error| error.to_string())?,
    );
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(
    files: &BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<T, String> {
    serde_json::from_slice(
        files
            .get(path)
            .ok_or_else(|| format!("bundle member {path} is missing"))?,
    )
    .map_err(|error| format!("invalid {path}: {error}"))
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn member_role(path: &str) -> &'static str {
    match path.split('/').next().unwrap_or("") {
        "state" => "reopen-state",
        "traces" => "flight-trace",
        "events" => "typed-events",
        "alignment" => "clock-alignment",
        "reconciliation" => "residual-metrics",
        "evidence" => "credibility",
        "report" => "offline-report",
        "geometry" => "dimensioned-geometry",
        _ => "supporting-artifact",
    }
}

fn standalone_html(model: &Value) -> String {
    let json = serde_json::to_string(model)
        .expect("model serializes")
        .replace('&', "\\u0026")
        .replace('<', "\\u003c");
    format!("<!doctype html>\n<meta charset=\"utf-8\"><title>Ascent Mission Review</title><style>body{{font:14px system-ui;max-width:1100px;margin:2rem auto;background:#101418;color:#e8edf2}}pre{{white-space:pre-wrap}}</style><h1>Ascent Mission Review</h1><div id=\"qualifications\"></div><pre id=\"report\"></pre><script>const model={json};document.getElementById('qualifications').textContent='Qualifications: '+model.qualifications.join('; ');document.getElementById('report').textContent=model.markdown;</script>\n")
}

fn normalized_pdf(model: &Value) -> Vec<u8> {
    const LINES_PER_PAGE: usize = 48;
    let mut lines = model["markdown"]
        .as_str()
        .unwrap_or("Ascent Mission Review")
        .lines()
        .map(pdf_literal)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push("Ascent Mission Review".into());
    }
    let pages = lines.chunks(LINES_PER_PAGE).collect::<Vec<_>>();
    let font_id = 3 + pages.len() * 2;
    let kids = (0..pages.len())
        .map(|index| format!("{} 0 R", 3 + index * 2))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        (1usize, "<</Type/Catalog/Pages 2 0 R>>".into()),
        (
            2usize,
            format!("<</Type/Pages/Count {}/Kids[{}]>>", pages.len(), kids),
        ),
    ];
    for (index, page_lines) in pages.iter().enumerate() {
        let page_id = 3 + index * 2;
        let content_id = page_id + 1;
        let mut stream = String::from("BT /F1 9 Tf 42 760 Td 12 TL\n");
        for line in *page_lines {
            let _ = writeln!(stream, "({line}) Tj T*");
        }
        stream.push_str("ET\n");
        objects.push((
            page_id,
            format!("<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]/Contents {content_id} 0 R/Resources<</Font<</F1 {font_id} 0 R>>>>>>"),
        ));
        objects.push((
            content_id,
            format!("<</Length {}>>\nstream\n{}endstream", stream.len(), stream),
        ));
    }
    objects.push((
        font_id,
        "<</Type/Font/Subtype/Type1/BaseFont/Helvetica/Encoding/WinAnsiEncoding>>".into(),
    ));
    objects.sort_by_key(|(id, _)| *id);

    let mut pdf = b"%PDF-1.4\n% Ascent normalized deterministic PDF v1\n".to_vec();
    let mut offsets = vec![0usize; font_id + 1];
    for (id, body) in objects {
        offsets[id] = pdf.len();
        pdf.extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", font_id + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{}\n%%EOF\n",
            font_id + 1,
            xref_offset
        )
        .as_bytes(),
    );
    pdf
}

fn pdf_literal(line: &str) -> String {
    line.chars()
        .map(|character| {
            if character.is_ascii() && !character.is_ascii_control() {
                character
            } else {
                '?'
            }
        })
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn fin_template_svg(document: &Document) -> Result<String, String> {
    let fin = document
        .vehicle
        .parts
        .iter()
        .flat_map(|p| &p.children)
        .find_map(|p| match &p.kind {
            ascent_domain::vehicle::PartKind::FinSet {
                root_chord_m,
                tip_chord_m,
                span_m,
                sweep_m,
                ..
            } => Some((*root_chord_m, *tip_chord_m, *span_m, *sweep_m)),
            _ => None,
        })
        .ok_or("dimensioned fin template requires a fin set")?;
    Ok(format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 800 500\"><title>Dimensioned fin template, SI units</title><metadata>{{\"document_sha256\":\"{}\",\"rendering_profile\":\"ascent-offline-review-v1\",\"units\":\"SI\"}}</metadata><path d=\"M100 400 L{} 400 L{} 100 L{} 100 Z\" fill=\"none\" stroke=\"black\"/><text x=\"100\" y=\"440\">root {:.6} m</text><text x=\"100\" y=\"70\">tip {:.6} m; span {:.6} m; sweep {:.6} m</text></svg>\n", hash(document.canonical_bytes().as_bytes()), 100.0 + fin.0 * 5000.0, 100.0 + (fin.3 + fin.1) * 5000.0, 100.0 + fin.3 * 5000.0, fin.0, fin.1, fin.2, fin.3))
}

fn longitudinal_svg(document: &Document) -> Result<String, String> {
    let markers = vehicle_markers(&document.vehicle, &document.design)?;
    let mut svg = String::new();
    let cg = markers.cg_ignition_from_nose_m;
    let cp = markers.cp_from_nose_m;
    writeln!(&mut svg, "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 240\"><title>Longitudinal CG CP layout</title><metadata>{{\"document_sha256\":\"{}\",\"rendering_profile\":\"ascent-offline-review-v1\",\"units\":\"SI\"}}</metadata><line x1=\"50\" y1=\"120\" x2=\"950\" y2=\"120\" stroke=\"black\"/><line x1=\"{}\" y1=\"60\" x2=\"{}\" y2=\"180\" stroke=\"blue\"/><text x=\"{}\" y=\"45\">CG {:.6} m</text><line x1=\"{}\" y1=\"60\" x2=\"{}\" y2=\"180\" stroke=\"red\"/><text x=\"{}\" y=\"205\">CP {:.6} m; stability {:.6} cal</text></svg>", hash(document.canonical_bytes().as_bytes()), 50.0 + cg * 800.0, 50.0 + cg * 800.0, 50.0 + cg * 800.0, cg, 50.0 + cp * 800.0, 50.0 + cp * 800.0, 50.0 + cp * 800.0, cp, markers.stability_ignition_cal).map_err(|e| e.to_string())?;
    Ok(svg)
}
