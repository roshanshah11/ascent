//! Project document layer (v0.2 Step 1): the file the user owns.
//!
//! An `.ascent` project is plain TOML — human-readable, git-diffable (the
//! anti-.ork). Runs embed their full summary + input_hash so a loaded
//! project can prove whether a stored result is stale for its design.
//! Format spec: docs/PROJECT_FORMAT.md.

use std::fs;
use std::path::{Path, PathBuf};

use crate::design::{Design, RunRecord};
use serde::{Deserialize, Serialize};

/// Bump only on breaking schema changes; unknown *fields* are tolerated
/// without a bump (forward compatibility for additive changes).
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    pub name: String,
    pub designs: Vec<Design>,
    pub runs: Vec<RunRecord>,
}

impl Project {
    pub fn new(name: &str) -> Self {
        Project {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            designs: vec![Design::reference()],
            runs: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::run_design;

    fn sample_project() -> Project {
        let mut p = Project::new("regression bird");
        let record = run_design(&Design::reference()).unwrap();
        p.runs.push(record);
        p
    }

    #[test]
    fn roundtrip_is_byte_identical() {
        let p = sample_project();
        let first = to_toml(&p).unwrap();
        let reloaded = from_toml(&first).unwrap();
        let second = to_toml(&reloaded).unwrap();
        assert_eq!(first, second, "TOML roundtrip must be byte-identical");
        // And the physics provenance survives intact.
        assert_eq!(
            reloaded.runs[0].summary.input_hash,
            p.runs[0].summary.input_hash
        );
    }

    #[test]
    fn unknown_fields_are_tolerated() {
        let mut text = to_toml(&Project::new("forward")).unwrap();
        text.insert_str(
            text.find('\n').unwrap() + 1,
            "future_feature_flag = \"from-v0.9\"\n",
        );
        let p = from_toml(&text).expect("additive fields must not break older readers");
        assert_eq!(p.name, "forward");
    }

    #[test]
    fn newer_schema_version_is_rejected_with_clear_message() {
        let text = to_toml(&Project::new("tomorrow")).unwrap();
        let bumped = text.replace("schema_version = 1", "schema_version = 999");
        let err = from_toml(&bumped).unwrap_err();
        assert!(err.contains("999"), "error should name the version: {err}");
        assert!(err.contains("newer"), "error should say why: {err}");
    }

    #[test]
    fn garbage_is_rejected_not_crashed() {
        assert!(from_toml("this is not toml [[[").is_err());
        assert!(from_toml("just_a_key = 1").is_err());
    }

    /// Unique per-test dir (avoids env-var races between test threads).
    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("ascent-autosave-tests")
            .join(format!("{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn crashed_session_leaves_a_recoverable_autosave_that_roundtrips() {
        let dir = scratch_dir("crash");
        let p = sample_project();
        let path = autosave_in(&dir, &p).unwrap();
        assert!(path.exists());

        // "Crash": no clean-exit discard runs. Next launch finds the file.
        let (found_path, recovered) = find_recovery_in(&dir).expect("autosave must be found");
        assert_eq!(found_path, path);
        assert_eq!(to_toml(&recovered).unwrap(), to_toml(&p).unwrap());
        assert_eq!(
            recovered.runs[0].summary.input_hash,
            p.runs[0].summary.input_hash,
            "recovery must preserve run provenance"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clean_save_discards_the_autosave_and_discard_is_idempotent() {
        let dir = scratch_dir("clean");
        autosave_in(&dir, &Project::new("wip")).unwrap();
        assert!(find_recovery_in(&dir).is_some());

        discard_recovery_in(&dir).unwrap();
        assert!(find_recovery_in(&dir).is_none(), "clean save must clear recovery");
        discard_recovery_in(&dir).expect("discarding nothing is not an error");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn interrupted_write_never_corrupts_the_previous_autosave() {
        let dir = scratch_dir("atomic");
        let good = sample_project();
        autosave_in(&dir, &good).unwrap();

        // Simulate a crash mid-write: a half-written temp file next to the
        // real autosave. Recovery must ignore it and read the good file.
        fs::write(dir.join("recovery.ascent.json.tmp"), "{\"schema_version\":").unwrap();
        let (_, recovered) = find_recovery_in(&dir).expect("good autosave still present");
        assert_eq!(to_toml(&recovered).unwrap(), to_toml(&good).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_autosave_is_treated_as_absent_not_fatal() {
        let dir = scratch_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(RECOVERY_FILE), "not json {{{").unwrap();
        assert!(find_recovery_in(&dir).is_none(), "corrupt recovery must not crash startup");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn autosave_of_a_hundred_run_project_is_fast() {
        let dir = scratch_dir("perf");
        let mut p = sample_project();
        let record = p.runs[0].clone();
        p.runs = std::iter::repeat_with(|| record.clone()).take(100).collect();

        let start = std::time::Instant::now();
        autosave_in(&dir, &p).unwrap();
        let elapsed = start.elapsed();
        // Acceptance: < 100 ms release; debug builds get slack but a slow
        // serializer still fails loudly.
        assert!(
            elapsed.as_millis() < if cfg!(debug_assertions) { 1000 } else { 100 },
            "autosave of 100-run project took {elapsed:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_field_edit_is_a_one_line_diff() {
        let base = Project::new("diff test");
        let mut edited = base.clone();
        edited.designs[0].cd = 0.65;
        let a = to_toml(&base).unwrap();
        let b = to_toml(&edited).unwrap();
        let differing: Vec<(&str, &str)> = a
            .lines()
            .zip(b.lines())
            .filter(|(x, y)| x != y)
            .collect();
        assert_eq!(a.lines().count(), b.lines().count());
        assert_eq!(differing.len(), 1, "one edit must be one changed line");
        assert!(differing[0].1.contains("0.65"));
    }
}
