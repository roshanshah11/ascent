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
