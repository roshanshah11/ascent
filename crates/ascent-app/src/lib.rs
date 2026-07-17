//! Tauri backend: coarse IPC only. The UI sends a whole Design and gets a
//! whole RunRecord back — no fine-grained property calls across the bridge.
//! All physics stays in ascent-domain / ascent-sim; this crate only maps
//! DTOs and downsamples the trajectory for playback.

mod credibility;
mod design;
mod dispersion_ipc;
mod evidence;
mod project;
mod review_ipc;

pub use credibility::{Factor, QuantityFlag, Regime, Scorecard};
pub use design::{run_design, Design, ImportedMotor, MotorInfo, RunRecord};
pub use dispersion_ipc::DispersionRequest;
pub use project::{from_toml, to_toml, Project};
pub use evidence::{evidence_for, EvidenceReport};
pub use review_ipc::{repair as review_repair, report as review_report, ReviewReport};

#[tauri::command]
fn reference_design() -> Design {
    Design::reference()
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
fn import_motor_file(contents: String) -> Result<ImportedMotor, String> {
    design::import_motor_file(&contents)
}

#[tauri::command]
fn run_evidence(design: Design) -> Result<EvidenceReport, String> {
    evidence_for(&design)
}

#[tauri::command]
fn run_dispersion(
    design: Design,
    request: DispersionRequest,
) -> Result<ascent_sim::DispersionSummary, String> {
    dispersion_ipc::run(&design, &request)
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
        .invoke_handler(tauri::generate_handler![
            reference_design,
            list_motors,
            run_simulation,
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
