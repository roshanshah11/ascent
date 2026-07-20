use ascent_domain::evidence::{ClockDomain, EVIDENCE_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentMethod {
    Manual,
    EventCorrelation,
    ConstrainedOffsetDrift,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentObservation {
    pub source_s: f64,
    pub target_s: f64,
    pub weight: f64,
    pub evidence_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClockDiscontinuity {
    pub source_time_s: f64,
    pub delta_s: f64,
    pub evidence_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentArtifact {
    pub schema_version: u16,
    pub source_domain: ClockDomain,
    pub target_domain: ClockDomain,
    pub method: AlignmentMethod,
    pub algorithm: String,
    pub algorithm_version: String,
    /// `target = source * scale + offset_s`; source samples remain untouched.
    pub offset_s: f64,
    pub scale: f64,
    pub overlap_source_s: [f64; 2],
    pub objective: String,
    pub objective_value: f64,
    pub confidence: f64,
    pub discontinuities: Vec<ClockDiscontinuity>,
    pub evidence_hashes: Vec<String>,
}

impl AlignmentArtifact {
    #[allow(clippy::too_many_arguments)]
    pub fn manual(
        source_domain: ClockDomain,
        target_domain: ClockDomain,
        offset_s: f64,
        scale: f64,
        overlap_source_s: [f64; 2],
        objective: impl Into<String>,
        evidence_hashes: Vec<String>,
    ) -> Result<Self, String> {
        let artifact = Self {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            source_domain,
            target_domain,
            method: AlignmentMethod::Manual,
            algorithm: "operator-declared-affine-relationship".into(),
            algorithm_version: "1".into(),
            offset_s,
            scale,
            overlap_source_s,
            objective: objective.into(),
            objective_value: 0.0,
            confidence: 1.0,
            discontinuities: vec![],
            evidence_hashes,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn from_event_pairs(
        source_domain: ClockDomain,
        target_domain: ClockDomain,
        observations: &[AlignmentObservation],
    ) -> Result<Self, String> {
        let mut artifact = Self::fit(source_domain, target_domain, observations, None)?;
        artifact.method = AlignmentMethod::EventCorrelation;
        artifact.algorithm = "weighted-event-affine-correlation".into();
        Ok(artifact)
    }

    pub fn fit_constrained(
        source_domain: ClockDomain,
        target_domain: ClockDomain,
        observations: &[AlignmentObservation],
        max_drift_ppm: f64,
    ) -> Result<Self, String> {
        if !max_drift_ppm.is_finite() || max_drift_ppm < 0.0 {
            return Err("maximum drift must be finite and non-negative".into());
        }
        Self::fit(
            source_domain,
            target_domain,
            observations,
            Some(max_drift_ppm),
        )
    }

    fn fit(
        source_domain: ClockDomain,
        target_domain: ClockDomain,
        observations: &[AlignmentObservation],
        max_drift_ppm: Option<f64>,
    ) -> Result<Self, String> {
        if observations.len() < 2 {
            return Err("alignment needs at least two independent observations".into());
        }
        for observation in observations {
            if !observation.source_s.is_finite()
                || !observation.target_s.is_finite()
                || !observation.weight.is_finite()
                || observation.weight <= 0.0
            {
                return Err(
                    "alignment observations and weights must be finite and positive".into(),
                );
            }
            validate_hash(&observation.evidence_hash)?;
        }
        let total_weight = observations.iter().map(|item| item.weight).sum::<f64>();
        let mean_source = observations
            .iter()
            .map(|item| item.source_s * item.weight)
            .sum::<f64>()
            / total_weight;
        let mean_target = observations
            .iter()
            .map(|item| item.target_s * item.weight)
            .sum::<f64>()
            / total_weight;
        let variance = observations
            .iter()
            .map(|item| item.weight * (item.source_s - mean_source).powi(2))
            .sum::<f64>();
        if variance <= f64::EPSILON {
            return Err("alignment observations have no source-time overlap span".into());
        }
        let covariance = observations
            .iter()
            .map(|item| item.weight * (item.source_s - mean_source) * (item.target_s - mean_target))
            .sum::<f64>();
        let scale = covariance / variance;
        let offset_s = mean_target - scale * mean_source;
        let drift_ppm = (scale - 1.0).abs() * 1_000_000.0;
        if max_drift_ppm.is_some_and(|maximum| drift_ppm > maximum) {
            return Err(format!(
                "estimated drift {drift_ppm:.3} ppm exceeds constraint"
            ));
        }
        let squared_error = observations
            .iter()
            .map(|item| {
                let residual = item.target_s - (item.source_s * scale + offset_s);
                item.weight * residual * residual
            })
            .sum::<f64>();
        let target_variance = observations
            .iter()
            .map(|item| item.weight * (item.target_s - mean_target).powi(2))
            .sum::<f64>();
        let confidence = if target_variance <= f64::EPSILON {
            if squared_error <= f64::EPSILON {
                1.0
            } else {
                0.0
            }
        } else {
            (1.0 - squared_error / target_variance).clamp(0.0, 1.0)
        };
        let overlap_source_s = [
            observations
                .iter()
                .map(|item| item.source_s)
                .min_by(f64::total_cmp)
                .unwrap(),
            observations
                .iter()
                .map(|item| item.source_s)
                .max_by(f64::total_cmp)
                .unwrap(),
        ];
        let mut evidence_hashes = observations
            .iter()
            .map(|item| item.evidence_hash.clone())
            .collect::<Vec<_>>();
        evidence_hashes.sort();
        evidence_hashes.dedup();
        let artifact = Self {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            source_domain,
            target_domain,
            method: if max_drift_ppm.is_some() {
                AlignmentMethod::ConstrainedOffsetDrift
            } else {
                AlignmentMethod::EventCorrelation
            },
            algorithm: "weighted-least-squares-affine-clock".into(),
            algorithm_version: "1".into(),
            offset_s,
            scale,
            overlap_source_s,
            objective: "weighted target-time squared residual".into(),
            objective_value: squared_error,
            confidence,
            discontinuities: vec![],
            evidence_hashes,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != EVIDENCE_SCHEMA_VERSION {
            return Err("unsupported alignment schema version".into());
        }
        if self.source_domain.0.trim().is_empty()
            || self.target_domain.0.trim().is_empty()
            || self.source_domain == self.target_domain
        {
            return Err("alignment requires distinct named clock domains".into());
        }
        if !self.offset_s.is_finite()
            || !self.scale.is_finite()
            || self.scale <= 0.0
            || self.overlap_source_s.iter().any(|value| !value.is_finite())
            || self.overlap_source_s[0] > self.overlap_source_s[1]
            || !self.objective_value.is_finite()
            || !self.confidence.is_finite()
            || !(0.0..=1.0).contains(&self.confidence)
        {
            return Err("alignment contains invalid parameters, overlap, or confidence".into());
        }
        if self.algorithm.trim().is_empty()
            || self.algorithm_version.trim().is_empty()
            || self.objective.trim().is_empty()
        {
            return Err("alignment algorithm and objective must be explicit".into());
        }
        for hash in &self.evidence_hashes {
            validate_hash(hash)?;
        }
        for discontinuity in &self.discontinuities {
            if !discontinuity.source_time_s.is_finite() || !discontinuity.delta_s.is_finite() {
                return Err("clock discontinuities must be finite".into());
            }
            validate_hash(&discontinuity.evidence_hash)?;
        }
        Ok(())
    }

    pub fn map(&self, source_time_s: f64) -> Result<f64, String> {
        self.validate()?;
        if !source_time_s.is_finite() {
            return Err("source timestamp must be finite".into());
        }
        if !(self.overlap_source_s[0]..=self.overlap_source_s[1]).contains(&source_time_s) {
            return Err("source timestamp is outside valid alignment overlap".into());
        }
        Ok(source_time_s * self.scale + self.offset_s)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| error.to_string())
    }
}

fn validate_hash(hash: &str) -> Result<(), String> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid alignment evidence hash {hash}"));
    }
    Ok(())
}
