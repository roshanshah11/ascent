//! Tauri backend: coarse IPC only. The UI sends a whole Design and gets a
//! whole RunRecord back — no fine-grained property calls across the bridge.
//! All physics stays in ascent-domain / ascent-sim; this crate only maps
//! DTOs and downsamples the trajectory for playback.

mod command;
mod credibility;
mod design;
mod dispersion_ipc;
mod document;
mod evidence;
mod project;
mod review_ipc;
mod study;

pub use command::Command;
pub use credibility::{Factor, QuantityFlag, Regime, Scorecard};
pub use document::{Document, DocumentState};
pub use study::{study_input_hash, Study, StudyId, StudyKind, StudyResults};
pub use design::{run_design, Design, ImportedMotor, MotorInfo, RunRecord, SpreadResult};
pub use dispersion_ipc::DispersionRequest;
pub use project::{from_toml, to_toml, Project};
pub use evidence::{evidence_for, EvidenceReport};
pub use review_ipc::{repair as review_repair, report as review_report, ReviewReport};

#[tauri::command]
fn reference_design() -> Design {
    Design::reference()
}

type DocState<'a> = tauri::State<'a, std::sync::Mutex<Document>>;

fn doc_lock<'a>(state: &'a DocState<'_>) -> std::sync::MutexGuard<'a, Document> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[tauri::command]
fn get_document(state: DocState) -> DocumentState {
    doc_lock(&state).state()
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

#[tauri::command]
fn list_motors() -> Vec<MotorInfo> {
    design::bundled_motors()
}

#[tauri::command]
fn run_simulation(design: Design) -> Result<RunRecord, String> {
    run_design(&design)
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
    let tree = doc_lock(&state).vehicle.clone();
    dispersion_ipc::run(&tree, &design, &request)
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
fn flight_review(target_apogee_m: f64) -> Result<ReviewReport, String> {
    review_ipc::report(target_apogee_m)
}

#[tauri::command]
fn solve_review(target_apogee_m: f64) -> Result<ascent_review::Repair, String> {
    review_ipc::repair(target_apogee_m)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(std::sync::Mutex::new(Document::default()))
        .invoke_handler(tauri::generate_handler![
            reference_design,
            get_document,
            dispatch_command,
            undo_document,
            redo_document,
            session_journal,
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
            solve_review
        ])
        .run(tauri::generate_context!())
        .expect("error while running ascent");
}
