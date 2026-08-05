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
    /// Executed comparison of the model's numerical outputs against this case's
    /// measured data. Absent for every rung below `FlightValidated`; **required**
    /// (present, internally valid, passing, and bound to this case) for a case to
    /// load at `FlightValidated`. This is the artifact the `FlightValidated`
    /// label attests to — see [`ComparisonArtifact`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<ComparisonArtifact>,
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
        // A comparison artifact, if attached at any rung, must itself be
        // well-formed. The *policy* that ties the artifact to the
        // `FlightValidated` label lives in `enforce_evidence_policy`, kept
        // separate so `case_hash` can hash the spec without recursion.
        if let Some(comparison) = &self.comparison {
            comparison.validate()?;
        }
        Ok(())
    }

    /// The load-time evidence-label invariant. A case labeled `FlightValidated`
    /// cannot load unless it carries an executed comparison artifact that is
    /// internally valid, bound to this exact case definition, and passing. Every
    /// other rung is accepted as-is. This is what makes the `FlightValidated`
    /// badge impossible to assert without evidence.
    pub fn enforce_evidence_policy(&self) -> Result<(), String> {
        if self.evidence_level == EvidenceLevel::FlightValidated {
            let comparison = self.comparison.as_ref().ok_or_else(|| {
                "FlightValidated case requires an executed comparison artifact".to_string()
            })?;
            comparison.validate()?;
            if comparison.case_id != self.case_id {
                return Err(format!(
                    "comparison artifact case_id {} does not match case {}",
                    comparison.case_id, self.case_id
                ));
            }
            if comparison.case_hash != self.case_hash()? {
                return Err("comparison artifact is bound to a different case definition".into());
            }
            if !comparison.pass {
                return Err("FlightValidated requires a passing comparison; a failed \
                            comparison must be published at a lower evidence level"
                    .into());
            }
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
        case.enforce_evidence_policy()?;
        Ok(case)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| error.to_string())
    }

    /// The identity of the case *specification* (everything except the attached
    /// comparison artifact). Stable whether or not an artifact is bound, so a
    /// comparison can reference the exact spec it was run against without a
    /// hash-of-itself cycle.
    pub fn case_hash(&self) -> Result<String, String> {
        let mut spec = self.clone();
        spec.comparison = None;
        Ok(hash_bytes(&spec.canonical_bytes()?))
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

/// How a compared metric participates in the overall comparison result.
///
/// The distinction is deliberate: a `Primary` metric is a *flight-validation*
/// metric whose pass/fail gates the overall comparison, while an
/// `InputConsistency` metric is a self-check on the inputs (e.g. simulated
/// burnout vs. the imported motor's burn time) that is verified and reported
/// but **never** counted toward the flight-validation pass/fail. Serialized
/// artifacts written before this field existed deserialize as `Primary`, which
/// preserves the original "every metric gates the result" behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    /// A flight-validation metric: contributes to the overall comparison pass.
    #[default]
    Primary,
    /// An input-consistency check: verified and reported, but excluded from the
    /// overall flight-validation pass/fail.
    InputConsistency,
}

/// One metric of an executed comparison: the measured value, the simulated
/// value the model produced, both error forms, the tolerance decided *before*
/// the result was seen, and the pass/fail that follows from them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparedMetric {
    pub id: String,
    pub unit: String,
    /// Whether this metric gates the overall result (`Primary`) or is an
    /// input-consistency self-check (`InputConsistency`). Defaults to `Primary`.
    #[serde(default)]
    pub kind: MetricKind,
    pub measured: f64,
    pub simulated: f64,
    pub abs_error: f64,
    pub rel_error: f64,
    pub tolerance: f64,
    pub pass: bool,
}

impl ComparedMetric {
    /// The only honest constructor: derive `abs_error`, `rel_error`, and `pass`
    /// from the measured/simulated pair and the pre-frozen `tolerance`, using
    /// exactly the expressions [`ComparisonArtifact::validate`] re-checks. A
    /// metric built here therefore always survives validation, and a failing
    /// comparison cannot be turned into a pass without changing the numbers that
    /// produced it. `rel_error` is defined as 0 when `measured` is 0 (the
    /// convention `validate` enforces), which is what lets a normalized-residual
    /// metric use a 0 baseline.
    pub fn derive(
        id: impl Into<String>,
        unit: impl Into<String>,
        measured: f64,
        simulated: f64,
        tolerance: f64,
    ) -> Self {
        let abs_error = (simulated - measured).abs();
        let rel_error = if measured.abs() > 0.0 {
            abs_error / measured.abs()
        } else {
            0.0
        };
        Self {
            id: id.into(),
            unit: unit.into(),
            kind: MetricKind::Primary,
            measured,
            simulated,
            abs_error,
            rel_error,
            tolerance,
            pass: abs_error <= tolerance,
        }
    }

    /// Reclassify a derived metric as an input-consistency check, so it is
    /// verified and reported but excluded from the overall flight-validation
    /// pass/fail. The numbers are untouched — only its role changes.
    pub fn as_input_consistency(mut self) -> Self {
        self.kind = MetricKind::InputConsistency;
        self
    }
}

/// Machine-readable proof that the model was executed and compared against
/// measured flight data. This is the artifact the `FlightValidated` evidence
/// level attests to; a case cannot wear that label without one (see
/// [`ValidationCase::enforce_evidence_policy`]).
///
/// `validate` re-derives every pass/fail and error from the stored measured and
/// simulated values, so an artifact cannot claim a pass its own numbers do not
/// support — the fields cannot be quietly hand-forged to a green result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonArtifact {
    pub schema_version: u16,
    /// The case this comparison was run against.
    pub case_id: String,
    /// Spec hash of that case ([`ValidationCase::case_hash`]) — pins the exact
    /// inputs/mapping the comparison used.
    pub case_hash: String,
    /// Version identity of the model that produced the simulated values.
    pub model_version: String,
    /// Named SHA-256 hashes of every source the comparison consumed (measured
    /// fixture, model source, config), so the artifact is reproducible.
    pub source_hashes: BTreeMap<String, String>,
    /// SHA-256 of the canonical simulation input that produced the outputs.
    pub input_hash: String,
    pub metrics: Vec<ComparedMetric>,
    /// Overall flight-validation result: true iff there is at least one
    /// `Primary` metric and every `Primary` metric passed. `InputConsistency`
    /// metrics are excluded — they are self-checks, not flight-validation
    /// evidence, and cannot alone force a pass or fail.
    pub pass: bool,
    pub known_limitations: Vec<String>,
}

impl ComparisonArtifact {
    /// Structural + arithmetic integrity. Rejects an artifact whose stored
    /// pass/fail or errors are inconsistent with its measured/simulated values,
    /// so the label cannot rest on fabricated numbers.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != EVIDENCE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported comparison schema version {}",
                self.schema_version
            ));
        }
        for (name, value) in [
            ("comparison case id", self.case_id.as_str()),
            ("comparison model version", self.model_version.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{name} must not be empty"));
            }
        }
        validate_hash(&self.case_hash)?;
        validate_hash(&self.input_hash)?;
        if self.source_hashes.is_empty() {
            return Err("comparison must record at least one source hash".into());
        }
        for (name, hash) in &self.source_hashes {
            if name.trim().is_empty() {
                return Err("comparison source name must not be empty".into());
            }
            validate_hash(hash)?;
        }
        if self.metrics.is_empty() {
            return Err("comparison must compare at least one metric".into());
        }
        for metric in &self.metrics {
            if metric.id.trim().is_empty() || metric.unit.trim().is_empty() {
                return Err(format!(
                    "compared metric {} is missing id or unit",
                    metric.id
                ));
            }
            for value in [
                metric.measured,
                metric.simulated,
                metric.abs_error,
                metric.rel_error,
                metric.tolerance,
            ] {
                if !value.is_finite() {
                    return Err(format!(
                        "compared metric {} has a non-finite value",
                        metric.id
                    ));
                }
            }
            if metric.tolerance < 0.0 {
                return Err(format!(
                    "compared metric {} has a negative tolerance",
                    metric.id
                ));
            }
            let expected_abs = (metric.simulated - metric.measured).abs();
            if (metric.abs_error - expected_abs).abs() > 1e-9 * (1.0 + metric.measured.abs()) {
                return Err(format!(
                    "compared metric {} abs_error disagrees with measured/simulated",
                    metric.id
                ));
            }
            let denom = metric.measured.abs();
            let expected_rel = if denom > 0.0 {
                expected_abs / denom
            } else {
                0.0
            };
            if (metric.rel_error - expected_rel).abs() > 1e-9 * (1.0 + expected_rel) {
                return Err(format!(
                    "compared metric {} rel_error disagrees with measured/simulated",
                    metric.id
                ));
            }
            if metric.pass != (metric.abs_error <= metric.tolerance) {
                return Err(format!(
                    "compared metric {} pass flag disagrees with its tolerance",
                    metric.id
                ));
            }
        }
        // The overall result is derived from the *primary* (flight-validation)
        // metrics only. An input-consistency check never gates the pass, and a
        // comparison with no primary metric at all cannot claim a pass.
        let mut primary = self
            .metrics
            .iter()
            .filter(|metric| metric.kind == MetricKind::Primary)
            .peekable();
        let expected_pass = primary.peek().is_some() && primary.all(|metric| metric.pass);
        if self.pass != expected_pass {
            return Err("comparison overall pass disagrees with its primary metrics".into());
        }
        Ok(())
    }

    /// Assemble an artifact from already-derived metrics. The overall `pass` is
    /// computed from the metrics — never passed in — and the result is
    /// validated before it is returned, so this path cannot emit an artifact
    /// whose stored numbers disagree with themselves or whose `pass` was forged.
    #[allow(clippy::too_many_arguments)]
    pub fn assemble(
        case_id: impl Into<String>,
        case_hash: impl Into<String>,
        model_version: impl Into<String>,
        source_hashes: BTreeMap<String, String>,
        input_hash: impl Into<String>,
        metrics: Vec<ComparedMetric>,
        known_limitations: Vec<String>,
    ) -> Result<Self, String> {
        let mut primary = metrics
            .iter()
            .filter(|metric| metric.kind == MetricKind::Primary)
            .peekable();
        let pass = primary.peek().is_some() && primary.all(|metric| metric.pass);
        let artifact = Self {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            case_id: case_id.into(),
            case_hash: case_hash.into(),
            model_version: model_version.into(),
            source_hashes,
            input_hash: input_hash.into(),
            metrics,
            pass,
            known_limitations,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| error.to_string())
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
