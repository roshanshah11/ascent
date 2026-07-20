//! Copilot seam (v0.3 Step 9): the propose/approve loop. Any client — an
//! AI agent, a script, a remote tool — submits a command batch; Ascent
//! dry-runs it on a clone of the document and returns a structured
//! verdict: per-command errors, a diff summary, and exactly which
//! studies' stored results would go stale. Nothing mutates until a
//! second, explicit apply call. This is the approval-gated,
//! physics-in-the-loop contract in docs/COPILOT_INTERFACE.md — no
//! network, no LLM runtime; the seam is the deliverable.

use crate::command::Command;
use crate::design::{build_flight, run_design_with_atmosphere, RunRecord};
use crate::document::Document;
use crate::evidence::{evidence_for_with_atmosphere, EvidenceReport};
use crate::markers::{vehicle_markers, VehicleMarkers};
use crate::review_ipc::{report_for, ReviewReport};
use crate::study::StudyId;
use crate::DocumentState;
use ascent_domain::evidence::FlightTrace;
use ascent_review::reconciliation::{Reconciler, ReconciliationResult};
use ascent_sim::{
    content_hash, flight_trace_from_sixdof, simulate_sixdof_staged, SimConfig, SixDofLaunch,
    Wind3DLayer, Wind3DProfile,
};
use serde::{Deserialize, Serialize};

/// One command's dry-run outcome, in batch order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandCheck {
    pub index: usize,
    /// Canonical text form of the command (JOURNAL_FORMAT.md grammar).
    pub text: String,
    pub error: Option<String>,
}

/// What the batch would change, computed on the dry-run clone. Field
/// deltas, not value dumps — the approving human (or supervising agent)
/// reads this, so it names things.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub vehicle_changed: bool,
    pub design_changed: bool,
    /// The imported atmosphere profile would change (v0.5). Serde-defaulted
    /// so pre-v0.5 clients' recorded proposals still deserialize.
    #[serde(default)]
    pub atmosphere_changed: bool,
    pub parts_added: u32,
    pub parts_removed: u32,
    pub studies_added: u32,
    pub studies_removed: u32,
    /// Studies whose *stored results* would stop matching their inputs
    /// if this batch applied — the physics-in-the-loop warning.
    pub studies_made_stale: Vec<StudyId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    pub valid: bool,
    pub commands: Vec<CommandCheck>,
    /// Present only when the batch is valid end-to-end.
    pub diff: Option<DiffSummary>,
}

/// Complete cloned proposal universe. Presentation clients consume this one
/// object for geometry/state, physics, evidence, and report preview so visual
/// ghosts cannot diverge from semantic consequences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterfactualReview {
    pub proposal: Proposal,
    pub baseline_document_hash: String,
    pub proposed_document_hash: String,
    pub parent_hashes: Vec<String>,
    pub baseline_run: RunRecord,
    pub proposed_run: RunRecord,
    pub baseline_state: DocumentState,
    pub proposed_state: DocumentState,
    pub baseline_markers: VehicleMarkers,
    pub proposed_markers: VehicleMarkers,
    pub baseline_trace: FlightTrace,
    pub proposed_trace: FlightTrace,
    pub measured_trace: Option<FlightTrace>,
    pub baseline_evidence: EvidenceReport,
    pub proposed_evidence: EvidenceReport,
    pub baseline_review: ReviewReport,
    pub proposed_review: ReviewReport,
    pub baseline_reconciliation: Option<ReconciliationResult>,
    pub proposed_reconciliation: Option<ReconciliationResult>,
    pub qualifications: Vec<String>,
    pub baseline_report_html: String,
    pub report_preview_html: String,
}

fn count_parts(doc: &Document) -> u32 {
    fn walk(parts: &[ascent_domain::vehicle::Part]) -> u32 {
        parts.iter().map(|p| 1 + walk(&p.children)).sum()
    }
    walk(&doc.vehicle.parts)
}

fn diff(before: &Document, after: &Document) -> DiffSummary {
    let parts_before = count_parts(before);
    let parts_after = count_parts(after);
    let studies_before = before.studies.len() as u32;
    let studies_after = after.studies.len() as u32;
    // A study goes stale when its stored results matched the inputs
    // before the batch and no longer do after it. Studies without
    // results can't go stale; already-stale studies aren't news.
    let studies_made_stale = after
        .studies
        .iter()
        .filter(|s| {
            let was_current = before
                .studies
                .iter()
                .find(|b| b.id == s.id)
                .is_some_and(|b| {
                    !b.is_stale(&before.vehicle, &before.design, before.atmosphere.as_ref())
                });
            was_current && s.is_stale(&after.vehicle, &after.design, after.atmosphere.as_ref())
        })
        .map(|s| s.id)
        .collect();
    DiffSummary {
        vehicle_changed: before.vehicle != after.vehicle,
        design_changed: before.design != after.design,
        atmosphere_changed: before.atmosphere != after.atmosphere,
        parts_added: parts_after.saturating_sub(parts_before),
        parts_removed: parts_before.saturating_sub(parts_after),
        studies_added: studies_after.saturating_sub(studies_before),
        studies_removed: studies_before.saturating_sub(studies_after),
        studies_made_stale,
    }
}

/// Dry-run a batch against a clone of `doc`. The document is never
/// mutated. Commands after the first failure are still checked — each
/// against the state the previous *successful* commands produced — so
/// the client sees every error in one round-trip.
pub fn propose_batch(doc: &Document, commands: &[Command]) -> Proposal {
    let mut clone = doc.clone();
    let mut checks = Vec::with_capacity(commands.len());
    let mut valid = true;
    for (index, cmd) in commands.iter().enumerate() {
        let error = clone.dispatch(cmd.clone()).err();
        if error.is_some() {
            valid = false;
        }
        checks.push(CommandCheck {
            index,
            text: cmd.to_text(),
            error,
        });
    }
    Proposal {
        valid,
        commands: checks,
        diff: valid.then(|| diff(doc, &clone)),
    }
}

/// Apply a batch atomically: re-validate on a clone first, then land
/// every command on the real document. A batch that fails validation
/// leaves the document untouched; a batch that passes lands whole and
/// journals normally. Returns the (re-checked) proposal it applied.
pub fn apply_batch(doc: &mut Document, commands: &[Command]) -> Result<Proposal, String> {
    let proposal = propose_batch(doc, commands);
    if !proposal.valid {
        let errors: Vec<String> = proposal
            .commands
            .iter()
            .filter_map(|c| {
                c.error
                    .as_ref()
                    .map(|e| format!("[{}] {}: {}", c.index, c.text, e))
            })
            .collect();
        return Err(format!("batch rejected: {}", errors.join("; ")));
    }
    doc.dispatch(Command::Batch {
        commands: commands.to_vec(),
    })
    .map_err(|e| format!("batch diverged during atomic apply (document changed?): {e}"))?;
    Ok(proposal)
}

pub fn build_counterfactual_review(
    doc: &Document,
    commands: &[Command],
) -> Result<CounterfactualReview, String> {
    let proposal = propose_batch(doc, commands);
    if !proposal.valid {
        return Err("cannot build a counterfactual universe from an invalid proposal".into());
    }
    let baseline_document_hash = content_hash(&doc.canonical_bytes());
    let baseline_run = run_design_with_atmosphere(&doc.design, doc.atmosphere.as_ref())?;
    let baseline_trace = sixdof_trace(doc)?;
    let baseline_evidence = evidence_for_with_atmosphere(&doc.design, doc.atmosphere.as_ref())?;
    let baseline_review = report_for(&doc.vehicle, &doc.design, baseline_run.summary.apogee_m)?;
    let mut proposed = doc.clone();
    proposed.dispatch(Command::Batch {
        commands: commands.to_vec(),
    })?;
    let proposed_document_hash = content_hash(&proposed.canonical_bytes());
    let proposed_run = run_design_with_atmosphere(&proposed.design, proposed.atmosphere.as_ref())?;
    let proposed_trace = sixdof_trace(&proposed)?;
    let proposed_evidence =
        evidence_for_with_atmosphere(&proposed.design, proposed.atmosphere.as_ref())?;
    let proposed_review = report_for(
        &proposed.vehicle,
        &proposed.design,
        baseline_run.summary.apogee_m,
    )?;
    let report_preview_html =
        crate::report::flight_readiness_html(&proposed, Some(&proposed_review));
    let baseline_report_html = crate::report::flight_readiness_html(doc, Some(&baseline_review));
    let measured_trace = doc.telemetry.first().map(|bundle| bundle.trace.clone());
    let baseline_reconciliation = doc.reconciliation.clone();
    let mut qualifications = Vec::new();
    let proposed_reconciliation = match (
        doc.alignment.as_ref(),
        doc.reconciliation.as_ref(),
        measured_trace.as_ref(),
    ) {
        (Some(alignment), Some(definition), Some(measured))
            if !definition.channel_pairs.is_empty() && !definition.phase_windows.is_empty() =>
        {
            match Reconciler::compare(
                &proposed_trace,
                measured,
                alignment,
                &definition.channel_pairs,
                &definition.phase_windows,
                &definition.diagnostic_candidates,
            ) {
                Ok(result) => Some(result),
                Err(error) => {
                    qualifications.push(format!("proposal reconciliation unavailable: {error}"));
                    None
                }
            }
        }
        _ => {
            qualifications.push(
                "proposal has no rerunnable measured-flight reconciliation definition".into(),
            );
            None
        }
    };
    Ok(CounterfactualReview {
        proposal,
        baseline_document_hash: baseline_document_hash.clone(),
        proposed_document_hash,
        parent_hashes: vec![
            baseline_document_hash,
            baseline_run.summary.input_hash.clone(),
        ],
        baseline_run,
        proposed_run,
        baseline_state: doc.state(),
        proposed_state: proposed.state(),
        baseline_markers: vehicle_markers(&doc.vehicle, &doc.design)?,
        proposed_markers: vehicle_markers(&proposed.vehicle, &proposed.design)?,
        baseline_trace,
        proposed_trace,
        measured_trace,
        baseline_evidence,
        proposed_evidence,
        baseline_review,
        proposed_review,
        baseline_reconciliation,
        proposed_reconciliation,
        qualifications,
        baseline_report_html,
        report_preview_html,
    })
}

fn sixdof_trace(document: &Document) -> Result<FlightTrace, String> {
    let (rocket, _, mut environment) = build_flight(&document.design)?;
    let wind = document
        .atmosphere
        .as_ref()
        .map_or_else(Wind3DProfile::calm, |profile| Wind3DProfile {
            layers: profile
                .layers
                .iter()
                .map(|layer| {
                    let direction = layer.wind_direction_deg.to_radians();
                    Wind3DLayer {
                        altitude_m: layer.altitude_m,
                        velocity_ms: [
                            layer.wind_speed_ms * direction.cos(),
                            layer.wind_speed_ms * direction.sin(),
                            0.0,
                        ],
                    }
                })
                .collect(),
        });
    if let Some(model) = document
        .atmosphere
        .as_ref()
        .and_then(ascent_sim::AtmosphereProfile::atmosphere_model)
    {
        environment.atmosphere = model;
    }
    let mut stages =
        crate::staging::sixdof_stages_from_tree(&document.vehicle, document.design.cd)?;
    // The legacy flat-design motor selector is the authoritative selector for
    // a single-stage document. Multi-stage vehicles retain their explicit
    // per-stage tree motor mounts.
    if stages.len() == 1 {
        stages[0].motor = crate::design::find_motor(&document.design.motor_designation)?;
    }
    let result = simulate_sixdof_staged(
        &stages,
        rocket.recovery,
        &wind,
        &SixDofLaunch::vertical(),
        &environment,
        &SimConfig::default(),
    )?;
    flight_trace_from_sixdof(&result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::study::{study_input_hash, StudyKind, StudyResults};
    use ascent_domain::vehicle::PartId;
    use serde_json::json;

    fn doc_with_current_study() -> Document {
        let mut doc = Document::default();
        doc.dispatch(Command::CreateStudy {
            name: "spread".into(),
            kind: StudyKind::Dispersion { flights: 100 },
            engine: "native".into(),
            seed: 42,
        })
        .unwrap();
        let hash = study_input_hash(
            &doc.vehicle,
            &doc.design,
            doc.atmosphere.as_ref(),
            &doc.studies[0],
        );
        doc.dispatch(Command::SetStudyResults {
            id: StudyId(1),
            results: Some(StudyResults {
                input_hash: hash,
                data: json!({ "p50": 300.0 }),
            }),
        })
        .unwrap();
        doc
    }

    fn span_edit() -> Command {
        Command::SetPartParam {
            id: PartId(3),
            param: "span_m".into(),
            value: json!(0.05),
        }
    }

    #[test]
    fn importing_an_atmosphere_stales_current_studies() {
        use ascent_sim::AtmosphereProfile;
        let doc = doc_with_current_study();
        let profile = AtmosphereProfile::from_csv(
            "koun-12z",
            "altitude_m,wind_speed_ms,wind_direction_deg,density_kg_m3\n0,3,0,1.225\n1000,6,20,1.112\n",
        )
        .unwrap();
        let proposal = propose_batch(
            &doc,
            &[Command::SetAtmosphere {
                profile: Some(profile),
            }],
        );
        assert!(proposal.valid);
        let diff = proposal.diff.unwrap();
        assert!(diff.atmosphere_changed);
        assert_eq!(
            diff.studies_made_stale,
            vec![StudyId(1)],
            "swapping the atmosphere must provably stale stored results"
        );
        // Clearing an already-absent profile changes nothing and stales nothing.
        let noop = propose_batch(&doc, &[Command::SetAtmosphere { profile: None }]);
        let noop_diff = noop.diff.unwrap();
        assert!(!noop_diff.atmosphere_changed);
        assert!(noop_diff.studies_made_stale.is_empty());
    }

    #[test]
    fn valid_batch_returns_a_diff_without_mutating() {
        let doc = doc_with_current_study();
        let before = doc.canonical_bytes();
        let proposal = propose_batch(
            &doc,
            &[
                span_edit(),
                Command::SetSimParam {
                    param: "cd".into(),
                    value: json!(0.7),
                },
            ],
        );
        assert_eq!(doc.canonical_bytes(), before, "propose must never mutate");
        assert!(proposal.valid);
        let diff = proposal.diff.unwrap();
        assert!(diff.vehicle_changed);
        assert!(diff.design_changed);
        assert_eq!(diff.parts_added, 0);
        assert_eq!(diff.studies_made_stale, vec![StudyId(1)]);
        assert_eq!(proposal.commands[0].text, "set-part-param 3 span_m 0.05");
    }

    #[test]
    fn invalid_batch_reports_every_error_with_indices() {
        let doc = Document::default();
        let proposal = propose_batch(
            &doc,
            &[
                Command::SetSimParam {
                    param: "cd".into(),
                    value: json!(0.7),
                },
                Command::SetPartParam {
                    id: PartId(99),
                    param: "span_m".into(),
                    value: json!(0.05),
                },
                Command::SetSimParam {
                    param: "dc".into(),
                    value: json!(1.0),
                },
            ],
        );
        assert!(!proposal.valid);
        assert!(proposal.diff.is_none());
        assert_eq!(proposal.commands[0].error, None);
        assert!(proposal.commands[1]
            .error
            .as_ref()
            .unwrap()
            .contains("no part 99"));
        assert!(proposal.commands[2]
            .error
            .as_ref()
            .unwrap()
            .contains("no parameter"));
        assert_eq!(proposal.commands[2].index, 2);
    }

    #[test]
    fn apply_after_propose_lands_atomically_and_journals() {
        let mut doc = doc_with_current_study();
        let original_span = match &doc.vehicle.parts[1].children[0].kind {
            ascent_domain::vehicle::PartKind::FinSet { span_m, .. } => *span_m,
            _ => unreachable!(),
        };
        let batch = [
            span_edit(),
            Command::SelectMotor {
                designation: "B6".into(),
            },
        ];
        let proposal = apply_batch(&mut doc, &batch).unwrap();
        assert!(proposal.valid);
        assert_eq!(doc.design.motor_designation, "B6");
        assert!(doc.studies[0].is_stale(&doc.vehicle, &doc.design, doc.atmosphere.as_ref()));

        // Both commands journaled — replay reproduces the applied state.
        let replayed = Document::replay(&doc.journal_jsonl()).unwrap();
        assert_eq!(replayed.canonical_bytes(), doc.canonical_bytes());

        // And they undo like any other command.
        doc.undo();
        assert_eq!(doc.design.motor_designation, "C6");
        let restored_span = match &doc.vehicle.parts[1].children[0].kind {
            ascent_domain::vehicle::PartKind::FinSet { span_m, .. } => *span_m,
            _ => unreachable!(),
        };
        assert_eq!(
            restored_span, original_span,
            "one undo restores the whole approved batch"
        );
        assert_eq!(
            doc.journal_jsonl()
                .lines()
                .filter(|line| line.contains("\"cmd\":\"batch\""))
                .count(),
            1,
            "approval journals one canonical batch"
        );
    }

    #[test]
    fn rejected_batch_leaves_the_document_untouched() {
        let mut doc = doc_with_current_study();
        let before = doc.canonical_bytes();
        let err = apply_batch(
            &mut doc,
            &[
                span_edit(),
                Command::SetPartParam {
                    id: PartId(99),
                    param: "span_m".into(),
                    value: json!(1.0),
                },
            ],
        )
        .unwrap_err();
        assert!(err.contains("[1]"), "error names the failing index: {err}");
        assert_eq!(doc.canonical_bytes(), before, "no partial application");
    }

    #[test]
    fn counterfactual_universe_derives_consequences_without_touching_baseline() {
        let doc = doc_with_current_study();
        let before = doc.canonical_bytes();
        let commands = [
            span_edit(),
            Command::SetSimParam {
                param: "cd".into(),
                value: json!(0.72),
            },
        ];
        let first = build_counterfactual_review(&doc, &commands).unwrap();
        let second = build_counterfactual_review(&doc, &commands).unwrap();

        assert_eq!(doc.canonical_bytes(), before);
        assert_ne!(first.baseline_document_hash, first.proposed_document_hash);
        assert_eq!(
            serde_json::to_vec(&first.baseline_state).unwrap(),
            serde_json::to_vec(&doc.state()).unwrap()
        );
        assert_ne!(first.baseline_state.vehicle, first.proposed_state.vehicle);
        assert_ne!(
            first.baseline_markers.cp_from_nose_m,
            first.proposed_markers.cp_from_nose_m
        );
        assert_ne!(
            first.baseline_run.summary.apogee_m,
            first.proposed_run.summary.apogee_m
        );
        first.proposed_trace.validate().unwrap();
        assert!(first
            .proposed_trace
            .channels
            .iter()
            .any(|channel| channel.id == "truth.attitude"));
        assert_eq!(
            first.proposed_evidence.input_hash,
            first.proposed_run.summary.input_hash
        );
        assert!(first.report_preview_html.contains("Flight-Readiness"));
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap(),
            "identical proposal inputs produce byte-identical universes"
        );
    }

    #[test]
    fn stale_list_ignores_unrun_and_already_stale_studies() {
        let mut doc = doc_with_current_study();
        // A second study with no results.
        doc.dispatch(Command::CreateStudy {
            name: "unrun".into(),
            kind: StudyKind::Dispersion { flights: 10 },
            engine: "native".into(),
            seed: 1,
        })
        .unwrap();
        // Make study 1 already stale before the batch.
        doc.dispatch(span_edit()).unwrap();
        assert!(doc.studies[0].is_stale(&doc.vehicle, &doc.design, doc.atmosphere.as_ref()));

        let proposal = propose_batch(
            &doc,
            &[Command::SetSimParam {
                param: "cd".into(),
                value: json!(0.8),
            }],
        );
        assert_eq!(
            proposal.diff.unwrap().studies_made_stale,
            Vec::<StudyId>::new(),
            "already-stale and unrun studies are not news"
        );
    }

    #[test]
    fn as_built_mass_override_stales_studies_and_journal_replays() {
        let mut doc = doc_with_current_study();
        let change = Command::SetPartParam {
            id: PartId(3),
            param: "as_built_mass_g".into(),
            value: json!(10.0),
        };
        let proposal = propose_batch(&doc, std::slice::from_ref(&change));
        assert!(proposal.valid);
        assert_eq!(proposal.diff.unwrap().studies_made_stale, vec![StudyId(1)]);
        apply_batch(&mut doc, &[change]).unwrap();
        assert_eq!(doc.vehicle.parts[1].children[0].as_built_mass_g, Some(10.0));
        assert!(doc.studies[0].is_stale(&doc.vehicle, &doc.design, doc.atmosphere.as_ref()));
        let replayed = Document::replay(&doc.journal_jsonl()).unwrap();
        assert_eq!(replayed.canonical_bytes(), doc.canonical_bytes());
    }
}
