//! Headless runner logic (v0.3 Step 8). The CLI binary is a thin shell
//! over these functions, so everything the CLI can do is unit-tested
//! here without process spawning. This is also the CI story: a journal
//! replay or a study run is a deterministic, hash-stamped artifact that
//! a pipeline can diff.

use std::path::Path;

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

/// `ascent-cli compare-flight <case> --output <directory>`: run the executed,
/// non-calibrated flight comparison for `case` and write a self-contained
/// evidence directory.
///
/// The comparison itself lives in `ascent_review::ndrt_2020_export` — this
/// function only resolves the selector, does the filesystem I/O, and turns the
/// outcome into the CLI's `Result<String, String>` contract, so an integrity,
/// execution, or artifact-validation failure surfaces as a nonzero exit.
///
/// A *failing* comparison is still exported in full, then reported as an error:
/// the evidence is written before the failure is raised, never suppressed.
pub fn compare_flight(
    case_ref: &str,
    output_dir: &Path,
    generated_at_unix_s: u64,
) -> Result<String, String> {
    use ascent_review::ndrt_2020_export as export;

    if case_ref != export::CASE_SELECTOR {
        return Err(format!(
            "unknown flight comparison '{case_ref}' (available: {})",
            export::CASE_SELECTOR
        ));
    }
    let exported = export::run_canonical_export(generated_at_unix_s)?;

    std::fs::create_dir_all(output_dir)
        .map_err(|error| format!("cannot create {}: {error}", output_dir.display()))?;
    for file in &exported.files {
        let path = output_dir.join(file.name);
        std::fs::write(&path, &file.bytes)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }

    let report = format!(
        "{}\n  wrote {} files to {}\n",
        exported.terminal_summary,
        exported.files.len(),
        output_dir.display()
    );
    if !exported.pass() {
        // Written, then failed: the directory is on disk and the failure is loud.
        return Err(format!(
            "flight comparison FAILED its primary metrics — evidence written to {}\n\n{report}",
            output_dir.display()
        ));
    }
    Ok(report)
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
    ascent-cli compare-flight ndrt-2020 --output <directory>
                                          run the executed, non-calibrated flight
                                          comparison against the hash-pinned NDRT
                                          2020 measured flight and write a
                                          self-contained evidence directory
                                          (comparison artifact, Markdown summary,
                                          provenance manifest, case copy)
";
