//! The command spine (v0.3 Step 2). Every mutation of user-owned state is
//! a named, serde-serializable `Command` applied through `Document` — the
//! GUI, the console, the CLI, and any future copilot are all clients of
//! this one dispatcher. Grammar contract: `docs/JOURNAL_FORMAT.md`.
//!
//! Undo fidelity rule: `apply` returns a *concrete* forward form plus its
//! inverse. `AddPart` allocates an id at apply time, so its concrete
//! forward form is a `RestorePart` carrying the resolved part — redo then
//! reproduces the identical document instead of allocating a fresh id.

use crate::design::Design;
use crate::study::{Study, StudyId, StudyKind, StudyResults};
use ascent_domain::vehicle::{Part, PartId, PartKind, Vehicle};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    AddPart {
        parent: Option<PartId>,
        kind: PartKind,
    },
    RemovePart {
        id: PartId,
    },
    /// Concrete re-insertion (inverse of RemovePart, concrete form of
    /// AddPart). `index` is the position in the parent's child vector, or
    /// the root vector when `parent` is None.
    RestorePart {
        parent: Option<PartId>,
        index: usize,
        part: Part,
    },
    /// Set one named parameter on one part. Parameter names are the serde
    /// field names from VEHICLE_TREE.md; validation is by construction —
    /// the patched part must deserialize back into a valid PartKind.
    SetPartParam {
        id: PartId,
        param: String,
        value: Value,
    },
    /// Set one named parameter on the flat sim design ("dry_mass_g",
    /// "diameter_mm", "cd", "rail_length_m", "name", "chute.enabled",
    /// "chute.diameter_cm", "chute.cd").
    SetSimParam {
        param: String,
        value: Value,
    },
    SelectMotor {
        designation: String,
    },
    /// Coarse design replacement — reset and crash-recovery restore.
    SetDesign {
        design: Design,
    },
    CreateStudy {
        name: String,
        kind: StudyKind,
        engine: String,
        seed: u64,
    },
    DeleteStudy {
        id: StudyId,
    },
    /// Concrete re-insertion (inverse of DeleteStudy, concrete form of
    /// CreateStudy — same fidelity rule as RestorePart).
    RestoreStudy {
        index: usize,
        study: Study,
    },
    /// Set one named parameter on one study ("name", "engine", "seed",
    /// or "kind" as a whole tagged object). `id` and `results` are not
    /// parameters — ids are immutable and results land via SetStudyResults.
    SetStudyParam {
        id: StudyId,
        param: String,
        value: Value,
    },
    /// Land (or clear) a study's results. The job runner dispatches this
    /// on completion, so results arrive through the journal like every
    /// other mutation.
    SetStudyResults {
        id: StudyId,
        results: Option<StudyResults>,
    },
}

/// Result of applying a command: the concrete forward form (what redo
/// replays) and the inverse (what undo replays).
pub struct Applied {
    pub forward: Command,
    pub inverse: Command,
}

pub fn apply(
    vehicle: &mut Vehicle,
    design: &mut Design,
    studies: &mut Vec<Study>,
    next_part_id: &mut u32,
    next_study_id: &mut u32,
    cmd: Command,
) -> Result<Applied, String> {
    match cmd {
        Command::AddPart { parent, kind } => {
            let id = PartId(*next_part_id);
            let part = Part {
                id,
                kind,
                children: vec![],
            };
            let index = insert_part(vehicle, parent, None, part.clone())?;
            *next_part_id += 1;
            Ok(Applied {
                forward: Command::RestorePart {
                    parent,
                    index,
                    part,
                },
                inverse: Command::RemovePart { id },
            })
        }
        Command::RestorePart {
            parent,
            index,
            part,
        } => {
            insert_part(vehicle, parent, Some(index), part.clone())?;
            let id = part.id;
            Ok(Applied {
                forward: Command::RestorePart {
                    parent,
                    index,
                    part,
                },
                inverse: Command::RemovePart { id },
            })
        }
        Command::RemovePart { id } => {
            let (parent, index, part) = remove_part(vehicle, id)?;
            Ok(Applied {
                forward: Command::RemovePart { id },
                inverse: Command::RestorePart {
                    parent,
                    index,
                    part,
                },
            })
        }
        Command::SetPartParam { id, param, value } => {
            let part = find_part_mut(vehicle, id).ok_or_else(|| format!("no part {}", id.0))?;
            let old = patch_kind(&mut part.kind, &param, value.clone())?;
            vehicle.validate()?;
            Ok(Applied {
                forward: Command::SetPartParam {
                    id,
                    param: param.clone(),
                    value,
                },
                inverse: Command::SetPartParam {
                    id,
                    param,
                    value: old,
                },
            })
        }
        Command::SetSimParam { param, value } => {
            let old = patch_design(design, &param, value.clone())?;
            Ok(Applied {
                forward: Command::SetSimParam {
                    param: param.clone(),
                    value,
                },
                inverse: Command::SetSimParam { param, value: old },
            })
        }
        Command::SelectMotor { designation } => {
            let old = std::mem::replace(&mut design.motor_designation, designation.clone());
            Ok(Applied {
                forward: Command::SelectMotor { designation },
                inverse: Command::SelectMotor { designation: old },
            })
        }
        Command::SetDesign { design: next } => {
            let old = std::mem::replace(design, next.clone());
            Ok(Applied {
                forward: Command::SetDesign { design: next },
                inverse: Command::SetDesign { design: old },
            })
        }
        Command::CreateStudy {
            name,
            kind,
            engine,
            seed,
        } => {
            let id = StudyId(*next_study_id);
            let study = Study {
                id,
                name,
                kind,
                engine,
                seed,
                results: None,
            };
            let index = studies.len();
            studies.push(study.clone());
            *next_study_id += 1;
            Ok(Applied {
                forward: Command::RestoreStudy { index, study },
                inverse: Command::DeleteStudy { id },
            })
        }
        Command::RestoreStudy { index, study } => {
            if studies.iter().any(|s| s.id == study.id) {
                return Err(format!("study {} already exists", study.id.0));
            }
            let at = index.min(studies.len());
            studies.insert(at, study.clone());
            let id = study.id;
            Ok(Applied {
                forward: Command::RestoreStudy { index: at, study },
                inverse: Command::DeleteStudy { id },
            })
        }
        Command::DeleteStudy { id } => {
            let index = studies
                .iter()
                .position(|s| s.id == id)
                .ok_or_else(|| format!("no study {}", id.0))?;
            let study = studies.remove(index);
            Ok(Applied {
                forward: Command::DeleteStudy { id },
                inverse: Command::RestoreStudy { index, study },
            })
        }
        Command::SetStudyParam { id, param, value } => {
            let study = find_study_mut(studies, id).ok_or_else(|| format!("no study {}", id.0))?;
            let old = patch_study(study, &param, value.clone())?;
            Ok(Applied {
                forward: Command::SetStudyParam {
                    id,
                    param: param.clone(),
                    value,
                },
                inverse: Command::SetStudyParam {
                    id,
                    param,
                    value: old,
                },
            })
        }
        Command::SetStudyResults { id, results } => {
            let study = find_study_mut(studies, id).ok_or_else(|| format!("no study {}", id.0))?;
            let old = std::mem::replace(&mut study.results, results.clone());
            Ok(Applied {
                forward: Command::SetStudyResults { id, results },
                inverse: Command::SetStudyResults { id, results: old },
            })
        }
    }
}

fn find_study_mut(studies: &mut [Study], id: StudyId) -> Option<&mut Study> {
    studies.iter_mut().find(|s| s.id == id)
}

/// Patch-through-serde for a Study, same discipline as parts and the
/// design: the field must exist, the patched object must deserialize
/// back, and identity/results fields are off limits.
fn patch_study(study: &mut Study, param: &str, value: Value) -> Result<Value, String> {
    if param == "id" {
        return Err("a study's id is fixed".into());
    }
    if param == "results" {
        return Err("results land via set_study_results, not as a parameter".into());
    }
    let mut json = serde_json::to_value(&*study).map_err(|e| e.to_string())?;
    let map = json.as_object_mut().expect("study is an object");
    let old = map
        .get(param)
        .cloned()
        .ok_or_else(|| format!("study has no parameter '{param}'"))?;
    map.insert(param.to_string(), value);
    *study = serde_json::from_value(json).map_err(|e| format!("invalid value for '{param}': {e}"))?;
    Ok(old)
}

fn insert_part(
    vehicle: &mut Vehicle,
    parent: Option<PartId>,
    index: Option<usize>,
    part: Part,
) -> Result<usize, String> {
    let mut candidate = vehicle.clone();
    let slot = match parent {
        None => &mut candidate.parts,
        Some(pid) => {
            &mut find_part_mut(&mut candidate, pid)
                .ok_or_else(|| format!("no parent part {}", pid.0))?
                .children
        }
    };
    let at = index.unwrap_or(slot.len()).min(slot.len());
    slot.insert(at, part);
    candidate.validate()?;
    *vehicle = candidate;
    Ok(at)
}

fn remove_part(
    vehicle: &mut Vehicle,
    id: PartId,
) -> Result<(Option<PartId>, usize, Part), String> {
    if let Some(index) = vehicle.parts.iter().position(|p| p.id == id) {
        let part = vehicle.parts.remove(index);
        return Ok((None, index, part));
    }
    fn walk(parts: &mut [Part], id: PartId) -> Option<(PartId, usize, Part)> {
        for parent in parts.iter_mut() {
            if let Some(index) = parent.children.iter().position(|c| c.id == id) {
                let part = parent.children.remove(index);
                return Some((parent.id, index, part));
            }
            if let Some(found) = walk(&mut parent.children, id) {
                return Some(found);
            }
        }
        None
    }
    walk(&mut vehicle.parts, id)
        .map(|(pid, i, part)| (Some(pid), i, part))
        .ok_or_else(|| format!("no part {}", id.0))
}

pub fn find_part_mut(vehicle: &mut Vehicle, id: PartId) -> Option<&mut Part> {
    fn walk(parts: &mut [Part], id: PartId) -> Option<&mut Part> {
        for part in parts.iter_mut() {
            if part.id == id {
                return Some(part);
            }
            if let Some(found) = walk(&mut part.children, id) {
                return Some(found);
            }
        }
        None
    }
    walk(&mut vehicle.parts, id)
}

/// Patch one serde field on a PartKind via its JSON form: the field must
/// already exist (no typo'd params) and the patched object must
/// deserialize back into a valid PartKind (no type confusion). Returns
/// the old value for the inverse.
fn patch_kind(kind: &mut PartKind, param: &str, value: Value) -> Result<Value, String> {
    if param == "type" {
        return Err("a part's type is fixed; add a new part instead".into());
    }
    let mut json = serde_json::to_value(&*kind).map_err(|e| e.to_string())?;
    let map = json.as_object_mut().expect("tagged enum is an object");
    let old = map
        .get(param)
        .cloned()
        .ok_or_else(|| format!("part has no parameter '{param}'"))?;
    map.insert(param.to_string(), value);
    *kind = serde_json::from_value(json).map_err(|e| format!("invalid value for '{param}': {e}"))?;
    Ok(old)
}

/// Same patch-through-serde trick for the flat Design, with dotted paths
/// for the chute sub-object.
fn patch_design(design: &mut Design, param: &str, value: Value) -> Result<Value, String> {
    let mut json = serde_json::to_value(&*design).map_err(|e| e.to_string())?;
    let target = match param.strip_prefix("chute.") {
        Some(sub) => json
            .get_mut("chute")
            .and_then(Value::as_object_mut)
            .map(|m| (m, sub.to_string())),
        None => json.as_object_mut().map(|m| (m, param.to_string())),
    };
    let (map, key) = target.ok_or_else(|| format!("bad parameter path '{param}'"))?;
    if key == "motor_designation" {
        return Err("use select_motor to change the motor".into());
    }
    let old = map
        .get(&key)
        .cloned()
        .ok_or_else(|| format!("design has no parameter '{param}'"))?;
    map.insert(key, value);
    *design = serde_json::from_value(json).map_err(|e| format!("invalid value for '{param}': {e}"))?;
    Ok(old)
}
