//! Flight-readiness report (v0.5 Step 6): a deterministic Markdown
//! artifact synthesized from the document's stored studies plus an
//! optional structural/rule review. Two disciplines make it trustworthy:
//!
//! - **Provenance.** Every performance number is drawn from a study's
//!   stored results and stamped with that study's input hash. A reader can
//!   tie any figure back to the exact (vehicle, design, atmosphere, study)
//!   that produced it — the same content hash the job runner records.
//! - **Honesty about staleness.** A study whose stored hash no longer
//!   matches the current inputs renders `STALE`, showing both hashes, so
//!   the report never presents an out-of-date number as current.
//!
//! The generator is a pure function of its inputs: no clock, no
//! randomness, iteration in document order. Same inputs → byte-identical
//! Markdown, so the report itself is a determinism artifact.
//!
//! Structural margins and rule compliance come from an optional
//! [`ReviewReport`]. When it is absent (Codex's structural pack has not
//! run yet, or the caller chose not to compute it), those sections
//! degrade to an explicit "not yet computed" line rather than inventing
//! numbers or omitting the section silently.

use crate::document::Document;
use crate::review_ipc::ReviewReport;
use crate::study::{study_input_hash, Study, StudyKind};
use std::fmt::Write as _;

/// Provenance status of a study's stored results against current inputs.
enum Freshness {
    /// No results stored — the study has never run.
    NotRun,
    /// Stored hash matches current inputs.
    Current(String),
    /// Stored hash no longer matches (stored, current).
    Stale(String, String),
}

fn freshness(doc: &Document, study: &Study) -> Freshness {
    let current = study_input_hash(&doc.vehicle, &doc.design, doc.atmosphere.as_ref(), study);
    match &study.results {
        None => Freshness::NotRun,
        Some(r) if r.input_hash == current => Freshness::Current(r.input_hash.clone()),
        Some(r) => Freshness::Stale(r.input_hash.clone(), current),
    }
}

/// First 12 hex chars — enough to identify a hash in prose without the
/// full 64-char string dominating the line.
fn short(hash: &str) -> &str {
    &hash[..hash.len().min(12)]
}

fn kind_label(kind: &StudyKind) -> String {
    match kind {
        StudyKind::SingleFlight => "single flight".into(),
        StudyKind::Dispersion { flights } => format!("dispersion, {flights} flights"),
        StudyKind::MotorTrade { candidates } => {
            format!("motor trade ({} candidates)", candidates.len())
        }
        StudyKind::StabilitySweep {
            param,
            from,
            to,
            steps,
        } => {
            format!("stability sweep of {param}: {from} → {to} in {steps}")
        }
    }
}

/// Pull a finite f64 field out of a study's result payload; `None` when
/// absent or non-finite so the report can say "—" instead of "NaN".
fn num(data: &serde_json::Value, key: &str) -> Option<f64> {
    data.get(key)
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite())
}

fn fmt_m(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{v:.1} m"),
        None => "—".into(),
    }
}

/// Render the description of the design's recovery configuration.
fn recovery_line(doc: &Document) -> String {
    let chute = &doc.design.chute;
    if !chute.enabled {
        return "none (ballistic descent)".into();
    }
    match chute.main_deploy_altitude_m {
        None => format!(
            "single-deploy · {:.0} cm main @ Cd {}",
            chute.diameter_cm, chute.cd
        ),
        Some(deploy_m) => {
            let drogue = match (chute.drogue_diameter_cm, chute.drogue_cd) {
                (Some(d), Some(cd)) => format!("{d:.0} cm drogue @ Cd {cd} at apogee"),
                _ => "free descent".into(),
            };
            format!(
                "dual-deploy · {drogue}, {:.0} cm main @ Cd {} at {deploy_m:.0} m",
                chute.diameter_cm, chute.cd
            )
        }
    }
}

/// Build the flight-readiness report as Markdown. `review` supplies
/// structural margins and rule compliance; pass `None` to render those
/// sections as not-yet-computed.
pub fn flight_readiness_markdown(doc: &Document, review: Option<&ReviewReport>) -> String {
    let mut out = String::new();
    let d = &doc.design;

    let _ = writeln!(out, "# Flight-Readiness Report: {}", d.name);
    out.push('\n');

    // ---- Configuration ----
    let _ = writeln!(out, "## Configuration");
    out.push('\n');
    let _ = writeln!(out, "- Motor: {}", d.motor_designation);
    let _ = writeln!(out, "- Dry mass: {:.1} g", d.dry_mass_g);
    let _ = writeln!(out, "- Diameter: {:.1} mm", d.diameter_mm);
    let _ = writeln!(out, "- Drag coefficient: {}", d.cd);
    let _ = writeln!(out, "- Rail length: {:.2} m", d.rail_length_m);
    let _ = writeln!(out, "- Recovery: {}", recovery_line(doc));
    let atmosphere = match &doc.atmosphere {
        Some(profile) => format!(
            "imported profile \"{}\" ({} layers)",
            profile.name,
            profile.layers.len()
        ),
        None => "1976 US Standard Atmosphere (analytic)".into(),
    };
    let _ = writeln!(out, "- Atmosphere: {atmosphere}");
    out.push('\n');

    // ---- Studies ----
    let _ = writeln!(out, "## Studies");
    out.push('\n');
    if doc.studies.is_empty() {
        let _ = writeln!(
            out,
            "No studies defined. Create and run a study to populate performance numbers."
        );
        out.push('\n');
    }
    let mut any_stale = false;
    let mut any_not_run = false;
    for study in &doc.studies {
        let _ = writeln!(out, "### {} — {}", study.name, kind_label(&study.kind));
        let _ = writeln!(out, "engine `{}` · seed `{}`", study.engine, study.seed);
        out.push('\n');
        match freshness(doc, study) {
            Freshness::NotRun => {
                any_not_run = true;
                let _ = writeln!(out, "- Status: **NOT RUN** — no stored results.");
            }
            Freshness::Current(hash) => {
                let _ = writeln!(out, "- Status: **CURRENT** · provenance `{}`", short(&hash));
            }
            Freshness::Stale(stored, current) => {
                any_stale = true;
                let _ = writeln!(
                    out,
                    "- Status: **STALE** · stored `{}` ≠ current `{}` — re-run before trusting these numbers.",
                    short(&stored),
                    short(&current)
                );
            }
        }
        if let Some(results) = &study.results {
            render_result_numbers(&mut out, study, &results.data);
        }
        out.push('\n');
    }

    // ---- Structural margins ----
    let _ = writeln!(out, "## Structural Margins");
    out.push('\n');
    match review {
        None => {
            let _ = writeln!(out, "Not yet computed — run a structural review to populate fin-flutter and airframe margins.");
        }
        Some(report) if report.review.structural_checks.is_empty() => {
            let _ = writeln!(
                out,
                "No structural checks available for this configuration."
            );
        }
        Some(report) => {
            let _ = writeln!(out, "| Check | Value | Limit | Margin | Status | Source |");
            let _ = writeln!(out, "|---|---|---|---|---|---|");
            for check in &report.review.structural_checks {
                let _ = writeln!(
                    out,
                    "| {} | {:.3} {} | {:.3} {} | {:+.3} | {} | {} |",
                    check.label,
                    check.value,
                    check.units,
                    check.limit,
                    check.units,
                    check.margin,
                    if check.pass { "PASS" } else { "**FAIL**" },
                    check.source,
                );
            }
        }
    }
    out.push('\n');

    // ---- Rule compliance ----
    let _ = writeln!(out, "## Rule Compliance");
    out.push('\n');
    match review {
        None => {
            let _ = writeln!(
                out,
                "Not yet computed — run a flight review against the rule pack."
            );
        }
        Some(report) => {
            let _ = writeln!(out, "| Rule | Measured | Required | Status | Citation |");
            let _ = writeln!(out, "|---|---|---|---|---|");
            for check in &report.review.checks {
                let _ = writeln!(
                    out,
                    "| {} | {:.3} | {:.3} | {} | {} |",
                    check.description,
                    check.measured,
                    check.required,
                    if check.pass { "PASS" } else { "**FAIL**" },
                    check.citation,
                );
            }
            let _ = writeln!(
                out,
                "\nTarget apogee {:.1} m (±{:.1} m): {}. Mission feasible: {}.",
                report.target_apogee_m,
                report.target_tolerance_m,
                if report.target_met {
                    "met"
                } else {
                    "**missed**"
                },
                if report.mission_feasible {
                    "yes"
                } else {
                    "**no**"
                },
            );
        }
    }
    out.push('\n');

    // ---- Verdict ----
    let _ = writeln!(out, "## Verdict");
    out.push('\n');
    let mut blockers: Vec<String> = Vec::new();
    if doc.studies.is_empty() {
        blockers.push("no studies have been run".into());
    }
    if any_stale {
        blockers.push("one or more studies are stale".into());
    }
    if any_not_run {
        blockers.push("one or more studies have not been run".into());
    }
    match review {
        None => blockers.push("structural and rule review not computed".into()),
        Some(report) => {
            if !report.review.feasible {
                blockers.push("review rules do not all pass".into());
            }
            if !report.target_met {
                blockers.push("target apogee not met".into());
            }
        }
    }
    if blockers.is_empty() {
        let _ = writeln!(
            out,
            "**FLIGHT READY** — all studies current, review passes, target met."
        );
    } else {
        let _ = writeln!(out, "**NOT READY.** Blockers:");
        for blocker in &blockers {
            let _ = writeln!(out, "- {blocker}");
        }
    }

    out
}

/// Extract and render the headline numbers from a study's stored payload,
/// dispatching on kind. Unknown shapes render nothing extra rather than
/// dumping raw JSON.
fn render_result_numbers(out: &mut String, study: &Study, data: &serde_json::Value) {
    match &study.kind {
        StudyKind::Dispersion { .. } => {
            let _ = writeln!(
                out,
                "- Apogee p5 / p50 / p95: {} / {} / {}",
                fmt_m(num(data, "apogee_p5_m")),
                fmt_m(num(data, "apogee_p50_m")),
                fmt_m(num(data, "apogee_p95_m")),
            );
            let _ = writeln!(
                out,
                "- Mean landing range: {}",
                fmt_m(num(data, "landing_mean_m"))
            );
            if let Some(ellipse) = data.get("landing_ellipse") {
                let _ = writeln!(
                    out,
                    "- Landing ellipse (2σ): downrange {}, crossrange {}",
                    fmt_m(num(ellipse, "a_m")),
                    fmt_m(num(ellipse, "b_m")),
                );
            }
        }
        StudyKind::SingleFlight => {
            if let Some(apogee) = num(data, "apogee_m") {
                let _ = writeln!(out, "- Apogee: {}", fmt_m(Some(apogee)));
            }
            if let Some(landing) = num(data, "landing_velocity_ms") {
                let _ = writeln!(out, "- Landing velocity: {landing:.1} m/s");
            }
        }
        // Motor trades and sweeps carry richer payloads; the report links
        // to their provenance without flattening every candidate here.
        StudyKind::MotorTrade { .. } | StudyKind::StabilitySweep { .. } => {}
    }
}

/// A minimal deterministic HTML wrapper around the Markdown body: the
/// Markdown is embedded verbatim inside a `<pre>` so the artifact renders
/// in a browser without a Markdown engine and without reordering. Kept
/// deliberately simple — rich rendering is the frontend's job.
pub fn flight_readiness_html(doc: &Document, review: Option<&ReviewReport>) -> String {
    let markdown = flight_readiness_markdown(doc, review);
    let escaped = markdown
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>Flight-Readiness Report</title></head>\n<body><pre>{escaped}</pre></body></html>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::study::{StudyId, StudyKind, StudyResults};
    use serde_json::json;

    fn doc_with_dispersion_results(current: bool) -> Document {
        let mut doc = Document::default();
        doc.dispatch(Command::CreateStudy {
            name: "landing spread".into(),
            kind: StudyKind::Dispersion { flights: 500 },
            engine: "native".into(),
            seed: 7,
        })
        .unwrap();
        let hash = if current {
            study_input_hash(
                &doc.vehicle,
                &doc.design,
                doc.atmosphere.as_ref(),
                &doc.studies[0],
            )
        } else {
            "0000000000000000stale".into()
        };
        doc.dispatch(Command::SetStudyResults {
            id: StudyId(1),
            results: Some(StudyResults {
                input_hash: hash,
                data: json!({
                    "apogee_p5_m": 340.0,
                    "apogee_p50_m": 360.0,
                    "apogee_p95_m": 380.0,
                    "landing_mean_m": 25.0,
                    "landing_ellipse": { "a_m": 12.0, "b_m": 0.0, "bearing_deg": 0.0 },
                }),
            }),
        })
        .unwrap();
        doc
    }

    #[test]
    fn report_is_deterministic() {
        let doc = doc_with_dispersion_results(true);
        let a = flight_readiness_markdown(&doc, None);
        let b = flight_readiness_markdown(&doc, None);
        assert_eq!(a, b, "same inputs must produce byte-identical Markdown");
    }

    #[test]
    fn current_study_numbers_are_hash_linked() {
        let doc = doc_with_dispersion_results(true);
        let md = flight_readiness_markdown(&doc, None);
        let hash = study_input_hash(
            &doc.vehicle,
            &doc.design,
            doc.atmosphere.as_ref(),
            &doc.studies[0],
        );
        assert!(
            md.contains("**CURRENT**"),
            "current study must be marked current"
        );
        assert!(
            md.contains(short(&hash)),
            "the provenance hash must appear: {md}"
        );
        assert!(md.contains("360.0 m"), "p50 apogee must render");
        assert!(md.contains("Landing ellipse"), "ellipse must render");
    }

    #[test]
    fn stale_study_is_flagged_with_both_hashes() {
        let doc = doc_with_dispersion_results(false);
        let md = flight_readiness_markdown(&doc, None);
        let current = study_input_hash(
            &doc.vehicle,
            &doc.design,
            doc.atmosphere.as_ref(),
            &doc.studies[0],
        );
        assert!(md.contains("**STALE**"));
        assert!(
            md.contains(short(&current)),
            "current hash shown for a stale study"
        );
        assert!(md.contains("NOT READY"), "a stale study blocks readiness");
    }

    #[test]
    fn missing_review_degrades_gracefully() {
        let doc = doc_with_dispersion_results(true);
        let md = flight_readiness_markdown(&doc, None);
        assert!(
            md.contains("## Structural Margins") && md.contains("Not yet computed"),
            "structural section must degrade, not vanish"
        );
        assert!(md.contains("## Rule Compliance"));
    }

    #[test]
    fn dual_deploy_recovery_is_described() {
        let mut doc = Document::default();
        doc.dispatch(Command::SetSimParam {
            param: "chute.main_deploy_altitude_m".into(),
            value: json!(60.0),
        })
        .unwrap();
        let md = flight_readiness_markdown(&doc, None);
        assert!(
            md.contains("dual-deploy"),
            "recovery line must reflect dual-deploy: {md}"
        );
        assert!(md.contains("60"), "deploy altitude must appear");
    }

    #[test]
    fn no_studies_reports_not_ready() {
        let doc = Document::default();
        let md = flight_readiness_markdown(&doc, None);
        assert!(md.contains("No studies defined"));
        assert!(md.contains("NOT READY"));
    }

    #[test]
    fn review_present_renders_structural_and_rule_tables() {
        let doc = doc_with_dispersion_results(true);
        let review = crate::review_ipc::report_for(&doc.vehicle, &doc.design, 350.0)
            .expect("reference tree reviews");
        let md = flight_readiness_markdown(&doc, Some(&review));
        // Structural margins table header and at least one flutter/airframe row.
        assert!(md.contains("| Check | Value | Limit | Margin | Status | Source |"));
        assert!(!review.review.structural_checks.is_empty());
        for check in &review.review.structural_checks {
            assert!(
                md.contains(&check.label),
                "structural check {} must appear",
                check.label
            );
        }
        // Rule compliance table with the review's citations.
        assert!(md.contains("| Rule | Measured | Required | Status | Citation |"));
        assert!(md.contains("Target apogee 350.0 m"));
        assert!(
            !md.contains("Not yet computed"),
            "review present: no degradation text"
        );
    }

    #[test]
    fn html_wrapper_embeds_and_escapes_the_markdown() {
        let doc = doc_with_dispersion_results(true);
        let html = flight_readiness_html(&doc, None);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<pre>"));
        // The Markdown '#' headers survive; angle brackets in prose escape.
        assert!(html.contains("Flight-Readiness Report"));
        assert!(
            !html.contains("<h1>"),
            "no Markdown-to-HTML rendering, just a verbatim wrapper"
        );
    }
}
