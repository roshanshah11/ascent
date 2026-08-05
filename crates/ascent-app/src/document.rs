//! The document store (v0.3 Step 2): Rust owns the user's state. The
//! frontend renders `DocumentState` snapshots and sends `Command`s; undo,
//! redo, and the journal live here. Replaying a journal from the default
//! document reproduces the live document byte-identically — that
//! determinism is what makes the journal a trustworthy session record.

use crate::command::{apply, Applied, Command};
use crate::design::Design;
use crate::study::Study;
use ascent_domain::evidence::TelemetryBundle;
use ascent_domain::vehicle::{reference_vehicle, Vehicle};
use ascent_review::alignment::AlignmentArtifact;
use ascent_review::reconciliation::ReconciliationResult;
use ascent_sim::AtmosphereProfile;
use serde::{Deserialize, Serialize};

pub const JOURNAL_VERSION: u32 = 1;

/// Human and machine provenance attached to an engineering decision.
/// Empty fields mean the caller used the legacy dispatcher; they are kept
/// explicit so old journals remain readable without inventing authorship.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionMetadata {
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub affected_requirements: Vec<String>,
    #[serde(default)]
    pub evidence_hashes: Vec<String>,
}

/// One journaled operation. Undo/redo are journaled too — a session
/// record that skipped them could not reproduce the final state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
// Keep the established flat JSON journal schema. Boxing `command` would only
// save transient stack space while complicating the public replay contract.
#[allow(clippy::large_enum_variant)]
pub enum JournalOp {
    Dispatch {
        command: Command,
        #[serde(default, skip_serializing_if = "DecisionMetadata::is_empty")]
        metadata: DecisionMetadata,
    },
    Undo,
    Redo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JournalHeader {
    journal_version: u32,
}

/// Everything that defines the document's identity. Serialized in full
/// for the byte-identical replay check; `journal` is deliberately not
/// part of it (the journal is the input, not the state).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub vehicle: Vehicle,
    pub design: Design,
    pub studies: Vec<Study>,
    /// Imported wind/density profile (v0.5). None = analytic defaults.
    /// Serde-defaulted so pre-v0.5 project files load unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<AtmosphereProfile>,
    /// Immutable raw and normalized avionics evidence (v0.6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telemetry: Vec<TelemetryBundle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<AlignmentArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<ReconciliationResult>,
    next_part_id: u32,
    next_study_id: u32,
    past: Vec<AppliedPair>,
    future: Vec<AppliedPair>,
    #[serde(skip)]
    journal: Vec<JournalOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AppliedPair {
    forward: Command,
    inverse: Command,
}

/// Snapshot DTO the frontend renders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentState {
    pub vehicle: Vehicle,
    pub design: Design,
    pub studies: Vec<Study>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<AtmosphereProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telemetry: Vec<TelemetryBundle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<AlignmentArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<ReconciliationResult>,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl Default for Document {
    fn default() -> Self {
        let vehicle = reference_vehicle();
        let next_part_id = 1 + vehicle_max_id(&vehicle);
        Document {
            vehicle,
            design: Design::reference(),
            studies: Vec::new(),
            atmosphere: None,
            telemetry: Vec::new(),
            alignment: None,
            reconciliation: None,
            next_part_id,
            next_study_id: 1,
            past: Vec::new(),
            future: Vec::new(),
            journal: Vec::new(),
        }
    }
}

fn vehicle_max_id(v: &Vehicle) -> u32 {
    fn walk(parts: &[ascent_domain::vehicle::Part]) -> u32 {
        parts
            .iter()
            .map(|p| p.id.0.max(walk(&p.children)))
            .max()
            .unwrap_or(0)
    }
    walk(&v.parts)
}

impl Document {
    pub fn dispatch(&mut self, cmd: Command) -> Result<(), String> {
        self.dispatch_with_metadata(cmd, DecisionMetadata::default())
    }

    pub fn dispatch_with_metadata(
        &mut self,
        cmd: Command,
        metadata: DecisionMetadata,
    ) -> Result<(), String> {
        let Applied { forward, inverse } = apply(
            &mut self.vehicle,
            &mut self.design,
            &mut self.studies,
            &mut self.atmosphere,
            &mut self.telemetry,
            &mut self.alignment,
            &mut self.reconciliation,
            &mut self.next_part_id,
            &mut self.next_study_id,
            cmd.clone(),
        )?;
        self.past.push(AppliedPair { forward, inverse });
        self.future.clear();
        self.journal.push(JournalOp::Dispatch {
            command: cmd,
            metadata,
        });
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(pair) = self.past.pop() else {
            return false;
        };
        let mut scratch_id = self.next_part_id;
        let mut scratch_study_id = self.next_study_id;
        let undone = apply(
            &mut self.vehicle,
            &mut self.design,
            &mut self.studies,
            &mut self.atmosphere,
            &mut self.telemetry,
            &mut self.alignment,
            &mut self.reconciliation,
            &mut scratch_id,
            &mut scratch_study_id,
            pair.inverse.clone(),
        )
        .expect("inverse of an applied command must apply");
        // The inverse's own inverse re-does the original — keep the
        // concrete pair so redo reproduces identical state.
        debug_assert_eq!(undone.forward, pair.inverse);
        self.future.push(pair);
        self.journal.push(JournalOp::Undo);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(pair) = self.future.pop() else {
            return false;
        };
        let mut scratch_id = self.next_part_id;
        let mut scratch_study_id = self.next_study_id;
        apply(
            &mut self.vehicle,
            &mut self.design,
            &mut self.studies,
            &mut self.atmosphere,
            &mut self.telemetry,
            &mut self.alignment,
            &mut self.reconciliation,
            &mut scratch_id,
            &mut scratch_study_id,
            pair.forward.clone(),
        )
        .expect("a previously applied command must re-apply");
        self.past.push(pair);
        self.journal.push(JournalOp::Redo);
        true
    }

    pub fn state(&self) -> DocumentState {
        DocumentState {
            vehicle: self.vehicle.clone(),
            design: self.design.clone(),
            studies: self.studies.clone(),
            atmosphere: self.atmosphere.clone(),
            telemetry: self.telemetry.clone(),
            alignment: self.alignment.clone(),
            reconciliation: self.reconciliation.clone(),
            can_undo: !self.past.is_empty(),
            can_redo: !self.future.is_empty(),
        }
    }

    /// The session journal as JSONL: header line, then one op per line.
    pub fn journal_jsonl(&self) -> String {
        let mut out = serde_json::to_string(&JournalHeader {
            journal_version: JOURNAL_VERSION,
        })
        .expect("header serializes");
        for op in &self.journal {
            out.push('\n');
            out.push_str(&serde_json::to_string(op).expect("op serializes"));
        }
        out.push('\n');
        out
    }

    /// Rebuild a document by replaying a journal from the default
    /// document. Errors carry the 1-based line number.
    pub fn replay(jsonl: &str) -> Result<Document, String> {
        let mut lines = jsonl.lines().enumerate();
        let (_, header_line) = lines.next().ok_or("empty journal")?;
        let header: JournalHeader = serde_json::from_str(header_line)
            .map_err(|e| format!("line 1: bad journal header: {e}"))?;
        if header.journal_version != JOURNAL_VERSION {
            return Err(format!(
                "journal version {} not supported (this build reads {})",
                header.journal_version, JOURNAL_VERSION
            ));
        }
        let mut doc = Document::default();
        for (i, line) in lines {
            if line.trim().is_empty() {
                continue;
            }
            let op: JournalOp =
                serde_json::from_str(line).map_err(|e| format!("line {}: {e}", i + 1))?;
            match op {
                JournalOp::Dispatch { command, metadata } => doc
                    .dispatch_with_metadata(command, metadata)
                    .map_err(|e| format!("line {}: {e}", i + 1))?,
                JournalOp::Undo => {
                    doc.undo();
                }
                JournalOp::Redo => {
                    doc.redo();
                }
            }
        }
        Ok(doc)
    }

    /// Canonical byte form for determinism checks.
    pub fn canonical_bytes(&self) -> String {
        serde_json::to_string(self).expect("document serializes")
    }
}

impl DecisionMetadata {
    fn is_empty(&self) -> bool {
        self.author.is_empty()
            && self.source.is_empty()
            && self.intent.is_empty()
            && self.affected_requirements.is_empty()
            && self.evidence_hashes.is_empty()
    }
}
