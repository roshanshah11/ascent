//! Tauri backend: coarse IPC only. The UI sends a whole Design and gets a
//! whole RunRecord back — no fine-grained property calls across the bridge.
//! All physics stays in ascent-domain / ascent-sim; this crate only maps
//! DTOs and downsamples the trajectory for playback.

mod design;

pub use design::{run_design, Design, MotorInfo, RunRecord};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            reference_design,
            list_motors,
            run_simulation
        ])
        .run(tauri::generate_context!())
        .expect("error while running ascent");
}
