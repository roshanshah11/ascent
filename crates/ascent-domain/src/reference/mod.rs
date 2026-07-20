//! Evidence-graded reference missions.
//!
//! Each reference mission pairs a [`Vehicle`](crate::vehicle::Vehicle) with a
//! set of [`Source`] documents and [`EvidenceBinding`]s that grade every field
//! used to construct it. The vertical slice (Step 1 of the Unity visual-engine
//! plan) ships the NASA Black Brant IX reference mission; the deterministic
//! two-stage 6-DOF trace that consumes it lives in `ascent-sim`
//! (`ascent_sim::reference_trace`), because `ascent-domain` must not depend on
//! `ascent-sim`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path};

use crate::motor::Motor;
use crate::vehicle::Vehicle;

pub mod black_brant_ix;

pub use black_brant_ix::{black_brant_ix_reference, BLACK_BRANT_IX_MISSION_ID};

/// Loading or validating a checked-in reference mission failed.
#[derive(Debug, thiserror::Error)]
pub enum ReferenceError {
    #[error("invalid {artifact}: {source}")]
    Parse {
        artifact: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}")]
    Validation(String),
}

/// Identifier for a reference mission (e.g. `nasa.black-brant-ix.reference`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionId(pub String);

impl MissionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for MissionId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for MissionId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// How strongly a source supports a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    /// Directly measured / spec'd by the cited source.
    Authoritative,
    /// Scaled, rounded, or reconstructed from the source — not a 1:1 figure.
    Approximate,
    /// Computed from other inputs by a named formula.
    Derived,
}

impl Grade {
    pub fn as_str(&self) -> &'static str {
        match self {
            Grade::Authoritative => "authoritative",
            Grade::Approximate => "approximate",
            Grade::Derived => "derived",
        }
    }
}

impl fmt::Display for Grade {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A single documented source for reference-mission evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// Stable identifier used by [`EvidenceBinding::source_id`].
    pub id: String,
    /// Human-readable title of the source document.
    pub title: String,
    /// Canonical URL (or locator) of the source.
    pub url: String,
    /// Date the source was retrieved / recorded (ISO-8601).
    pub retrieved: String,
    /// Rights status of the source (e.g. `public-domain`).
    pub rights: String,
    /// SHA-256 hex digest of the stored source artifact.
    pub sha256: String,
    /// SHA-256 of the original upstream artifact when the checked-in artifact
    /// is a redistributable excerpt rather than the complete source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_sha256: Option<String>,
    /// Path relative to the repository root where the artifact is stored.
    pub stored_path: String,
    /// Grade of the source's support for the fields it backs.
    pub grade: Grade,
}

/// A graded link between a mission field and the source that supports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBinding {
    pub source_id: String,
    pub field_path: String,
    pub grade: Grade,
    /// For `derived` bindings: the formula used to compute the field.
    #[serde(default)]
    pub formula: Option<String>,
    /// For `derived` bindings: the named inputs to the formula.
    #[serde(default)]
    pub inputs: Vec<String>,
    /// For `approximate` bindings: a statement of the approximation's impact.
    #[serde(default)]
    pub impact: Option<String>,
}

/// A caveat / scope note attached to a reference mission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionCaveat(pub String);

impl std::ops::Deref for MissionCaveat {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MissionCaveat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// An evidence-graded reference mission: geometry, motors, sources, and the
/// bindings that grade every field used to build it.
#[derive(Debug, Clone)]
pub struct ReferenceMission {
    pub mission_id: MissionId,
    pub vehicle: Vehicle,
    pub evidence: Vec<EvidenceBinding>,
    pub sources: Vec<Source>,
    pub motors: Vec<Motor>,
    pub caveat: MissionCaveat,
}

impl ReferenceMission {
    /// Validate that every evidence binding references a known source, every
    /// source declares its rights, and (when a repository root is available)
    /// every stored artifact's SHA-256 matches its recorded digest.
    pub fn verify(&self, repo_root: Option<&str>) -> Result<(), String> {
        self.vehicle.validate()?;
        if self.vehicle.stages().len() != 2 || self.motors.len() != 2 {
            return Err(
                "Black Brant IX reference must contain exactly two stages and motors".into(),
            );
        }
        for motor in &self.motors {
            motor.validate().map_err(|error| error.to_string())?;
        }
        let known_sources: Vec<&str> = self.sources.iter().map(|s| s.id.as_str()).collect();
        let mut bound_fields = BTreeSet::new();
        for binding in &self.evidence {
            if !known_sources.contains(&binding.source_id.as_str()) {
                return Err(format!(
                    "binding {} -> unknown source {}",
                    binding.field_path, binding.source_id
                ));
            }
            if !bound_fields.insert(binding.field_path.as_str()) {
                return Err(format!(
                    "duplicate evidence binding for {}",
                    binding.field_path
                ));
            }
            if binding.grade == Grade::Derived {
                if binding
                    .formula
                    .as_ref()
                    .is_none_or(|formula| formula.is_empty())
                {
                    return Err(format!(
                        "derived binding {} needs a formula",
                        binding.field_path
                    ));
                }
                if binding.inputs.is_empty() {
                    return Err(format!(
                        "derived binding {} needs named inputs",
                        binding.field_path
                    ));
                }
            }
            if binding.grade == Grade::Approximate
                && binding
                    .impact
                    .as_ref()
                    .is_none_or(|impact| impact.is_empty())
            {
                return Err(format!(
                    "approximate binding {} needs an impact statement",
                    binding.field_path
                ));
            }
        }
        for source in &self.sources {
            if source.rights.trim().is_empty() {
                return Err(format!(
                    "source {} has no rights statement: {}",
                    source.id, source.rights
                ));
            }
            if source.sha256.len() != 64
                || !source
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err(format!("source {} has invalid sha256", source.id));
            }
            if source.original_sha256.as_ref().is_some_and(|digest| {
                digest.len() != 64
                    || !digest
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            }) {
                return Err(format!("source {} has invalid original sha256", source.id));
            }
            let stored = Path::new(&source.stored_path);
            if stored.is_absolute()
                || stored
                    .components()
                    .any(|part| matches!(part, Component::ParentDir))
            {
                return Err(format!("source {} has unsafe stored path", source.id));
            }
            if let Some(root) = repo_root {
                let full = Path::new(root).join(stored);
                let bytes = std::fs::read(&full).map_err(|error| {
                    format!(
                        "source {} not readable at {}: {error}",
                        source.id,
                        full.display()
                    )
                })?;
                if bytes.is_empty() {
                    return Err(format!("source {} artifact is empty", source.id));
                }
                let actual = format!("{:x}", Sha256::digest(bytes));
                if actual != source.sha256 {
                    return Err(format!(
                        "source {} hash mismatch: {} != {}",
                        source.id, actual, source.sha256
                    ));
                }
            }
        }
        Ok(())
    }

    /// Require the mission-specific evidence categories that its loader uses
    /// to construct engineering inputs. `verify` already rejects duplicates;
    /// this closes the complementary missing-binding case.
    pub fn require_evidence_fields(&self, required: &[&str]) -> Result<(), String> {
        for field in required {
            if !self
                .evidence
                .iter()
                .any(|binding| binding.field_path == *field)
            {
                return Err(format!("missing evidence binding for {field}"));
            }
        }
        Ok(())
    }
}
