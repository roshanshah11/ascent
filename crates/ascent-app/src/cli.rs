//! Headless runner logic (v0.3 Step 8). The CLI binary is a thin shell
//! over these functions, so everything the CLI can do is unit-tested
//! here without process spawning. This is also the CI story: a journal
//! replay or a study run is a deterministic, hash-stamped artifact that
//! a pipeline can diff.

use crate::design::Design;
use crate::dispersion_ipc::{self, DispersionRequest};
use crate::document::Document;
use crate::project::{from_toml, Project};
use crate::study::{study_input_hash, Study, StudyKind};
use ascent_domain::vehicle::{reference_vehicle, Vehicle};
use ascent_sim::{Variation, VaryParam};
use serde::Serialize;

/// `ascent-cli replay <session.jsonl>`: rebuild the document from a
/// journal and return its canonical byte form. Byte-equality with the
/// live session's `canonical_bytes()` is the determinism proof.
pub fn replay_journal(jsonl: &str) -> Result<String, String> {
    Ok(Document::replay(jsonl)?.canonical_bytes())
}

/// Output of `ascent-cli run-study`: the results plus enough provenance
/// to verify them independently.
#[derive(Debug, Serialize)]
pub struct StudyRunOutput {
    pub project: String,
    pub study: String,
    pub engine: String,
    pub seed: u64,
    /// Content hash of (vehicle, design, study config) — the same hash
    /// the GUI's job runner stamps, so results are comparable across
    /// entry points.
    pub input_hash: String,
    pub results: serde_json::Value,
}

fn resolve_study<'a>(project: &'a Project, reference: &str) -> Result<&'a Study, String> {
    if let Ok(id) = reference.parse::<u32>() {
        if let Some(study) = project.studies.iter().find(|s| s.id.0 == id) {
            return Ok(study);
        }
    }
    let mut by_name = project.studies.iter().filter(|s| s.name == reference);
    match (by_name.next(), by_name.next()) {
        (Some(study), None) => Ok(study),
        (Some(_), Some(_)) => Err(format!(
            "study name '{reference}' is ambiguous — use its id"
        )),
        (None, _) => Err(format!(
            "no study '{reference}' in project (studies: {})",
            project
                .studies
                .iter()
                .map(|s| format!("{} '{}'", s.id.0, s.name))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The same fixed variation set the job runner uses (jobs.rs) — one
/// convention, one hash meaning, until StudyKind carries variations.
fn dispersion_request(seed: u64, flights: u32) -> DispersionRequest {
    DispersionRequest {
        seed,
        samples: flights,
        vary: vec![
            Variation {
                param: VaryParam::ThrustPct,
                sigma: 3.0,
            },
            Variation {
                param: VaryParam::WindSpeedMs,
                sigma: 1.5,
            },
        ],
        base_wind_ms: 3.0,
    }
}

/// `ascent-cli run-study <project.ascent> <study-id-or-name>`: run the
/// study headless and return the results JSON, hash-stamped.
pub fn run_study(project_toml: &str, study_ref: &str) -> Result<String, String> {
    let project = from_toml(project_toml)?;
    let study = resolve_study(&project, study_ref)?;
    let design: &Design = project.designs.first().ok_or("project has no designs")?;
    let vehicle: Vehicle = project.vehicle.clone().unwrap_or_else(reference_vehicle);

    let StudyKind::Dispersion { flights } = study.kind else {
        return Err(format!(
            "study '{}' is not runnable headless yet (only dispersion studies run today)",
            study.name
        ));
    };
    // The headless TOML project format carries no imported atmosphere yet;
    // CLI runs are analytic-atmosphere runs and hash accordingly.
    let summary = dispersion_ipc::run(
        &vehicle,
        design,
        None,
        &dispersion_request(study.seed, flights),
    )?;
    let output = StudyRunOutput {
        project: project.name.clone(),
        study: study.name.clone(),
        engine: study.engine.clone(),
        seed: study.seed,
        input_hash: study_input_hash(&vehicle, design, None, study),
        results: serde_json::to_value(&summary).map_err(|e| e.to_string())?,
    };
    serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
}

pub const USAGE: &str = "\
ascent-cli — headless Ascent runner

USAGE:
    ascent-cli replay <session.jsonl>     rebuild a document from a journal,
                                          print its canonical JSON to stdout
    ascent-cli run-study <project.ascent> <study-id-or-name>
                                          run a study headless, print
                                          hash-stamped results JSON to stdout
    ascent-cli create-review-bundle <output.ascent-review>
                                          create the deterministic golden
                                          offline review smoke fixture
    ascent-cli verify-review-bundle <bundle.ascent-review>
                                          verify hashes and exact journal reopen
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::project::to_toml;
    use crate::study::StudyId;
    use serde_json::json;

    fn session() -> Document {
        let mut doc = Document::default();
        doc.dispatch(Command::SetSimParam {
            param: "cd".into(),
            value: json!(0.7),
        })
        .unwrap();
        doc.dispatch(Command::CreateStudy {
            name: "spread".into(),
            kind: StudyKind::Dispersion { flights: 30 },
            engine: "native".into(),
            seed: 42,
        })
        .unwrap();
        doc.undo();
        doc.redo();
        doc
    }

    #[test]
    fn replay_reproduces_the_session_byte_identically() {
        let doc = session();
        let replayed = replay_journal(&doc.journal_jsonl()).unwrap();
        assert_eq!(replayed, doc.canonical_bytes());
    }

    #[test]
    fn replay_reports_journal_errors() {
        let err = replay_journal("{\"journal_version\":9}\n").unwrap_err();
        assert!(err.contains("version 9"));
    }

    fn project_with_study() -> Project {
        let mut project = Project::new("cli bird");
        project.studies.push(Study {
            id: StudyId(1),
            name: "spread".into(),
            kind: StudyKind::Dispersion { flights: 25 },
            engine: "native".into(),
            seed: 42,
            results: None,
        });
        project
    }

    #[test]
    fn run_study_is_deterministic_and_hash_stamped() {
        let toml = to_toml(&project_with_study()).unwrap();
        let a = run_study(&toml, "spread").unwrap();
        let b = run_study(&toml, "1").unwrap();
        assert_eq!(
            a, b,
            "same project + study must be byte-identical, by name or id"
        );

        let parsed: serde_json::Value = serde_json::from_str(&a).unwrap();
        assert_eq!(parsed["study"], "spread");
        assert_eq!(parsed["seed"], 42);
        assert_eq!(parsed["results"]["samples"], 25);
        let hash = parsed["input_hash"].as_str().unwrap();
        assert_eq!(hash.len(), 64, "sha-256 hex");

        // The stamp matches what the GUI's job runner would compute for
        // the same inputs.
        let project = project_with_study();
        let expected = study_input_hash(
            &reference_vehicle(),
            &project.designs[0],
            None,
            &project.studies[0],
        );
        assert_eq!(hash, expected);
    }

    #[test]
    fn run_study_errors_are_actionable() {
        let toml = to_toml(&project_with_study()).unwrap();
        let err = run_study(&toml, "nope").unwrap_err();
        assert!(err.contains("no study 'nope'"), "{err}");
        assert!(err.contains("1 'spread'"), "error lists what exists: {err}");

        let mut project = project_with_study();
        project.studies[0].kind = StudyKind::SingleFlight;
        let err = run_study(&to_toml(&project).unwrap(), "spread").unwrap_err();
        assert!(err.contains("not runnable"), "{err}");
    }
}
