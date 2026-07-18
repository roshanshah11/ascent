//! Copilot seam (v0.3 Step 9): the propose/approve loop. Any client — an
//! AI agent, a script, a remote tool — submits a command batch; Ascent
//! dry-runs it on a clone of the document and returns a structured
//! verdict: per-command errors, a diff summary, and exactly which
//! studies' stored results would go stale. Nothing mutates until a
//! second, explicit apply call. This is the approval-gated,
//! physics-in-the-loop contract in docs/COPILOT_INTERFACE.md — no
//! network, no LLM runtime; the seam is the deliverable.

use crate::command::Command;
use crate::document::Document;
use crate::study::StudyId;
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
                .is_some_and(|b| !b.is_stale(&before.vehicle, &before.design));
            was_current && s.is_stale(&after.vehicle, &after.design)
        })
        .map(|s| s.id)
        .collect();
    DiffSummary {
        vehicle_changed: before.vehicle != after.vehicle,
        design_changed: before.design != after.design,
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
            .filter_map(|c| c.error.as_ref().map(|e| format!("[{}] {}: {}", c.index, c.text, e)))
            .collect();
        return Err(format!("batch rejected: {}", errors.join("; ")));
    }
    for cmd in commands {
        // Validated on the clone one call ago; a failure here means the
        // document changed between propose and apply — stop loudly
        // rather than land a half batch silently.
        doc.dispatch(cmd.clone())
            .map_err(|e| format!("batch diverged mid-apply (document changed?): {e}"))?;
    }
    Ok(proposal)
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
        let hash = study_input_hash(&doc.vehicle, &doc.design, &doc.studies[0]);
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
    fn valid_batch_returns_a_diff_without_mutating() {
        let doc = doc_with_current_study();
        let before = doc.canonical_bytes();
        let proposal = propose_batch(
            &doc,
            &[span_edit(), Command::SetSimParam { param: "cd".into(), value: json!(0.7) }],
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
                Command::SetSimParam { param: "cd".into(), value: json!(0.7) },
                Command::SetPartParam {
                    id: PartId(99),
                    param: "span_m".into(),
                    value: json!(0.05),
                },
                Command::SetSimParam { param: "dc".into(), value: json!(1.0) },
            ],
        );
        assert!(!proposal.valid);
        assert!(proposal.diff.is_none());
        assert_eq!(proposal.commands[0].error, None);
        assert!(proposal.commands[1].error.as_ref().unwrap().contains("no part 99"));
        assert!(proposal.commands[2].error.as_ref().unwrap().contains("no parameter"));
        assert_eq!(proposal.commands[2].index, 2);
    }

    #[test]
    fn apply_after_propose_lands_atomically_and_journals() {
        let mut doc = doc_with_current_study();
        let batch = [span_edit(), Command::SelectMotor { designation: "B6".into() }];
        let proposal = apply_batch(&mut doc, &batch).unwrap();
        assert!(proposal.valid);
        assert_eq!(doc.design.motor_designation, "B6");
        assert!(doc.studies[0].is_stale(&doc.vehicle, &doc.design));

        // Both commands journaled — replay reproduces the applied state.
        let replayed = Document::replay(&doc.journal_jsonl()).unwrap();
        assert_eq!(replayed.canonical_bytes(), doc.canonical_bytes());

        // And they undo like any other command.
        doc.undo();
        assert_eq!(doc.design.motor_designation, "C6");
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
        assert!(doc.studies[0].is_stale(&doc.vehicle, &doc.design));

        let proposal = propose_batch(
            &doc,
            &[Command::SetSimParam { param: "cd".into(), value: json!(0.8) }],
        );
        assert_eq!(
            proposal.diff.unwrap().studies_made_stale,
            Vec::<StudyId>::new(),
            "already-stale and unrun studies are not news"
        );
    }
}
