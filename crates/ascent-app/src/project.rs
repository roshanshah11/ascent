//! Project document layer (v0.2 Step 1): the file the user owns.
//!
//! An `.ascent` project is plain TOML — human-readable, git-diffable (the
//! anti-.ork). Runs embed their full summary + input_hash so a loaded
//! project can prove whether a stored result is stale for its design.
//! Format spec: docs/PROJECT_FORMAT.md.

use std::fs;
use std::path::{Path, PathBuf};

use crate::design::{Design, RunRecord};
use crate::study::Study;
use ascent_domain::evidence::TelemetryBundle;
use ascent_review::alignment::AlignmentArtifact;
use ascent_review::reconciliation::ReconciliationResult;
use serde::{Deserialize, Serialize};

/// Bump only on breaking schema changes; unknown *fields* are tolerated
/// without a bump (forward compatibility for additive changes).
///
/// v2 (v0.3 Step 4): adds `studies`. A v1 file is a valid v2 file with
/// zero studies — `#[serde(default)]` makes the migration a no-op, so
/// older projects load with nothing lost.
pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    pub name: String,
    pub designs: Vec<Design>,
    pub runs: Vec<RunRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub studies: Vec<Study>,
    /// The vehicle model tree (additive, v0.3 Step 8). Absent in older
    /// files; readers fall back to the reference vehicle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<ascent_domain::vehicle::Vehicle>,
    /// Immutable telemetry and selected review artifacts are additive v0.6
    /// fields. Raw evidence remains embedded, so migrated projects reopen
    /// without external files or network access.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telemetry: Vec<TelemetryBundle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<AlignmentArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<ReconciliationResult>,
}

impl Project {
    pub fn new(name: &str) -> Self {
        Project {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            designs: vec![Design::reference()],
            runs: Vec::new(),
            studies: Vec::new(),
            vehicle: None,
            telemetry: Vec::new(),
            alignment: None,
            reconciliation: None,
        }
    }
}

pub fn to_toml(project: &Project) -> Result<String, String> {
    toml::to_string_pretty(project).map_err(|e| format!("failed to serialize project: {e}"))
}

pub fn from_toml(text: &str) -> Result<Project, String> {
    // Peek at schema_version before full deserialization so a future-format
    // file fails with a version message, not a field-shape error.
    let value: toml::Value =
        toml::from_str(text).map_err(|e| format!("not a valid project file: {e}"))?;
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_integer())
        .ok_or("not a valid project file: missing schema_version")?;
    if version as u32 > SCHEMA_VERSION {
        return Err(format!(
            "project was saved by a newer Ascent (schema_version {version}, this build reads up to {SCHEMA_VERSION}) — update Ascent to open it"
        ));
    }
    value
        .try_into()
        .map_err(|e| format!("could not read project: {e}"))
}

// ---- Autosave + crash recovery (v0.2 Step 8) ----------------------------
//
// MS-Office pattern: autosaves live in their own location, never the user's
// file. A clean save/exit discards the autosave; if one survives to the next
// launch, the app crashed and the user is offered a restore.

/// Autosave is app-internal crash-recovery state, not the user's document,
/// so it uses JSON: serializing a 100-run project in TOML costs ~340 ms
/// (pretty tables for every playback sample) vs single-digit ms in JSON —
/// and the 100 ms autosave budget is a hard acceptance bound. The user's
/// `.ascent` file stays TOML.
const RECOVERY_FILE: &str = "recovery.ascent.json";

/// Where autosaves live: `~/.ascent/autosave` (HOME on unix, USERPROFILE on
/// Windows), overridable with ASCENT_AUTOSAVE_DIR for tests and portability.
fn autosave_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("ASCENT_AUTOSAVE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".ascent").join("autosave")
}

/// Atomic write into `dir`: serialize to a temp file, then rename. A crash
/// mid-write leaves the previous autosave intact; rename on the same
/// filesystem is atomic, so the recovery file is always complete TOML.
pub fn autosave_in(dir: &Path, project: &Project) -> Result<PathBuf, String> {
    let text = serde_json::to_string(project).map_err(|e| format!("autosave serialize: {e}"))?;
    fs::create_dir_all(dir).map_err(|e| format!("cannot create autosave dir: {e}"))?;
    let tmp = dir.join(format!("{RECOVERY_FILE}.tmp"));
    let dest = dir.join(RECOVERY_FILE);
    fs::write(&tmp, text).map_err(|e| format!("autosave write failed: {e}"))?;
    fs::rename(&tmp, &dest).map_err(|e| format!("autosave rename failed: {e}"))?;
    Ok(dest)
}

/// A surviving, parseable autosave means the last session did not exit
/// cleanly. A corrupt one is treated as absent — recovery must never take
/// down startup.
pub fn find_recovery_in(dir: &Path) -> Option<(PathBuf, Project)> {
    let path = dir.join(RECOVERY_FILE);
    let text = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok().map(|p| (path, p))
}

/// Remove the autosave (clean save, clean exit, or user said discard).
/// Missing file is fine — discard is idempotent.
pub fn discard_recovery_in(dir: &Path) -> Result<(), String> {
    let path = dir.join(RECOVERY_FILE);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("could not discard autosave: {e}")),
    }
}

pub fn autosave(project: &Project) -> Result<PathBuf, String> {
    autosave_in(&autosave_dir(), project)
}

pub fn find_recovery() -> Option<(PathBuf, Project)> {
    find_recovery_in(&autosave_dir())
}

pub fn discard_recovery() -> Result<(), String> {
    discard_recovery_in(&autosave_dir())
}
