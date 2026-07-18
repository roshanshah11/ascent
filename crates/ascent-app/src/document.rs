//! The document store (v0.3 Step 2): Rust owns the user's state. The
//! frontend renders `DocumentState` snapshots and sends `Command`s; undo,
//! redo, and the journal live here. Replaying a journal from the default
//! document reproduces the live document byte-identically — that
//! determinism is what makes the journal a trustworthy session record.

use crate::command::{apply, Applied, Command};
use crate::design::Design;
use crate::study::Study;
use ascent_domain::vehicle::{reference_vehicle, Vehicle};
use serde::{Deserialize, Serialize};

pub const JOURNAL_VERSION: u32 = 1;

/// One journaled operation. Undo/redo are journaled too — a session
/// record that skipped them could not reproduce the final state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum JournalOp {
    Dispatch { command: Command },
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
        let Applied { forward, inverse } = apply(
            &mut self.vehicle,
            &mut self.design,
            &mut self.studies,
            &mut self.next_part_id,
            &mut self.next_study_id,
            cmd.clone(),
        )?;
        self.past.push(AppliedPair { forward, inverse });
        self.future.clear();
        self.journal.push(JournalOp::Dispatch { command: cmd });
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
                JournalOp::Dispatch { command } => doc
                    .dispatch(command)
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

#[cfg(test)]
mod tests {
    use super::*;
    use ascent_domain::vehicle::{NoseShape, PartId, PartKind};
    use serde_json::json;

    fn fin_kind() -> PartKind {
        PartKind::FinSet {
            count: 4,
            root_chord_m: 0.04,
            tip_chord_m: 0.02,
            span_m: 0.03,
            sweep_m: 0.0,
            thickness_mm: 2.0,
            mass_g: 5.0,
        }
    }

    #[test]
    fn dispatch_undo_redo_restores_byte_equal_state() {
        let mut doc = Document::default();
        let before = doc.canonical_bytes();
        doc.dispatch(Command::SetSimParam {
            param: "cd".into(),
            value: json!(0.75),
        })
        .unwrap();
        let after = doc.canonical_bytes();
        assert_ne!(before, after);
        assert!(doc.undo());
        assert_eq!(doc.design.cd, 0.60);
        assert!(doc.redo());
        assert_eq!(doc.canonical_bytes(), after);
    }

    #[test]
    fn add_part_allocates_a_fresh_id_and_undo_removes_it() {
        let mut doc = Document::default();
        let before_vehicle = doc.vehicle.clone();
        doc.dispatch(Command::AddPart {
            parent: Some(PartId(2)),
            kind: fin_kind(),
        })
        .unwrap();
        assert_eq!(doc.vehicle.parts[1].children.len(), 4);
        assert_eq!(doc.vehicle.parts[1].children[3].id, PartId(6));
        assert!(doc.undo());
        // The id counter stays monotonic (ids are never reused), so the
        // restoration check is on the tree itself, not canonical bytes.
        assert_eq!(doc.vehicle, before_vehicle);
        assert!(!doc.state().can_undo);
    }

    #[test]
    fn redo_after_add_reuses_the_same_id() {
        let mut doc = Document::default();
        doc.dispatch(Command::AddPart {
            parent: Some(PartId(2)),
            kind: fin_kind(),
        })
        .unwrap();
        let after = doc.canonical_bytes();
        doc.undo();
        doc.redo();
        assert_eq!(doc.canonical_bytes(), after);
    }

    #[test]
    fn set_part_param_patches_and_rejects_unknown_params() {
        let mut doc = Document::default();
        doc.dispatch(Command::SetPartParam {
            id: PartId(3),
            param: "root_chord_m".into(),
            value: json!(0.06),
        })
        .unwrap();
        match &crate::command::find_part_mut(&mut doc.vehicle, PartId(3)).unwrap().kind {
            PartKind::FinSet { root_chord_m, .. } => assert_eq!(*root_chord_m, 0.06),
            other => panic!("unexpected kind {other:?}"),
        }
        let err = doc
            .dispatch(Command::SetPartParam {
                id: PartId(3),
                param: "chord".into(),
                value: json!(0.06),
            })
            .unwrap_err();
        assert!(err.contains("no parameter"));
    }

    #[test]
    fn invalid_values_leave_the_document_untouched() {
        let mut doc = Document::default();
        let before = doc.canonical_bytes();
        assert!(doc
            .dispatch(Command::SetPartParam {
                id: PartId(3),
                param: "count".into(),
                value: json!("three"),
            })
            .is_err());
        assert!(doc
            .dispatch(Command::SetSimParam {
                param: "cd".into(),
                value: json!("slippery"),
            })
            .is_err());
        assert_eq!(doc.canonical_bytes(), before);
    }

    #[test]
    fn structural_parts_cannot_nest_via_commands() {
        let mut doc = Document::default();
        let err = doc
            .dispatch(Command::AddPart {
                parent: Some(PartId(2)),
                kind: PartKind::NoseCone {
                    shape: NoseShape::Conical,
                    length_m: 0.05,
                    base_radius_m: 0.0125,
                    mass_g: 4.0,
                },
            })
            .unwrap_err();
        assert!(err.contains("structural"));
    }

    #[test]
    fn fifty_command_journal_replays_byte_identically() {
        let mut doc = Document::default();
        for i in 0..20 {
            doc.dispatch(Command::SetSimParam {
                param: "cd".into(),
                value: json!(0.5 + (i as f64) * 0.01),
            })
            .unwrap();
        }
        doc.dispatch(Command::AddPart {
            parent: Some(PartId(2)),
            kind: fin_kind(),
        })
        .unwrap();
        for _ in 0..10 {
            doc.undo();
        }
        for _ in 0..5 {
            doc.redo();
        }
        doc.dispatch(Command::SelectMotor {
            designation: "B6".into(),
        })
        .unwrap();
        doc.dispatch(Command::RemovePart { id: PartId(4) }).unwrap();
        for i in 0..12 {
            doc.dispatch(Command::SetPartParam {
                id: PartId(3),
                param: "span_m".into(),
                value: json!(0.03 + (i as f64) * 0.001),
            })
            .unwrap();
        }
        let journal = doc.journal_jsonl();
        assert!(journal.lines().count() > 50);
        let replayed = Document::replay(&journal).unwrap();
        assert_eq!(replayed.canonical_bytes(), doc.canonical_bytes());
    }

    #[test]
    fn journal_errors_carry_line_numbers() {
        let mut journal = String::from("{\"journal_version\":1}\n");
        journal.push_str("{\"op\":\"dispatch\",\"command\":{\"cmd\":\"warp_drive\"}}\n");
        let err = Document::replay(&journal).unwrap_err();
        assert!(err.starts_with("line 2:"), "{err}");
        let err = Document::replay("{\"journal_version\":9}\n").unwrap_err();
        assert!(err.contains("version 9"));
    }

    #[test]
    fn select_motor_round_trips_through_undo() {
        let mut doc = Document::default();
        doc.dispatch(Command::SelectMotor {
            designation: "B6".into(),
        })
        .unwrap();
        assert_eq!(doc.design.motor_designation, "B6");
        doc.undo();
        assert_eq!(doc.design.motor_designation, "C6");
    }

    fn create_dispersion_study(doc: &mut Document) {
        doc.dispatch(Command::CreateStudy {
            name: "landing spread".into(),
            kind: crate::study::StudyKind::Dispersion { flights: 1000 },
            engine: "native".into(),
            seed: 42,
        })
        .unwrap();
    }

    #[test]
    fn study_crud_is_undoable() {
        use crate::study::StudyId;
        let mut doc = Document::default();
        let before = doc.canonical_bytes();

        // Create → undo removes it → redo restores the identical study.
        create_dispersion_study(&mut doc);
        assert_eq!(doc.studies.len(), 1);
        assert_eq!(doc.studies[0].id, StudyId(1));
        let after_create = doc.canonical_bytes();
        assert!(doc.undo());
        assert!(doc.studies.is_empty());
        assert!(doc.redo());
        assert_eq!(doc.canonical_bytes(), after_create);

        // Param edit round-trips through undo.
        doc.dispatch(Command::SetStudyParam {
            id: StudyId(1),
            param: "seed".into(),
            value: json!(7),
        })
        .unwrap();
        assert_eq!(doc.studies[0].seed, 7);
        doc.undo();
        assert_eq!(doc.studies[0].seed, 42);

        // Delete → undo restores at the same index with results intact.
        doc.dispatch(Command::SetStudyResults {
            id: StudyId(1),
            results: Some(crate::study::StudyResults {
                input_hash: "h".into(),
                data: json!({ "p50": 300.0 }),
            }),
        })
        .unwrap();
        // (Compare the studies, not canonical bytes — undoing a delete
        // legitimately leaves the redo stack populated.)
        let with_results = doc.studies.clone();
        doc.dispatch(Command::DeleteStudy { id: StudyId(1) }).unwrap();
        assert!(doc.studies.is_empty());
        assert!(doc.undo());
        assert_eq!(doc.studies, with_results);

        // Full unwind returns to the pristine document (ids stay
        // monotonic, so compare trees/state, not the counter).
        while doc.undo() {}
        assert!(doc.studies.is_empty());
        assert_eq!(doc.vehicle, Document::default().vehicle);
        let _ = before;
    }

    #[test]
    fn study_params_reject_identity_and_results() {
        use crate::study::StudyId;
        let mut doc = Document::default();
        create_dispersion_study(&mut doc);
        for (param, needle) in [
            ("id", "fixed"),
            ("results", "set_study_results"),
            ("flights", "no parameter"),
        ] {
            let err = doc
                .dispatch(Command::SetStudyParam {
                    id: StudyId(1),
                    param: param.into(),
                    value: json!(1),
                })
                .unwrap_err();
            assert!(err.contains(needle), "{param}: {err}");
        }
    }

    #[test]
    fn study_input_hash_goes_stale_when_the_tree_changes() {
        use crate::study::{study_input_hash, StudyId, StudyResults};
        let mut doc = Document::default();
        create_dispersion_study(&mut doc);

        // Land results stamped with the current input hash.
        let hash = study_input_hash(&doc.vehicle, &doc.design, &doc.studies[0]);
        doc.dispatch(Command::SetStudyResults {
            id: StudyId(1),
            results: Some(StudyResults {
                input_hash: hash,
                data: json!({ "p50": 300.0 }),
            }),
        })
        .unwrap();
        assert!(
            !doc.studies[0].is_stale(&doc.vehicle, &doc.design),
            "freshly stamped results are current"
        );

        // Edit the vehicle tree → the stored hash no longer matches.
        doc.dispatch(Command::SetPartParam {
            id: PartId(3),
            param: "span_m".into(),
            value: json!(0.05),
        })
        .unwrap();
        assert!(
            doc.studies[0].is_stale(&doc.vehicle, &doc.design),
            "tree edit must invalidate stored study results"
        );

        // Undo the edit → results are provably current again.
        doc.undo();
        assert!(!doc.studies[0].is_stale(&doc.vehicle, &doc.design));
    }

    #[test]
    fn journal_with_study_commands_replays_byte_identically() {
        use crate::study::StudyId;
        let mut doc = Document::default();
        create_dispersion_study(&mut doc);
        doc.dispatch(Command::SetStudyParam {
            id: StudyId(1),
            param: "name".into(),
            value: json!("renamed"),
        })
        .unwrap();
        doc.undo();
        doc.dispatch(Command::CreateStudy {
            name: "second".into(),
            kind: crate::study::StudyKind::SingleFlight,
            engine: "native".into(),
            seed: 0,
        })
        .unwrap();
        doc.dispatch(Command::DeleteStudy { id: StudyId(1) }).unwrap();
        doc.undo();
        doc.redo();
        let replayed = Document::replay(&doc.journal_jsonl()).unwrap();
        assert_eq!(replayed.canonical_bytes(), doc.canonical_bytes());
    }

    #[test]
    fn set_design_supports_reset_and_recovery() {
        let mut doc = Document::default();
        let mut recovered = Design::reference();
        recovered.dry_mass_g = 40.0;
        recovered.name = "Recovered".into();
        doc.dispatch(Command::SetDesign { design: recovered }).unwrap();
        assert_eq!(doc.design.dry_mass_g, 40.0);
        doc.undo();
        assert_eq!(doc.design.dry_mass_g, 34.0);
    }
}
