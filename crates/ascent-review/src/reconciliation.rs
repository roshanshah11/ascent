use ascent_domain::evidence::{Channel, ChannelKind, FlightTrace, Unit, EVIDENCE_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};

use crate::alignment::AlignmentArtifact;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelPair {
    pub predicted_id: String,
    pub measured_id: String,
}

impl ChannelPair {
    pub fn new(predicted_id: impl Into<String>, measured_id: impl Into<String>) -> Self {
        Self {
            predicted_id: predicted_id.into(),
            measured_id: measured_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseWindow {
    pub name: String,
    pub start_review_s: f64,
    pub end_review_s: f64,
}

impl PhaseWindow {
    pub fn new(
        name: impl Into<String>,
        start_review_s: f64,
        end_review_s: f64,
    ) -> Result<Self, String> {
        let phase = Self {
            name: name.into(),
            start_review_s,
            end_review_s,
        };
        if phase.name.trim().is_empty()
            || !start_review_s.is_finite()
            || !end_review_s.is_finite()
            || start_review_s >= end_review_s
        {
            return Err("phase requires a name and increasing finite interval".into());
        }
        Ok(phase)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualPoint {
    pub review_time_s: f64,
    pub predicted_time_s: f64,
    pub measured_source_time_s: f64,
    pub value: f64,
    pub normalized: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualMetrics {
    pub count: usize,
    pub bias: f64,
    pub mae: f64,
    pub rmse: f64,
    pub max_absolute: f64,
    pub uncertainty_coverage: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseMetrics {
    pub phase: String,
    pub metrics: ResidualMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelReconciliation {
    pub quantity: String,
    pub predicted_channel: String,
    pub measured_channel: String,
    pub frame_id: Option<String>,
    pub unit: Unit,
    pub residuals: Vec<ResidualPoint>,
    pub whole_flight: ResidualMetrics,
    pub phases: Vec<PhaseMetrics>,
    pub normalization_denominator: String,
    pub completeness: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventDelta {
    pub event_type: String,
    pub predicted_time_s: f64,
    pub measured_source_time_s: f64,
    pub measured_review_time_s: f64,
    pub delta_s: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticCandidate {
    pub name: String,
    pub residual_signature: Vec<f64>,
    pub evidence_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticHypothesis {
    pub name: String,
    pub label: String,
    pub similarity: f64,
    pub evidence_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconciliationResult {
    pub schema_version: u16,
    pub predicted_trace_id: String,
    pub measured_trace_id: String,
    pub alignment: AlignmentArtifact,
    pub channels: Vec<ChannelReconciliation>,
    pub event_deltas: Vec<EventDelta>,
    pub hypotheses: Vec<DiagnosticHypothesis>,
    pub qualifications: Vec<String>,
    /// Frozen comparison definition required to rerun the same official
    /// reconciliation for a counterfactual trace. Serde defaults retain
    /// compatibility with early v0.6 artifacts, which remain viewable but
    /// are explicitly not rerunnable.
    #[serde(default)]
    pub channel_pairs: Vec<ChannelPair>,
    #[serde(default)]
    pub phase_windows: Vec<PhaseWindow>,
    #[serde(default)]
    pub diagnostic_candidates: Vec<DiagnosticCandidate>,
}

impl ReconciliationResult {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|error| error.to_string())
    }
}

pub struct Reconciler;

impl Reconciler {
    pub fn compare(
        predicted: &FlightTrace,
        measured: &FlightTrace,
        alignment: &AlignmentArtifact,
        pairs: &[ChannelPair],
        phases: &[PhaseWindow],
        candidates: &[DiagnosticCandidate],
    ) -> Result<ReconciliationResult, String> {
        predicted.validate()?;
        measured.validate()?;
        alignment.validate()?;
        if pairs.is_empty() || phases.is_empty() {
            return Err("reconciliation needs channels and phase definitions".into());
        }
        if alignment.target_domain.0
            != predicted
                .time_bases
                .first()
                .ok_or("predicted trace has no time base")?
                .id
        {
            return Err("alignment target does not match predicted review domain".into());
        }
        let mut channels = Vec::with_capacity(pairs.len());
        let mut qualifications = Vec::new();
        for pair in pairs {
            let predicted_channel = find_channel(predicted, &pair.predicted_id)?;
            let measured_channel = find_channel(measured, &pair.measured_id)?;
            if predicted_channel.kind != measured_channel.kind
                || predicted_channel.unit != measured_channel.unit
                || predicted_channel.frame_id != measured_channel.frame_id
            {
                return Err(format!(
                    "channel mapping {} to {} has incompatible kind, unit, or frame",
                    pair.predicted_id, pair.measured_id
                ));
            }
            if measured_channel.time_base_id != alignment.source_domain.0 {
                return Err(format!(
                    "measured channel {} uses the wrong clock domain",
                    pair.measured_id
                ));
            }
            let sigma = measured_channel
                .uncertainty
                .as_ref()
                .and_then(|uncertainty| uncertainty.values.first())
                .copied()
                .filter(|value| *value > 0.0);
            if sigma.is_none() {
                qualifications.push(format!(
                    "{} has no usable measured uncertainty",
                    pair.measured_id
                ));
            }
            let mut residuals = Vec::new();
            let valid_count = measured_channel
                .samples
                .iter()
                .filter(|sample| sample.valid)
                .count();
            for measured_sample in measured_channel
                .samples
                .iter()
                .filter(|sample| sample.valid)
            {
                let review_time = match alignment.map(measured_sample.time) {
                    Ok(time) => time,
                    Err(_) => continue,
                };
                let predicted_values = interpolate(predicted_channel, review_time)?;
                let value = residual_value(
                    &predicted_channel.kind,
                    &predicted_values,
                    &measured_sample.values,
                )?;
                residuals.push(ResidualPoint {
                    review_time_s: review_time,
                    predicted_time_s: review_time,
                    measured_source_time_s: measured_sample.time,
                    value,
                    normalized: sigma.map(|denominator| value / denominator),
                });
            }
            if residuals.is_empty() {
                return Err(format!(
                    "channel {} has no aligned overlap",
                    pair.measured_id
                ));
            }
            let whole_flight = metrics(&residuals, sigma);
            let phase_metrics = phases
                .iter()
                .filter_map(|phase| {
                    let selected = residuals
                        .iter()
                        .filter(|sample| {
                            sample.review_time_s >= phase.start_review_s
                                && sample.review_time_s < phase.end_review_s
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    (!selected.is_empty()).then(|| PhaseMetrics {
                        phase: phase.name.clone(),
                        metrics: metrics(&selected, sigma),
                    })
                })
                .collect();
            channels.push(ChannelReconciliation {
                quantity: pair.predicted_id.clone(),
                predicted_channel: pair.predicted_id.clone(),
                measured_channel: pair.measured_id.clone(),
                frame_id: predicted_channel.frame_id.clone(),
                unit: predicted_channel.unit.clone(),
                residuals,
                whole_flight,
                phases: phase_metrics,
                normalization_denominator: sigma.map_or_else(
                    || "none; normalized residual unavailable".into(),
                    |value| {
                        format!(
                            "measured one_sigma = {value} {}",
                            unit_name(&measured_channel.unit)
                        )
                    },
                ),
                completeness: valid_count as f64 / measured_channel.samples.len().max(1) as f64,
            });
        }
        let event_deltas = event_deltas(predicted, measured, alignment)?;
        let signature = channels
            .first()
            .map(|channel| {
                channel
                    .residuals
                    .iter()
                    .map(|sample| sample.value)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut hypotheses = candidates
            .iter()
            .map(|candidate| {
                validate_hash(&candidate.evidence_hash)?;
                if candidate.name.trim().is_empty()
                    || candidate.residual_signature.len() != signature.len()
                {
                    return Err(
                        "diagnostic candidates need a name and matching signature length".into(),
                    );
                }
                Ok(DiagnosticHypothesis {
                    name: candidate.name.clone(),
                    label: "diagnostic hypothesis; not a proven cause".into(),
                    similarity: cosine_similarity(&signature, &candidate.residual_signature),
                    evidence_hash: candidate.evidence_hash.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        hypotheses.sort_by(|left, right| {
            right
                .similarity
                .total_cmp(&left.similarity)
                .then_with(|| left.name.cmp(&right.name))
        });
        if alignment.confidence < 0.8 {
            qualifications.push(format!(
                "alignment confidence is {:.3}",
                alignment.confidence
            ));
        }
        Ok(ReconciliationResult {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            predicted_trace_id: predicted.trace_id.clone(),
            measured_trace_id: measured.trace_id.clone(),
            alignment: alignment.clone(),
            channels,
            event_deltas,
            hypotheses,
            qualifications,
            channel_pairs: pairs.to_vec(),
            phase_windows: phases.to_vec(),
            diagnostic_candidates: candidates.to_vec(),
        })
    }
}

fn find_channel<'a>(trace: &'a FlightTrace, id: &str) -> Result<&'a Channel, String> {
    trace
        .channels
        .iter()
        .find(|channel| channel.id == id)
        .ok_or_else(|| format!("trace {} has no channel {id}", trace.trace_id))
}

fn interpolate(channel: &Channel, time: f64) -> Result<Vec<f64>, String> {
    if let Some(sample) = channel
        .samples
        .iter()
        .find(|sample| sample.time == time && sample.valid)
    {
        return Ok(sample.values.clone());
    }
    let pair = channel
        .samples
        .windows(2)
        .find(|pair| pair[0].valid && pair[1].valid && pair[0].time < time && time < pair[1].time)
        .ok_or_else(|| {
            format!(
                "channel {} has no valid sample at review time {time}",
                channel.id
            )
        })?;
    let fraction = (time - pair[0].time) / (pair[1].time - pair[0].time);
    let mut values = pair[0]
        .values
        .iter()
        .zip(&pair[1].values)
        .map(|(from, to)| from + fraction * (to - from))
        .collect::<Vec<_>>();
    if channel.kind == ChannelKind::AttitudeQuaternionWxyz {
        let norm = values.iter().map(|value| value * value).sum::<f64>().sqrt();
        if norm <= f64::EPSILON {
            return Err("interpolated attitude has zero norm".into());
        }
        for value in &mut values {
            *value /= norm;
        }
    }
    Ok(values)
}

fn residual_value(kind: &ChannelKind, predicted: &[f64], measured: &[f64]) -> Result<f64, String> {
    if predicted.len() != measured.len() {
        return Err("predicted and measured sample widths differ".into());
    }
    if *kind == ChannelKind::AttitudeQuaternionWxyz {
        let dot = predicted
            .iter()
            .zip(measured)
            .map(|(left, right)| left * right)
            .sum::<f64>()
            .abs()
            .clamp(-1.0, 1.0);
        return Ok(2.0 * dot.acos());
    }
    if predicted.len() == 1 {
        return Ok(predicted[0] - measured[0]);
    }
    Ok(predicted
        .iter()
        .zip(measured)
        .map(|(left, right)| (left - right).powi(2))
        .sum::<f64>()
        .sqrt())
}

fn metrics(residuals: &[ResidualPoint], sigma: Option<f64>) -> ResidualMetrics {
    let count = residuals.len();
    let bias = residuals.iter().map(|sample| sample.value).sum::<f64>() / count as f64;
    let mae = residuals
        .iter()
        .map(|sample| sample.value.abs())
        .sum::<f64>()
        / count as f64;
    let rmse = (residuals
        .iter()
        .map(|sample| sample.value.powi(2))
        .sum::<f64>()
        / count as f64)
        .sqrt();
    let max_absolute = residuals
        .iter()
        .map(|sample| sample.value.abs())
        .fold(0.0_f64, f64::max);
    let uncertainty_coverage = sigma.map(|denominator| {
        residuals
            .iter()
            .filter(|sample| sample.value.abs() <= denominator)
            .count() as f64
            / count as f64
    });
    ResidualMetrics {
        count,
        bias,
        mae,
        rmse,
        max_absolute,
        uncertainty_coverage,
    }
}

fn event_deltas(
    predicted: &FlightTrace,
    measured: &FlightTrace,
    alignment: &AlignmentArtifact,
) -> Result<Vec<EventDelta>, String> {
    let mut result = Vec::new();
    for predicted_event in &predicted.events {
        if let Some(measured_event) = measured
            .events
            .iter()
            .find(|event| event.event_type == predicted_event.event_type)
        {
            let review_time = alignment.map(measured_event.time)?;
            result.push(EventDelta {
                event_type: predicted_event.event_type.clone(),
                predicted_time_s: predicted_event.time,
                measured_source_time_s: measured_event.time,
                measured_review_time_s: review_time,
                delta_s: review_time - predicted_event.time,
                confidence: predicted_event.confidence.min(measured_event.confidence),
            });
        }
    }
    Ok(result)
}

fn cosine_similarity(left: &[f64], right: &[f64]) -> f64 {
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f64>();
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    if left_norm <= f64::EPSILON || right_norm <= f64::EPSILON {
        0.0
    } else {
        (dot / (left_norm * right_norm)).clamp(-1.0, 1.0)
    }
}

fn unit_name(unit: &Unit) -> String {
    match unit {
        Unit::Meter => "meter".into(),
        Unit::MeterPerSecond => "meter_per_second".into(),
        Unit::MeterPerSecondSquared => "meter_per_second_squared".into(),
        Unit::Radian => "radian".into(),
        Unit::RadianPerSecond => "radian_per_second".into(),
        Unit::Pascal => "pascal".into(),
        Unit::Kelvin => "kelvin".into(),
        Unit::Kilogram => "kilogram".into(),
        Unit::Second => "second".into(),
        Unit::Dimensionless => "dimensionless".into(),
        Unit::Custom(name) => name.clone(),
    }
}

fn validate_hash(hash: &str) -> Result<(), String> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid diagnostic evidence hash {hash}"));
    }
    Ok(())
}
