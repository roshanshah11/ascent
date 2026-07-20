//! Checked-in validation cases with frozen pedigree and acceptance policy.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use ascent_domain::evidence::{EvidenceLevel, EVIDENCE_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationFixture {
    pub path: String,
    pub sha256: String,
    pub license: String,
    pub source_url: String,
    pub upstream_revision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub id: String,
    pub unit: String,
    pub definition: String,
    pub max_absolute: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationCase {
    pub schema_version: u16,
    pub case_id: String,
    pub title: String,
    pub intended_use: String,
    pub evidence_level: EvidenceLevel,
    pub source_authority: String,
    pub fixture: ValidationFixture,
    pub input_pedigree: Vec<String>,
    pub validity_domain: Vec<String>,
    pub uncertainty: Vec<String>,
    pub configuration_mapping: BTreeMap<String, String>,
    pub metrics: Vec<MetricDefinition>,
    pub caveats: Vec<String>,
    pub known_mismatches: Vec<String>,
}

impl ValidationCase {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != EVIDENCE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported validation schema version {}",
                self.schema_version
            ));
        }
        for (name, value) in [
            ("case id", self.case_id.as_str()),
            ("title", self.title.as_str()),
            ("intended use", self.intended_use.as_str()),
            ("source authority", self.source_authority.as_str()),
            ("fixture path", self.fixture.path.as_str()),
            ("fixture license", self.fixture.license.as_str()),
            ("fixture source URL", self.fixture.source_url.as_str()),
            (
                "fixture upstream revision",
                self.fixture.upstream_revision.as_str(),
            ),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{name} must not be empty"));
            }
        }
        validate_hash(&self.fixture.sha256)?;
        if self.validity_domain.is_empty()
            || self.uncertainty.is_empty()
            || self.configuration_mapping.is_empty()
            || self.metrics.is_empty()
        {
            return Err(
                "validation case must declare validity, uncertainty, mapping, and metrics".into(),
            );
        }
        for metric in &self.metrics {
            if metric.id.trim().is_empty()
                || metric.definition.trim().is_empty()
                || metric.unit.trim().is_empty()
                || !metric.max_absolute.is_finite()
                || metric.max_absolute < 0.0
            {
                return Err(format!("metric {} is incomplete or invalid", metric.id));
            }
        }
        let path = Path::new(&self.fixture.path);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("fixture path must be a normalized repository-relative path".into());
        }
        Ok(())
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, String> {
        let payload = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        let case: Self = serde_json::from_slice(payload)
            .map_err(|error| format!("validation case JSON is invalid: {error}"))?;
        case.validate()?;
        if case.canonical_bytes()? != payload {
            return Err("validation case is not canonical JSON".into());
        }
        Ok(case)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| error.to_string())
    }

    pub fn case_hash(&self) -> Result<String, String> {
        Ok(hash_bytes(&self.canonical_bytes()?))
    }

    pub fn verify_fixture(&self, repository_root: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = std::fs::read(repository_root.join(&self.fixture.path))
            .map_err(|error| format!("fixture {} is unavailable: {error}", self.fixture.path))?;
        self.verify_fixture_bytes(&bytes)
    }

    pub fn verify_fixture_bytes(&self, bytes: &[u8]) -> Result<(), String> {
        let actual = hash_bytes(bytes);
        if actual != self.fixture.sha256 {
            return Err(format!(
                "fixture hash mismatch for {}: expected {}, got {}",
                self.fixture.path, self.fixture.sha256, actual
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricOutcome {
    pub id: String,
    pub observed: f64,
    pub threshold: f64,
    pub pass: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub schema_version: u16,
    pub case_id: String,
    pub case_hash: String,
    pub evidence_level: EvidenceLevel,
    pub pass: bool,
    pub metrics: Vec<MetricOutcome>,
    pub caveats: Vec<String>,
    pub known_mismatches: Vec<String>,
}

impl ComparisonResult {
    pub fn evaluate(
        case: &ValidationCase,
        observed: BTreeMap<String, f64>,
    ) -> Result<Self, String> {
        case.validate()?;
        let mut metrics = Vec::with_capacity(case.metrics.len());
        for definition in &case.metrics {
            let value = *observed
                .get(&definition.id)
                .ok_or_else(|| format!("missing observed metric {}", definition.id))?;
            if !value.is_finite() {
                return Err(format!("observed metric {} must be finite", definition.id));
            }
            metrics.push(MetricOutcome {
                id: definition.id.clone(),
                observed: value,
                threshold: definition.max_absolute,
                pass: value.abs() <= definition.max_absolute,
            });
        }
        if observed.len() != metrics.len() {
            return Err("observed metrics include undeclared values".into());
        }
        Ok(Self {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            case_id: case.case_id.clone(),
            case_hash: case.case_hash()?,
            evidence_level: case.evidence_level.clone(),
            pass: metrics.iter().all(|metric| metric.pass),
            metrics,
            caveats: case.caveats.clone(),
            known_mismatches: case.known_mismatches.clone(),
        })
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|error| error.to_string())
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_hash(hash: &str) -> Result<(), String> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid fixture SHA-256 {hash}"));
    }
    Ok(())
}
