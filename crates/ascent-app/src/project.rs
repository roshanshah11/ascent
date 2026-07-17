//! Project document layer (v0.2 Step 1): the file the user owns.
//!
//! An `.ascent` project is plain TOML — human-readable, git-diffable (the
//! anti-.ork). Runs embed their full summary + input_hash so a loaded
//! project can prove whether a stored result is stale for its design.
//! Format spec: docs/PROJECT_FORMAT.md.

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
