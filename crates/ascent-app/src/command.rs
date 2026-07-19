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

// ---- Text form (v0.3 Step 8) -------------------------------------------
//
// The canonical text grammar per docs/JOURNAL_FORMAT.md: a kebab-case
// verb, simple positional tokens, and JSON for structured payloads. The
// console and the CLI both parse to this same `Command` enum and go
// through the same dispatcher — no second mutation path, no second
// validation story. `parse_text(to_text(cmd)) == cmd` for every variant.

impl Command {
    /// Canonical text form of this command.
    pub fn to_text(&self) -> String {
        let json = |v: &dyn ErasedSer| v.to_json();
        // (helper trait below keeps the match arms readable)
        match self {
            Command::AddPart { parent, kind } => {
                format!("add-part {} {}", opt_id(parent), json(kind))
            }
            Command::RemovePart { id } => format!("remove-part {}", id.0),
            Command::RestorePart {
                parent,
                index,
                part,
            } => format!("restore-part {} {} {}", opt_id(parent), index, json(part)),
            Command::SetPartParam { id, param, value } => {
                format!("set-part-param {} {} {}", id.0, param, json(value))
            }
            Command::SetSimParam { param, value } => {
                format!("set-sim-param {} {}", param, json(value))
            }
            Command::SelectMotor { designation } => format!("select-motor {designation}"),
            Command::SetDesign { design } => format!("set-design {}", json(design)),
            Command::CreateStudy {
                name,
                kind,
                engine,
                seed,
            } => format!(
                "create-study {} {} {} {}",
                serde_json::Value::from(name.clone()),
                engine,
                seed,
                json(kind)
            ),
            Command::DeleteStudy { id } => format!("delete-study {}", id.0),
            Command::RestoreStudy { index, study } => {
                format!("restore-study {} {}", index, json(study))
            }
            Command::SetStudyParam { id, param, value } => {
                format!("set-study-param {} {} {}", id.0, param, json(value))
            }
            Command::SetStudyResults { id, results } => format!(
                "set-study-results {} {}",
                id.0,
                match results {
                    Some(r) => r.to_json(),
                    None => "null".into(),
                }
            ),
        }
    }

    /// Parse the canonical text form. Errors name what was expected.
    pub fn parse_text(line: &str) -> Result<Command, String> {
        let line = line.trim();
        let (verb, rest) = split_token(line);
        if verb.is_empty() {
            return Err("empty command".into());
        }
        match verb {
            "add-part" => {
                let (parent, kind_json) = split_token(rest);
                Ok(Command::AddPart {
                    parent: parse_opt_id(parent)?,
                    kind: parse_json("part kind", kind_json)?,
                })
            }
            "remove-part" => Ok(Command::RemovePart {
                id: PartId(parse_u32("part id", rest)?),
            }),
            "restore-part" => {
                let (parent, rest) = split_token(rest);
                let (index, part_json) = split_token(rest);
                Ok(Command::RestorePart {
                    parent: parse_opt_id(parent)?,
                    index: parse_usize("index", index)?,
                    part: parse_json("part", part_json)?,
                })
            }
            "set-part-param" => {
                let (id, rest) = split_token(rest);
                let (param, value_json) = split_token(rest);
                Ok(Command::SetPartParam {
                    id: PartId(parse_u32("part id", id)?),
                    param: require("parameter name", param)?,
                    value: parse_json("value", value_json)?,
                })
            }
            "set-sim-param" => {
                let (param, value_json) = split_token(rest);
                Ok(Command::SetSimParam {
                    param: require("parameter name", param)?,
                    value: parse_json("value", value_json)?,
                })
            }
            "select-motor" => Ok(Command::SelectMotor {
                designation: require("motor designation", rest)?,
            }),
            "set-design" => Ok(Command::SetDesign {
                design: parse_json("design", rest)?,
            }),
            "create-study" => {
                // Name is a JSON string (may contain spaces), then engine
                // token, seed, and the kind object.
                let (name_json, rest) = split_json_prefix(rest)?;
                let name: String =
                    serde_json::from_str(&name_json).map_err(|e| format!("bad study name: {e}"))?;
                let (engine, rest) = split_token(rest);
                let (seed, kind_json) = split_token(rest);
                Ok(Command::CreateStudy {
                    name,
                    engine: require("engine", engine)?,
                    seed: parse_u64("seed", seed)?,
                    kind: parse_json("study kind", kind_json)?,
                })
            }
            "delete-study" => Ok(Command::DeleteStudy {
                id: StudyId(parse_u32("study id", rest)?),
            }),
            "restore-study" => {
                let (index, study_json) = split_token(rest);
                Ok(Command::RestoreStudy {
                    index: parse_usize("index", index)?,
                    study: parse_json("study", study_json)?,
                })
            }
            "set-study-param" => {
                let (id, rest) = split_token(rest);
                let (param, value_json) = split_token(rest);
                Ok(Command::SetStudyParam {
                    id: StudyId(parse_u32("study id", id)?),
                    param: require("parameter name", param)?,
                    value: parse_json("value", value_json)?,
                })
            }
            "set-study-results" => {
                let (id, results_json) = split_token(rest);
                Ok(Command::SetStudyResults {
                    id: StudyId(parse_u32("study id", id)?),
                    results: parse_json("results", results_json)?,
                })
            }
            other => Err(format!("unknown command '{other}'")),
        }
    }
}

/// Tiny object-safe serialization helper so `to_text` reads flat.
trait ErasedSer {
    fn to_json(&self) -> String;
}

impl<T: Serialize> ErasedSer for T {
    fn to_json(&self) -> String {
        serde_json::to_string(self).expect("command payload serializes")
    }
}

fn opt_id(id: &Option<PartId>) -> String {
    match id {
        Some(p) => p.0.to_string(),
        None => "-".into(),
    }
}

fn split_token(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    match s.find(char::is_whitespace) {
        Some(at) => (&s[..at], s[at..].trim_start()),
        None => (s, ""),
    }
}

/// Split a leading JSON value (used for the quoted study name) from the
/// rest of the line, using serde's own parser to find the boundary.
fn split_json_prefix(s: &str) -> Result<(String, &str), String> {
    let s = s.trim_start();
    let mut stream = serde_json::Deserializer::from_str(s).into_iter::<Value>();
    let value = stream
        .next()
        .ok_or("expected a JSON value")?
        .map_err(|e| format!("bad JSON: {e}"))?;
    let consumed = stream.byte_offset();
    Ok((value.to_string(), &s[consumed..]))
}

fn require(what: &str, token: &str) -> Result<String, String> {
    if token.is_empty() {
        Err(format!("expected {what}"))
    } else {
        Ok(token.to_string())
    }
}

fn parse_opt_id(token: &str) -> Result<Option<PartId>, String> {
    if token == "-" {
        Ok(None)
    } else {
        Ok(Some(PartId(parse_u32("parent id (or '-')", token)?)))
    }
}

fn parse_u32(what: &str, token: &str) -> Result<u32, String> {
    token
        .parse()
        .map_err(|_| format!("expected {what}, got '{token}'"))
}

fn parse_u64(what: &str, token: &str) -> Result<u64, String> {
    token
        .parse()
        .map_err(|_| format!("expected {what}, got '{token}'"))
}

fn parse_usize(what: &str, token: &str) -> Result<usize, String> {
    token
        .parse()
        .map_err(|_| format!("expected {what}, got '{token}'"))
}

fn parse_json<T: serde::de::DeserializeOwned>(what: &str, s: &str) -> Result<T, String> {
    if s.trim().is_empty() {
        return Err(format!("expected {what} as JSON"));
    }
    serde_json::from_str(s.trim()).map_err(|e| format!("bad {what}: {e}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::study::{Study, StudyId, StudyKind, StudyResults};
    use ascent_domain::vehicle::NoseShape;
    use serde_json::json;

    fn every_variant() -> Vec<Command> {
        vec![
            Command::AddPart {
                parent: Some(PartId(2)),
                kind: PartKind::FinSet {
                    count: 4,
                    root_chord_m: 0.04,
                    tip_chord_m: 0.02,
                    span_m: 0.03,
                    sweep_m: 0.0,
                    thickness_mm: 2.0,
                    mass_g: 5.0,
                },
            },
            Command::AddPart {
                parent: None,
                kind: PartKind::NoseCone {
                    shape: NoseShape::Conical,
                    length_m: 0.05,
                    base_radius_m: 0.0125,
                    mass_g: 4.0,
                },
            },
            Command::AddPart {
                parent: None,
                kind: PartKind::StageCoupler {
                    length_m: 0.02,
                    outer_radius_m: 0.0125,
                    mass_g: 4.0,
                    separation_delay_s: 0.5,
                },
            },
            Command::RemovePart { id: PartId(3) },
            Command::RestorePart {
                parent: Some(PartId(2)),
                index: 1,
                part: Part {
                    id: PartId(7),
                    kind: PartKind::MassComponent {
                        name: "payload".into(),
                        position_m: 0.1,
                        mass_g: 12.0,
                    },
                    children: vec![],
                },
            },
            Command::SetPartParam {
                id: PartId(3),
                param: "root_chord_m".into(),
                value: json!(0.06),
            },
            Command::SetSimParam {
                param: "chute.diameter_cm".into(),
                value: json!(35.0),
            },
            Command::SelectMotor {
                designation: "B6".into(),
            },
            Command::SetDesign {
                design: Design::reference(),
            },
            Command::CreateStudy {
                name: "landing spread study".into(),
                kind: StudyKind::Dispersion { flights: 1000 },
                engine: "native".into(),
                seed: 42,
            },
            Command::DeleteStudy { id: StudyId(1) },
            Command::RestoreStudy {
                index: 0,
                study: Study {
                    id: StudyId(1),
                    name: "restored".into(),
                    kind: StudyKind::SingleFlight,
                    engine: "native".into(),
                    seed: 7,
                    results: Some(StudyResults {
                        input_hash: "abc".into(),
                        data: json!({ "p50": 300.0 }),
                    }),
                },
            },
            Command::SetStudyParam {
                id: StudyId(1),
                param: "seed".into(),
                value: json!(9),
            },
            Command::SetStudyResults {
                id: StudyId(1),
                results: None,
            },
            Command::SetStudyResults {
                id: StudyId(1),
                results: Some(StudyResults {
                    input_hash: "def".into(),
                    data: json!({ "n": 1 }),
                }),
            },
        ]
    }

    #[test]
    fn text_grammar_roundtrips_every_command_variant() {
        for cmd in every_variant() {
            let text = cmd.to_text();
            let parsed = Command::parse_text(&text)
                .unwrap_or_else(|e| panic!("parse failed for '{text}': {e}"));
            assert_eq!(parsed, cmd, "roundtrip mismatch for '{text}'");
        }
    }

    #[test]
    fn text_forms_are_the_documented_shapes() {
        assert_eq!(
            Command::SetPartParam {
                id: PartId(3),
                param: "root_chord_m".into(),
                value: json!(0.05),
            }
            .to_text(),
            "set-part-param 3 root_chord_m 0.05"
        );
        assert_eq!(
            Command::SelectMotor { designation: "C6".into() }.to_text(),
            "select-motor C6"
        );
        assert_eq!(Command::RemovePart { id: PartId(4) }.to_text(), "remove-part 4");
    }

    #[test]
    fn parse_errors_name_what_was_expected() {
        for (line, needle) in [
            ("", "empty command"),
            ("warp-drive 1", "unknown command"),
            ("remove-part x", "part id"),
            ("set-part-param 3 root_chord_m", "expected value as JSON"),
            ("set-part-param 3 root_chord_m not-json", "bad value"),
            ("add-part 2", "part kind"),
            ("select-motor", "motor designation"),
            ("create-study \"a\" native notanumber {\"kind\":\"single_flight\"}", "seed"),
        ] {
            let err = Command::parse_text(line).unwrap_err();
            assert!(err.contains(needle), "'{line}' → '{err}' (wanted '{needle}')");
        }
    }

    #[test]
    fn parsed_text_dispatches_like_the_gui() {
        use crate::document::Document;
        let mut via_text = Document::default();
        let mut via_enum = Document::default();
        let script = [
            "set-sim-param cd 0.7",
            "select-motor B6",
            "create-study \"spread\" native 42 {\"kind\":\"dispersion\",\"flights\":100}",
            "set-study-param 1 seed 9",
        ];
        for line in script {
            via_text.dispatch(Command::parse_text(line).unwrap()).unwrap();
        }
        via_enum
            .dispatch(Command::SetSimParam { param: "cd".into(), value: json!(0.7) })
            .unwrap();
        via_enum
            .dispatch(Command::SelectMotor { designation: "B6".into() })
            .unwrap();
        via_enum
            .dispatch(Command::CreateStudy {
                name: "spread".into(),
                kind: StudyKind::Dispersion { flights: 100 },
                engine: "native".into(),
                seed: 42,
            })
            .unwrap();
        via_enum
            .dispatch(Command::SetStudyParam {
                id: StudyId(1),
                param: "seed".into(),
                value: json!(9),
            })
            .unwrap();
        assert_eq!(via_text.canonical_bytes(), via_enum.canonical_bytes());
    }
}
