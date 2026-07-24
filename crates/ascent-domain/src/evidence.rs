//! Versioned, solver-independent temporal evidence contracts.
//!
//! These types deliberately live in `ascent-domain`: simulation, ingestion,
//! review, UI, and agent surfaces must project the same evidence model rather
//! than defining transport-specific variants.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EVIDENCE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClockDomain(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeSystem {
    SimulationElapsed,
    DeviceElapsed,
    UnixUtc,
    Gps,
    Tai,
    JournalSequence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeBase {
    pub id: String,
    pub system: TimeSystem,
    /// ISO-8601 or a system-specific epoch identifier. Elapsed clocks may omit it.
    pub epoch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameKind {
    EarthCenteredInertial,
    EarthCenteredEarthFixed,
    NorthEastDown,
    EastNorthUp,
    VehicleBody,
    SensorBody,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinateFrame {
    pub id: String,
    pub kind: FrameKind,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Raw,
    Calibrated,
    Estimated,
    Simulated,
    Derived,
    Presentation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Position,
    Velocity,
    AttitudeQuaternionWxyz,
    BodyRates,
    Acceleration,
    SpecificForce,
    Environment,
    Configuration,
    ActuatorState,
    Scalar,
    CameraPose,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Meter,
    MeterPerSecond,
    MeterPerSecondSquared,
    Radian,
    RadianPerSecond,
    Pascal,
    Kelvin,
    Kilogram,
    Second,
    Dimensionless,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Timestamp in the channel's declared source time base. Never aligned in place.
    pub time: f64,
    pub values: Vec<f64>,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Uncertainty {
    /// Named representation such as `one_sigma`, `diagonal_covariance`, or
    /// `upper_triangular_covariance`.
    pub representation: String,
    pub values: Vec<f64>,
    pub unit: Unit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub kind: ChannelKind,
    pub track: TrackKind,
    pub time_base_id: String,
    pub frame_id: Option<String>,
    pub unit: Unit,
    pub samples: Vec<Sample>,
    pub uncertainty: Option<Uncertainty>,
    pub parent_hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedEvent {
    pub id: String,
    pub event_type: String,
    pub time_base_id: String,
    pub time: f64,
    pub detector_id: String,
    pub detector_version: String,
    pub confidence: f64,
    pub source_sample_ranges: Vec<String>,
    pub evidence_hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlightTrace {
    pub schema_version: u16,
    pub trace_id: String,
    pub time_bases: Vec<TimeBase>,
    pub frames: Vec<CoordinateFrame>,
    pub channels: Vec<Channel>,
    pub events: Vec<TypedEvent>,
}

impl FlightTrace {
    pub fn validate(&self) -> Result<(), String> {
        validate_version(self.schema_version)?;
        require_id("trace", &self.trace_id)?;
        let time_bases = unique_ids("time base", self.time_bases.iter().map(|item| &item.id))?;
        let frames = unique_ids("frame", self.frames.iter().map(|item| &item.id))?;

        for frame in &self.frames {
            if frame
                .parent_id
                .as_ref()
                .is_some_and(|id| !frames.contains(id.as_str()))
            {
                return Err(format!("frame {} references unknown parent", frame.id));
            }
        }

        let mut channel_ids = HashSet::new();
        for channel in &self.channels {
            require_id("channel", &channel.id)?;
            if !channel_ids.insert(channel.id.as_str()) {
                return Err(format!("duplicate channel id {}", channel.id));
            }
            if !time_bases.contains(channel.time_base_id.as_str()) {
                return Err(format!("channel {} has unknown time base", channel.id));
            }
            if channel
                .frame_id
                .as_ref()
                .is_some_and(|id| !frames.contains(id.as_str()))
            {
                return Err(format!("channel {} has unknown frame", channel.id));
            }
            validate_channel(channel)?;
        }

        let mut event_ids = HashSet::new();
        for event in &self.events {
            require_id("event", &event.id)?;
            if !event_ids.insert(event.id.as_str()) {
                return Err(format!("duplicate event id {}", event.id));
            }
            if !time_bases.contains(event.time_base_id.as_str()) {
                return Err(format!("event {} has unknown time base", event.id));
            }
            if !event.time.is_finite() || !event.confidence.is_finite() {
                return Err(format!("event {} values must be finite", event.id));
            }
            if !(0.0..=1.0).contains(&event.confidence) {
                return Err(format!("event {} confidence must be in [0, 1]", event.id));
            }
            require_id("detector", &event.detector_id)?;
            require_id("detector version", &event.detector_version)?;
            validate_hashes(&event.evidence_hashes)?;
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        canonical_bytes(self)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, String> {
        from_canonical_bytes(bytes)
    }
}

fn validate_channel(channel: &Channel) -> Result<(), String> {
    if channel.track == TrackKind::Presentation && channel.kind != ChannelKind::CameraPose {
        return Err(format!(
            "presentation channel {} may contain camera pose only",
            channel.id
        ));
    }
    if channel.kind == ChannelKind::CameraPose && channel.track != TrackKind::Presentation {
        return Err(format!(
            "camera channel {} must be presentation-only",
            channel.id
        ));
    }
    validate_hashes(&channel.parent_hashes)?;
    if let Some(uncertainty) = &channel.uncertainty {
        require_id("uncertainty representation", &uncertainty.representation)?;
        if uncertainty.values.iter().any(|value| !value.is_finite()) {
            return Err(format!("channel {} uncertainty must be finite", channel.id));
        }
    }
    let expected_width = match channel.kind {
        ChannelKind::Position
        | ChannelKind::Velocity
        | ChannelKind::BodyRates
        | ChannelKind::Acceleration
        | ChannelKind::SpecificForce => Some(3),
        ChannelKind::AttitudeQuaternionWxyz => Some(4),
        ChannelKind::CameraPose => Some(7),
        _ => None,
    };
    let mut previous = None;
    for sample in &channel.samples {
        if !sample.time.is_finite() || sample.values.iter().any(|value| !value.is_finite()) {
            return Err(format!("channel {} samples must be finite", channel.id));
        }
        if previous.is_some_and(|time| sample.time <= time) {
            return Err(format!(
                "channel {} source clock must be strictly monotonic",
                channel.id
            ));
        }
        previous = Some(sample.time);
        if expected_width.is_some_and(|width| sample.values.len() != width) {
            return Err(format!("channel {} has invalid sample width", channel.id));
        }
        if channel.kind == ChannelKind::AttitudeQuaternionWxyz && sample.valid {
            let norm_squared = sample.values.iter().map(|value| value * value).sum::<f64>();
            if (norm_squared - 1.0).abs() > 1e-6 {
                return Err(format!("channel {} requires a unit quaternion", channel.id));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTelemetrySource {
    pub source_id: String,
    pub device: String,
    pub firmware: Option<String>,
    pub export_format: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationRecord {
    pub calibration_id: String,
    pub source_id: String,
    pub algorithm: String,
    pub algorithm_version: String,
    pub parameters: BTreeMap<String, f64>,
    pub parent_hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub sources: Vec<RawTelemetrySource>,
    pub calibrations: Vec<CalibrationRecord>,
    pub importer: ImportProvenance,
    /// Normalized tracks retain source-clock samples and lineage to raw sources.
    pub trace: FlightTrace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportProvenance {
    pub importer_id: String,
    pub importer_version: String,
    pub schema_hash: String,
    pub recorded_decisions: BTreeMap<String, String>,
}

impl TelemetryBundle {
    pub fn merge(bundle_id: impl Into<String>, bundles: Vec<Self>) -> Result<Self, String> {
        if bundles.is_empty() {
            return Err("cannot merge an empty telemetry collection".into());
        }
        for bundle in &bundles {
            bundle.validate()?;
        }
        let mut sources = Vec::new();
        let mut calibrations = Vec::new();
        let mut time_bases: BTreeMap<String, TimeBase> = BTreeMap::new();
        let mut frames: BTreeMap<String, CoordinateFrame> = BTreeMap::new();
        let mut channels = Vec::new();
        let mut events = Vec::new();
        let mut schema_hashes = Vec::new();
        let mut recorded_decisions = BTreeMap::new();
        for bundle in bundles {
            schema_hashes.push(bundle.importer.schema_hash.clone());
            recorded_decisions.insert(
                format!("bundle.{}", bundle.bundle_id),
                bundle.importer.schema_hash,
            );
            sources.extend(bundle.sources);
            calibrations.extend(bundle.calibrations);
            for time_base in bundle.trace.time_bases {
                if let Some(existing) = time_bases.get(&time_base.id) {
                    if existing != &time_base {
                        return Err(format!("conflicting time base {}", time_base.id));
                    }
                } else {
                    time_bases.insert(time_base.id.clone(), time_base);
                }
            }
            for frame in bundle.trace.frames {
                if let Some(existing) = frames.get(&frame.id) {
                    if existing != &frame {
                        return Err(format!("conflicting coordinate frame {}", frame.id));
                    }
                } else {
                    frames.insert(frame.id.clone(), frame);
                }
            }
            channels.extend(bundle.trace.channels);
            events.extend(bundle.trace.events);
        }
        schema_hashes.sort();
        let schema_hash = format!("{:x}", Sha256::digest(schema_hashes.join("\n").as_bytes()));
        let bundle = Self {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            bundle_id: bundle_id.into(),
            sources,
            calibrations,
            importer: ImportProvenance {
                importer_id: "ascent.telemetry-merge".into(),
                importer_version: env!("CARGO_PKG_VERSION").into(),
                schema_hash,
                recorded_decisions,
            },
            trace: FlightTrace {
                schema_version: EVIDENCE_SCHEMA_VERSION,
                trace_id: "merged-telemetry".into(),
                time_bases: time_bases.into_values().collect(),
                frames: frames.into_values().collect(),
                channels,
                events,
            },
        };
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_version(self.schema_version)?;
        require_id("telemetry bundle", &self.bundle_id)?;
        require_id("importer", &self.importer.importer_id)?;
        require_id("importer version", &self.importer.importer_version)?;
        validate_hash(&self.importer.schema_hash)?;
        self.trace.validate()?;
        let source_ids = unique_ids("source", self.sources.iter().map(|item| &item.source_id))?;
        for source in &self.sources {
            require_id("device", &source.device)?;
            validate_hash(&source.sha256)?;
            let actual = format!("{:x}", Sha256::digest(&source.bytes));
            if actual != source.sha256 {
                return Err(format!(
                    "raw source {} hash does not match bytes",
                    source.source_id
                ));
            }
        }
        for calibration in &self.calibrations {
            require_id("calibration", &calibration.calibration_id)?;
            if !source_ids.contains(calibration.source_id.as_str()) {
                return Err(format!(
                    "calibration {} references unknown source",
                    calibration.calibration_id
                ));
            }
            if calibration
                .parameters
                .values()
                .any(|value| !value.is_finite())
            {
                return Err(format!(
                    "calibration {} parameters must be finite",
                    calibration.calibration_id
                ));
            }
            validate_hashes(&calibration.parent_hashes)?;
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        canonical_bytes(self)
    }
}

/// The evidence ladder, weakest to strongest. Each rung names a *distinct kind*
/// of evidence, not merely "more testing":
///
/// - `Analytic` — agrees with a closed-form solution.
/// - `UnitVerified` — a unit/property test pins the behavior.
/// - `RegressionCompatible` — reproduces another tool's export within a frozen
///   tolerance (compatibility, not correctness).
/// - `CrossValidated` — agrees with an independent engine (e.g. RocketPy) on a
///   shared configuration.
/// - `FlightDataAvailable` — real measured flight data is present, license-clear
///   and hash-pinned, and the review plumbing ingests it — but the model's
///   numerical outputs have **not** yet been compared against it. This rung
///   makes a claim about *data availability*, never about model accuracy.
/// - `FlightValidated` — the model was executed and its numerical outputs were
///   compared against measured flight data, and the comparison **passed**
///   predefined tolerances. A case may only carry this rung if it also carries
///   an executed [`crate`]-external comparison artifact; the artifact's fields
///   (metrics, tolerances, pass/fail, source hashes, model version) are what the
///   label attests to. Without that artifact the label cannot be loaded — see
///   `ascent_review::validation::ValidationCase`.
///
/// The variants are *not* `Ord`: promotion between rungs is a deliberate,
/// evidence-gated act, never an automatic "greater-than".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLevel {
    Analytic,
    UnitVerified,
    RegressionCompatible,
    CrossValidated,
    FlightDataAvailable,
    FlightValidated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactEvidence {
    pub artifact_id: String,
    pub sha256: String,
    pub parent_hashes: Vec<String>,
    pub transformation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceManifest {
    pub schema_version: u16,
    pub manifest_id: String,
    pub intended_use: String,
    pub validity_domain: Vec<String>,
    pub evidence_level: EvidenceLevel,
    pub input_pedigree: Vec<String>,
    pub frozen_thresholds: BTreeMap<String, f64>,
    pub caveats: Vec<String>,
    pub artifacts: Vec<ArtifactEvidence>,
    pub root_hashes: BTreeSet<String>,
}

impl EvidenceManifest {
    pub fn validate(&self) -> Result<(), String> {
        validate_version(self.schema_version)?;
        require_id("manifest", &self.manifest_id)?;
        require_id("intended use", &self.intended_use)?;
        if self
            .frozen_thresholds
            .values()
            .any(|value| !value.is_finite())
        {
            return Err("evidence thresholds must be finite".into());
        }
        for root in &self.root_hashes {
            validate_hash(root)?;
        }
        let artifact_hashes = self
            .artifacts
            .iter()
            .map(|artifact| artifact.sha256.as_str())
            .collect::<HashSet<_>>();
        for artifact in &self.artifacts {
            require_id("artifact", &artifact.artifact_id)?;
            validate_hash(&artifact.sha256)?;
            for parent in &artifact.parent_hashes {
                validate_hash(parent)?;
                if !self.root_hashes.contains(parent) && !artifact_hashes.contains(parent.as_str())
                {
                    return Err(format!(
                        "artifact {} has broken parent hash {}",
                        artifact.artifact_id, parent
                    ));
                }
                if parent == &artifact.sha256 {
                    return Err(format!(
                        "artifact {} cannot parent itself",
                        artifact.artifact_id
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        canonical_bytes(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AffineClockTransform {
    pub target_domain: ClockDomain,
    pub offset_s: f64,
    pub scale: f64,
    pub valid_source_interval_s: Option<[f64; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewClock {
    review_domain: ClockDomain,
    transforms: BTreeMap<ClockDomain, AffineClockTransform>,
}

impl ReviewClock {
    pub fn new(
        review_domain: ClockDomain,
        transforms: BTreeMap<ClockDomain, AffineClockTransform>,
    ) -> Result<Self, String> {
        require_id("review clock domain", &review_domain.0)?;
        for (source, transform) in &transforms {
            require_id("source clock domain", &source.0)?;
            if transform.target_domain != review_domain {
                return Err(format!("clock {} does not target review domain", source.0));
            }
            if !transform.offset_s.is_finite()
                || !transform.scale.is_finite()
                || transform.scale <= 0.0
            {
                return Err(format!(
                    "clock {} transform must be finite and positive",
                    source.0
                ));
            }
            if let Some([start, end]) = transform.valid_source_interval_s {
                if !start.is_finite() || !end.is_finite() || start > end {
                    return Err(format!("clock {} has invalid source interval", source.0));
                }
            }
        }
        Ok(Self {
            review_domain,
            transforms,
        })
    }

    pub fn review_domain(&self) -> &ClockDomain {
        &self.review_domain
    }

    pub fn map_to_review(&self, source: &ClockDomain, source_time: f64) -> Result<f64, String> {
        if !source_time.is_finite() {
            return Err("source timestamp must be finite".into());
        }
        if source == &self.review_domain {
            return Ok(source_time);
        }
        let transform = self
            .transforms
            .get(source)
            .ok_or_else(|| format!("no transform for clock {}", source.0))?;
        if let Some([start, end]) = transform.valid_source_interval_s {
            if !(start..=end).contains(&source_time) {
                return Err(format!("timestamp outside clock {} overlap", source.0));
            }
        }
        Ok(source_time * transform.scale + transform.offset_s)
    }
}

fn validate_version(version: u16) -> Result<(), String> {
    if version != EVIDENCE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported evidence schema version {version}; expected {EVIDENCE_SCHEMA_VERSION}"
        ));
    }
    Ok(())
}

fn require_id(kind: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{kind} must not be empty"));
    }
    Ok(())
}

fn unique_ids<'a>(
    kind: &str,
    ids: impl Iterator<Item = &'a String>,
) -> Result<HashSet<&'a str>, String> {
    let mut result = HashSet::new();
    for id in ids {
        require_id(kind, id)?;
        if !result.insert(id.as_str()) {
            return Err(format!("duplicate {kind} id {id}"));
        }
    }
    Ok(result)
}

fn validate_hashes(hashes: &[String]) -> Result<(), String> {
    for hash in hashes {
        validate_hash(hash)?;
    }
    Ok(())
}

fn validate_hash(hash: &str) -> Result<(), String> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid SHA-256 hash {hash}"));
    }
    Ok(())
}

fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|error| format!("canonical serialization failed: {error}"))
}

fn from_canonical_bytes<T>(bytes: &[u8]) -> Result<T, String>
where
    T: DeserializeOwned + Serialize + Validated,
{
    let value: T = serde_json::from_slice(bytes)
        .map_err(|error| format!("canonical evidence decode failed: {error}"))?;
    value.validate_value()?;
    if canonical_bytes(&value)? != bytes {
        return Err("input is valid JSON but not canonical evidence bytes".into());
    }
    Ok(value)
}

trait Validated {
    fn validate_value(&self) -> Result<(), String>;
}

impl Validated for FlightTrace {
    fn validate_value(&self) -> Result<(), String> {
        self.validate()
    }
}
