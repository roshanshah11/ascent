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

/// One metric of an executed comparison: the measured value, the simulated
/// value the model produced, both error forms, the tolerance decided *before*
/// the result was seen, and the pass/fail that follows from them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparedMetric {
    pub id: String,
    pub unit: String,
    pub measured: f64,
    pub simulated: f64,
    pub abs_error: f64,
    pub rel_error: f64,
    pub tolerance: f64,
    pub pass: bool,
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
    /// Overall result: true iff every metric passed.
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
        let all_pass = self.metrics.iter().all(|metric| metric.pass);
        if self.pass != all_pass {
            return Err("comparison overall pass disagrees with its metrics".into());
        }
        Ok(())
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

#[cfg(test)]
mod evidence_policy_tests {
    use super::*;

    /// A structurally valid `FlightValidated` case *specification* with no
    /// comparison artifact attached. The policy invariant is what decides
    /// whether it may load.
    fn flight_case() -> ValidationCase {
        ValidationCase {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            case_id: "unit-flight".into(),
            title: "Unit flight case".into(),
            intended_use: "Exercise the FlightValidated load-time invariant".into(),
            evidence_level: EvidenceLevel::FlightValidated,
            source_authority: "synthetic test authority".into(),
            fixture: ValidationFixture {
                path: "data/validation/unit/flight.csv".into(),
                sha256: "a".repeat(64),
                license: "test license".into(),
                source_url: "https://example.invalid/flight.csv".into(),
                upstream_revision: "0000000000000000000000000000000000000000".into(),
            },
            input_pedigree: vec!["synthetic".into()],
            validity_domain: vec!["altitude versus time".into()],
            uncertainty: vec!["synthetic, no real uncertainty".into()],
            configuration_mapping: BTreeMap::from([("altitude".into(), "meters".into())]),
            metrics: vec![MetricDefinition {
                id: "apogee_absolute_error_m".into(),
                unit: "meter".into(),
                definition: "absolute simulated-minus-measured apogee".into(),
                max_absolute: 300.0,
            }],
            caveats: vec!["synthetic case".into()],
            known_mismatches: vec!["synthetic case".into()],
            comparison: None,
        }
    }

    /// A self-consistent artifact bound to `case_hash`. `abs`/`rel`/`pass` are
    /// derived exactly the way `ComparisonArtifact::validate` re-derives them so
    /// the artifact is internally honest.
    fn artifact(
        case_hash: String,
        measured: f64,
        simulated: f64,
        tolerance: f64,
    ) -> ComparisonArtifact {
        let abs_error = (simulated - measured).abs();
        let rel_error = if measured.abs() > 0.0 {
            abs_error / measured.abs()
        } else {
            0.0
        };
        let pass = abs_error <= tolerance;
        ComparisonArtifact {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            case_id: "unit-flight".into(),
            case_hash,
            model_version: "test-model-1".into(),
            source_hashes: BTreeMap::from([("fixture".into(), "b".repeat(64))]),
            input_hash: "c".repeat(64),
            metrics: vec![ComparedMetric {
                id: "apogee_absolute_error_m".into(),
                unit: "meter".into(),
                measured,
                simulated,
                abs_error,
                rel_error,
                tolerance,
                pass,
            }],
            pass,
            known_limitations: vec!["altitude channel only".into()],
        }
    }

    #[test]
    fn flight_validated_without_a_comparison_cannot_load() {
        let case = flight_case();
        assert!(case.enforce_evidence_policy().is_err());

        // Structural validation still passes — the bar is the *policy*, applied
        // at load time on top of a well-formed spec.
        assert!(case.validate().is_ok());
        let bytes = case.canonical_bytes().unwrap();
        let err = ValidationCase::from_canonical_bytes(&bytes).unwrap_err();
        assert!(
            err.contains("requires an executed comparison artifact"),
            "{err}"
        );
    }

    #[test]
    fn flight_validated_with_a_passing_bound_comparison_loads() {
        let mut case = flight_case();
        let hash = case.case_hash().unwrap();
        case.comparison = Some(artifact(hash, 3000.0, 3100.0, 300.0));

        case.enforce_evidence_policy().unwrap();
        let bytes = case.canonical_bytes().unwrap();
        let loaded = ValidationCase::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(loaded, case);
        assert_eq!(loaded.evidence_level, EvidenceLevel::FlightValidated);
    }

    #[test]
    fn flight_validated_with_a_failing_comparison_is_refused() {
        let mut case = flight_case();
        let hash = case.case_hash().unwrap();
        // 900 m error against a 300 m tolerance: an honest, failing artifact.
        let failing = artifact(hash, 3000.0, 3900.0, 300.0);
        assert!(!failing.pass);
        assert!(failing.validate().is_ok()); // internally consistent, just failing
        case.comparison = Some(failing);

        let err = case.enforce_evidence_policy().unwrap_err();
        assert!(err.contains("requires a passing comparison"), "{err}");
        let bytes = case.canonical_bytes().unwrap();
        assert!(ValidationCase::from_canonical_bytes(&bytes).is_err());
    }

    #[test]
    fn a_comparison_bound_to_a_different_case_is_refused() {
        let mut case = flight_case();
        let wrong_hash = "d".repeat(64);
        case.comparison = Some(artifact(wrong_hash, 3000.0, 3100.0, 300.0));

        let err = case.enforce_evidence_policy().unwrap_err();
        assert!(err.contains("bound to a different case"), "{err}");
    }

    #[test]
    fn a_comparison_cannot_fake_a_pass_its_numbers_do_not_support() {
        let hash = flight_case().case_hash().unwrap();
        let mut forged = artifact(hash, 3000.0, 3900.0, 300.0);
        // Hand-flip the pass flags without changing the measured/simulated values.
        forged.metrics[0].pass = true;
        forged.pass = true;
        let err = forged.validate().unwrap_err();
        assert!(err.contains("pass flag disagrees"), "{err}");
    }

    #[test]
    fn lower_rungs_load_without_a_comparison() {
        let mut case = flight_case();
        case.evidence_level = EvidenceLevel::FlightDataAvailable;
        case.comparison = None;
        case.enforce_evidence_policy().unwrap();
        let bytes = case.canonical_bytes().unwrap();
        ValidationCase::from_canonical_bytes(&bytes).unwrap();
    }

    #[test]
    fn case_hash_is_spec_only_and_ignores_the_attached_artifact() {
        let mut case = flight_case();
        let spec_hash = case.case_hash().unwrap();
        case.comparison = Some(artifact(spec_hash.clone(), 3000.0, 3100.0, 300.0));
        // Attaching the artifact must not change the case's spec identity.
        assert_eq!(spec_hash, case.case_hash().unwrap());
    }
}
