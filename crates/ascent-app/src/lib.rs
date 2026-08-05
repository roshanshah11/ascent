//! Tauri backend: coarse IPC only. The UI sends a whole Design and gets a
//! whole RunRecord back — no fine-grained property calls across the bridge.
//! All physics stays in ascent-domain / ascent-sim; this crate only maps
//! DTOs and downsamples the trajectory for playback.
//!
//! The curated non-IPC surface (what ascent-cli and ascent-mcp build on):
//! [`Command`] + the text grammar, [`Document`] / [`DocumentState`], the
//! copilot seam ([`propose_batch`], [`apply_batch`], [`Proposal`]), studies
//! ([`Study`], [`StudyKind`], [`study_input_hash`], [`run_study_now`],
//! [`JobRunner`]), design/run DTOs ([`Design`], [`run_design`],
//! [`RunRecord`]), projects ([`to_toml`] / [`from_toml`]), evidence
//! ([`evidence_for`]), review ([`review_report`], [`review_repair`]),
//! viewport markers ([`vehicle_markers`]), and staged-flight derivation
//! ([`planar_stages_from_tree`], [`sixdof_stages_from_tree`]).

//! Application services and Tauri IPC for the Ascent desktop workbench.
//!
//! UI, CLI, and MCP consumers should use the curated exports in this crate;
//! command application, persistence, and IPC implementation modules remain internal.

mod bundle;
mod cinema;
pub mod cli;
mod command;
mod credibility;
mod design;
mod dispersion_ipc;
mod document;
mod evidence;
mod jobs;
mod markers;
mod project;
mod propose;
mod report;
mod review_ipc;
mod staging;
mod study;

pub use bundle::{
    BundleMember, MissionReviewBundle, ReopenedReview, ReviewBundleInput, ReviewBundleManifest,
    REVIEW_BUNDLE_VERSION,
};
pub use cinema::{CampaignCinema, CinemaCheckpoint, StoryBeat};
pub use command::{command_grammar, Command, CommandGrammarEntry};
pub use credibility::{Factor, QuantityFlag, Regime, Scorecard};
pub use design::{run_design, Design, ImportedMotor, MotorInfo, RunRecord, SpreadResult};
pub use dispersion_ipc::DispersionRequest;
pub use document::{DecisionMetadata, Document, DocumentState};
pub use evidence::{evidence_for, evidence_for_study, EvidenceReport};
pub use jobs::{run_study_now, JobEvent, JobId, JobRunner, JobStatus, JobView};
pub use markers::{vehicle_markers, VehicleMarkers};
pub use project::{from_toml, to_toml, Project};
pub use propose::{
    apply_batch, build_counterfactual_review, propose_batch, CommandCheck, CounterfactualReview,
    DiffSummary, Proposal,
};
pub use report::{flight_readiness_html, flight_readiness_markdown};
pub use review_ipc::{repair as review_repair, report as review_report, ReviewReport};
pub use staging::{planar_stages_from_tree, sixdof_stages_from_tree};
pub use study::{study_input_hash, Study, StudyId, StudyKind, StudyResults};

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
struct PartPreview {
    markers: VehicleMarkers,
    apogee_m: f64,
}

#[tauri::command]
fn reference_design() -> Design {
    Design::reference()
}

/// The document is shared between IPC commands and the job runner's
/// worker thread, so it lives behind an Arc.
type SharedDocument = std::sync::Arc<std::sync::Mutex<Document>>;
type DocState<'a> = tauri::State<'a, SharedDocument>;

fn doc_lock<'a>(state: &'a DocState<'_>) -> std::sync::MutexGuard<'a, Document> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[tauri::command]
fn get_document(state: DocState) -> DocumentState {
    doc_lock(&state).state()
}

/// Read-only discovery surface for the canonical Rust command grammar.
#[tauri::command]
fn command_catalogue() -> Vec<CommandGrammarEntry> {
    command_grammar().to_vec()
}

/// Read-only viewport overlay query: CP/CG stations for the current tree
/// and motor. Never mutates; the render layer stays command-free.
#[tauri::command]
fn get_vehicle_markers(state: DocState) -> Result<markers::VehicleMarkers, String> {
    let doc = doc_lock(&state);
    markers::vehicle_markers(&doc.vehicle, &doc.design)
}

fn preview_part_param_document(
    document: &Document,
    id: u32,
    param: String,
    value: serde_json::Value,
) -> Result<PartPreview, String> {
    let mut preview = document.clone();
    preview.dispatch(Command::SetPartParam {
        id: ascent_domain::vehicle::PartId(id),
        param,
        value,
    })?;
    let markers = markers::vehicle_markers(&preview.vehicle, &preview.design)?;
    let flight = dispersion_ipc::run(
        &preview.vehicle,
        &preview.design,
        preview.atmosphere.as_ref(),
        &DispersionRequest {
            seed: 0,
            samples: 1,
            vary: Vec::new(),
            base_wind_ms: 0.0,
        },
    )?;
    Ok(PartPreview {
        markers,
        apogee_m: flight.apogee_p50_m,
    })
}

/// Read-only physics preview for a prospective part edit. The command is
/// applied only to a cloned document, so preview traffic never reaches the
/// journal, undo stack, or study staleness machinery.
#[tauri::command]
fn preview_part_param(
    state: DocState,
    id: u32,
    param: String,
    value: serde_json::Value,
) -> Result<PartPreview, String> {
    preview_part_param_document(&doc_lock(&state), id, param, value)
}

#[tauri::command]
fn dispatch_command(state: DocState, command: Command) -> Result<DocumentState, String> {
    let mut doc = doc_lock(&state);
    doc.dispatch(command)?;
    Ok(doc.state())
}

#[tauri::command]
fn undo_document(state: DocState) -> DocumentState {
    let mut doc = doc_lock(&state);
    doc.undo();
    doc.state()
}

#[tauri::command]
fn redo_document(state: DocState) -> DocumentState {
    let mut doc = doc_lock(&state);
    doc.redo();
    doc.state()
}

#[tauri::command]
fn session_journal(state: DocState) -> String {
    doc_lock(&state).journal_jsonl()
}

/// Offline review-bundle reopen seam. Every member hash and journal prefix is
/// verified before the canonical document replaces live state.
#[tauri::command]
fn open_review_bundle(state: DocState, bytes: Vec<u8>) -> Result<DocumentState, String> {
    let reopened = MissionReviewBundle::reopen(&bytes)?;
    let mut document = doc_lock(&state);
    *document = reopened.document;
    Ok(document.state())
}

/// Console line → parse the text grammar → the same dispatcher every
/// other client uses. No second mutation path.
#[tauri::command]
fn console_exec(state: DocState, line: String) -> Result<DocumentState, String> {
    let mut doc = doc_lock(&state);
    console_exec_document(&mut doc, &line)
}

fn console_exec_document(doc: &mut Document, line: &str) -> Result<DocumentState, String> {
    let command = Command::parse_text(line)?;
    doc.dispatch(command)?;
    Ok(doc.state())
}

/// Import an atmosphere profile from CSV text the user picked (the file
/// dialog and read happen in the frontend). Parses, validates, and lands
/// the profile through the dispatcher as a journaled `set-atmosphere` —
/// the CSV file itself is never referenced again after this call.
#[tauri::command]
fn import_atmosphere(state: DocState, name: String, csv: String) -> Result<DocumentState, String> {
    let profile = ascent_sim::AtmosphereProfile::from_csv(&name, &csv)?;
    let mut doc = doc_lock(&state);
    doc.dispatch(Command::SetAtmosphere {
        profile: Some(profile),
    })?;
    Ok(doc.state())
}

/// Copilot seam: dry-run a command batch, mutating nothing. Batches are
/// journal-grammar text lines, one command per entry.
#[tauri::command]
fn propose_commands(state: DocState, lines: Vec<String>) -> Result<Proposal, String> {
    let commands = parse_batch(&lines)?;
    Ok(propose_batch(&doc_lock(&state), &commands))
}

/// Complete read-only proposal universe for synchronized geometry, flight,
/// evidence, residual, and report preview surfaces.
#[tauri::command]
fn counterfactual_review(
    state: DocState,
    lines: Vec<String>,
) -> Result<CounterfactualReview, String> {
    let commands = parse_batch(&lines)?;
    build_counterfactual_review(&doc_lock(&state), &commands)
}

/// Copilot seam: apply a previously proposed batch atomically. The batch
/// is re-validated first; on any error the document is untouched.
#[tauri::command]
fn apply_proposal(state: DocState, lines: Vec<String>) -> Result<DocumentState, String> {
    let commands = parse_batch(&lines)?;
    let mut doc = doc_lock(&state);
    apply_batch(&mut doc, &commands)?;
    Ok(doc.state())
}

fn parse_batch(lines: &[String]) -> Result<Vec<Command>, String> {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| Command::parse_text(line).map_err(|e| format!("line {}: {e}", i + 1)))
        .collect()
}

#[tauri::command]
fn list_motors() -> Vec<MotorInfo> {
    design::bundled_motors()
}

#[tauri::command]
fn run_simulation(state: DocState, design: Design) -> Result<RunRecord, String> {
    let atmosphere = doc_lock(&state).atmosphere.clone();
    design::run_design_with_atmosphere(&design, atmosphere.as_ref())
}

#[tauri::command]
fn run_spread(design: Design) -> Result<SpreadResult, String> {
    design::run_spread(&design)
}

#[tauri::command]
fn import_motor_file(contents: String) -> Result<ImportedMotor, String> {
    design::import_motor_file(&contents)
}

#[tauri::command]
fn run_evidence(design: Design) -> Result<EvidenceReport, String> {
    evidence_for(&design)
}

#[tauri::command]
fn run_dispersion(
    state: DocState,
    design: Design,
    request: DispersionRequest,
) -> Result<ascent_sim::DispersionSummary, String> {
    let (tree, atmosphere) = {
        let doc = doc_lock(&state);
        (doc.vehicle.clone(), doc.atmosphere.clone())
    };
    dispersion_ipc::run(&tree, &design, atmosphere.as_ref(), &request)
}

#[tauri::command]
fn save_project(project: Project) -> Result<String, String> {
    project::to_toml(&project)
}

#[tauri::command]
fn load_project(contents: String) -> Result<Project, String> {
    project::from_toml(&contents)
}

#[tauri::command]
fn autosave_project(project: Project) -> Result<String, String> {
    project::autosave(&project).map(|p| p.display().to_string())
}

/// A surviving autosave from a crashed session, if any.
#[tauri::command]
fn check_recovery() -> Option<Project> {
    project::find_recovery().map(|(_, p)| p)
}

#[tauri::command]
fn discard_recovery() -> Result<(), String> {
    project::discard_recovery()
}

#[tauri::command]
fn flight_review(state: DocState, target_apogee_m: f64) -> Result<ReviewReport, String> {
    let doc = doc_lock(&state);
    review_ipc::report_for(&doc.vehicle, &doc.design, target_apogee_m)
}

/// Deterministic flight-readiness report as Markdown. The structural/rule
/// review is computed from the live tree; if that computation fails (e.g.
/// the vehicle has no fin thickness yet), the report still renders with
/// those sections marked "not yet computed" rather than erroring out.
#[tauri::command]
fn flight_readiness(state: DocState, target_apogee_m: f64) -> String {
    let doc = doc_lock(&state);
    let review = review_ipc::report_for(&doc.vehicle, &doc.design, target_apogee_m).ok();
    report::flight_readiness_markdown(&doc, review.as_ref())
}

#[tauri::command]
fn solve_review(state: DocState, target_apogee_m: f64) -> Result<ascent_review::Repair, String> {
    let doc = doc_lock(&state);
    review_ipc::repair_for(&doc.vehicle, &doc.design, target_apogee_m)
}

#[tauri::command]
fn enqueue_study_job(
    runner: tauri::State<std::sync::Arc<JobRunner>>,
    study_id: StudyId,
) -> Result<JobId, String> {
    runner.enqueue(study_id)
}

#[tauri::command]
fn cancel_job(
    runner: tauri::State<std::sync::Arc<JobRunner>>,
    job_id: JobId,
) -> Result<(), String> {
    runner.cancel(job_id)
}

#[tauri::command]
fn list_jobs(runner: tauri::State<std::sync::Arc<JobRunner>>) -> Vec<JobView> {
    runner.jobs()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_document = std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "ascent-review")
        })
        .map(|path| load_review_document(&path).unwrap_or_else(|error| panic!("{error}")))
        .unwrap_or_default();
    let doc: SharedDocument = std::sync::Arc::new(std::sync::Mutex::new(initial_document));
    tauri::Builder::default()
        .manage(doc.clone())
        .setup(move |app| {
            use tauri::{Emitter, Manager};
            let handle = app.handle().clone();
            let runner = JobRunner::new(doc, move |event| {
                let channel = match &event {
                    JobEvent::Progress { .. } => "job-progress",
                    JobEvent::Done { .. } => "job-done",
                };
                let _ = handle.emit(channel, &event);
            });
            app.manage(runner);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            reference_design,
            get_document,
            command_catalogue,
            get_vehicle_markers,
            preview_part_param,
            dispatch_command,
            undo_document,
            redo_document,
            session_journal,
            open_review_bundle,
            console_exec,
            import_atmosphere,
            propose_commands,
            counterfactual_review,
            apply_proposal,
            list_motors,
            run_simulation,
            run_spread,
            import_motor_file,
            run_evidence,
            run_dispersion,
            save_project,
            load_project,
            autosave_project,
            check_recovery,
            discard_recovery,
            flight_review,
            flight_readiness,
            solve_review,
            enqueue_study_job,
            cancel_job,
            list_jobs
        ])
        .run(tauri::generate_context!())
        .expect("error while running ascent");
}

fn load_review_document(path: &std::path::Path) -> Result<Document, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read review bundle {}: {error}", path.display()))?;
    MissionReviewBundle::reopen(&bytes)
        .map(|reopened| reopened.document)
        .map_err(|error| format!("cannot open review bundle {}: {error}", path.display()))
}
