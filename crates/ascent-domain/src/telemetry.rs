//! Schema-driven import into immutable [`TelemetryBundle`] evidence.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::evidence::{
    Channel, ChannelKind, CoordinateFrame, FlightTrace, FrameKind, ImportProvenance,
    RawTelemetrySource, Sample, TelemetryBundle, TimeBase, TimeSystem, TrackKind, Uncertainty,
    Unit, EVIDENCE_SCHEMA_VERSION,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportFormat {
    Csv {
        delimiter: String,
        has_header: bool,
    },
    JsonLines,
    Binary {
        record_size: usize,
        fields: BTreeMap<String, BinaryField>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinaryField {
    pub offset: usize,
    pub value_type: BinaryValueType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryValueType {
    F64Le,
    F32Le,
    I32Le,
    U32Le,
    I16Le,
    U16Le,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceSchema {
    pub source_id: String,
    pub device: String,
    pub firmware: Option<String>,
    pub export_format: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeMapping {
    pub field: String,
    pub scale_to_seconds: f64,
    pub domain: String,
    pub system: TimeSystem,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameSchema {
    pub id: String,
    pub kind: FrameKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelMapping {
    pub fields: Vec<String>,
    pub id: String,
    pub kind: ChannelKind,
    pub track: TrackKind,
    pub unit: Option<Unit>,
    pub scale: f64,
    pub offset: f64,
    pub uncertainty: Option<f64>,
    #[serde(default)]
    pub valid_field: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSchema {
    pub schema_version: u16,
    pub schema_id: String,
    pub format: ImportFormat,
    pub source: SourceSchema,
    pub time: TimeMapping,
    pub frame: FrameSchema,
    pub channels: Vec<ChannelMapping>,
}

impl ImportSchema {
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let schema: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("telemetry schema is invalid: {error}"))?;
        schema.validate()?;
        Ok(schema)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != EVIDENCE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported telemetry schema version {}",
                self.schema_version
            ));
        }
        if self.schema_id.trim().is_empty()
            || self.source.source_id.trim().is_empty()
            || self.time.field.trim().is_empty()
            || self.time.domain.trim().is_empty()
            || self.frame.id.trim().is_empty()
            || self.channels.is_empty()
        {
            return Err("telemetry schema has missing required identifiers".into());
        }
        if !self.time.scale_to_seconds.is_finite() || self.time.scale_to_seconds <= 0.0 {
            return Err("time scale must be finite and positive".into());
        }
        for channel in &self.channels {
            if channel.fields.is_empty() || channel.id.trim().is_empty() {
                return Err("channel mapping must declare fields and id".into());
            }
            if channel.unit.is_none() {
                return Err(format!("channel {} requires an explicit unit", channel.id));
            }
            if !channel.scale.is_finite() || !channel.offset.is_finite() {
                return Err(format!("channel {} transform must be finite", channel.id));
            }
            if channel
                .uncertainty
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            {
                return Err(format!("channel {} uncertainty is invalid", channel.id));
            }
        }
        Ok(())
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| error.to_string())
    }
}

pub struct TelemetryImporter;

impl TelemetryImporter {
    pub fn ingest(schema: &ImportSchema, raw: &[u8]) -> Result<TelemetryBundle, String> {
        schema.validate()?;
        let rows = parse_rows(&schema.format, raw)?;
        if rows.is_empty() {
            return Err("telemetry source contains no records".into());
        }
        let source_hash = hash_bytes(raw);
        let mut channels = Vec::with_capacity(schema.channels.len());
        for mapping in &schema.channels {
            let mut samples = Vec::with_capacity(rows.len());
            for row in &rows {
                let source_time = required_value(row, &schema.time.field)?;
                let time = source_time * schema.time.scale_to_seconds;
                let values = mapping
                    .fields
                    .iter()
                    .map(|field| {
                        required_value(row, field)
                            .map(|value| value * mapping.scale + mapping.offset)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let valid = mapping
                    .valid_field
                    .as_ref()
                    .map(|field| required_value(row, field).map(|value| value != 0.0))
                    .transpose()?
                    .unwrap_or(true);
                samples.push(Sample {
                    time,
                    values,
                    valid,
                });
            }
            let unit = mapping
                .unit
                .clone()
                .ok_or_else(|| format!("channel {} requires an explicit unit", mapping.id))?;
            channels.push(Channel {
                id: mapping.id.clone(),
                kind: mapping.kind.clone(),
                track: mapping.track.clone(),
                time_base_id: schema.time.domain.clone(),
                frame_id: Some(schema.frame.id.clone()),
                unit: unit.clone(),
                samples,
                uncertainty: mapping.uncertainty.map(|value| Uncertainty {
                    representation: "one_sigma".into(),
                    values: vec![value],
                    unit,
                }),
                parent_hashes: vec![source_hash.clone()],
            });
        }
        let trace = FlightTrace {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            trace_id: format!("normalized-{}", schema.source.source_id),
            time_bases: vec![TimeBase {
                id: schema.time.domain.clone(),
                system: schema.time.system.clone(),
                epoch: None,
            }],
            frames: vec![CoordinateFrame {
                id: schema.frame.id.clone(),
                kind: schema.frame.kind.clone(),
                parent_id: None,
            }],
            channels,
            events: vec![],
        };
        let schema_hash = hash_bytes(&schema.canonical_bytes()?);
        let bundle = TelemetryBundle {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            bundle_id: format!("{}-{}", schema.schema_id, &source_hash[..12]),
            sources: vec![RawTelemetrySource {
                source_id: schema.source.source_id.clone(),
                device: schema.source.device.clone(),
                firmware: schema.source.firmware.clone(),
                export_format: schema.source.export_format.clone(),
                sha256: source_hash,
                bytes: raw.to_vec(),
            }],
            calibrations: vec![],
            importer: ImportProvenance {
                importer_id: "ascent.schema-telemetry".into(),
                importer_version: env!("CARGO_PKG_VERSION").into(),
                schema_hash,
                recorded_decisions: BTreeMap::from([
                    ("schema_id".into(), schema.schema_id.clone()),
                    ("frame".into(), schema.frame.id.clone()),
                    ("time_domain".into(), schema.time.domain.clone()),
                ]),
            },
            trace,
        };
        bundle.validate()?;
        Ok(bundle)
    }
}

fn parse_rows(format: &ImportFormat, raw: &[u8]) -> Result<Vec<BTreeMap<String, f64>>, String> {
    match format {
        ImportFormat::Csv {
            delimiter,
            has_header,
        } => parse_csv(raw, delimiter, *has_header),
        ImportFormat::JsonLines => parse_json_lines(raw),
        ImportFormat::Binary {
            record_size,
            fields,
        } => parse_binary(raw, *record_size, fields),
    }
}

fn parse_csv(
    raw: &[u8],
    delimiter: &str,
    has_header: bool,
) -> Result<Vec<BTreeMap<String, f64>>, String> {
    if !has_header {
        return Err("CSV telemetry requires a recorded header mapping".into());
    }
    let delimiter = delimiter
        .chars()
        .next()
        .filter(|_| delimiter.chars().count() == 1)
        .ok_or_else(|| "CSV delimiter must be one character".to_string())?;
    let text = std::str::from_utf8(raw).map_err(|error| format!("CSV is not UTF-8: {error}"))?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let headers = split_csv_line(lines.next().ok_or("CSV is empty")?, delimiter)?;
    let mut rows = Vec::new();
    for (index, line) in lines.enumerate() {
        let fields = split_csv_line(line, delimiter)?;
        if fields.len() != headers.len() {
            return Err(format!(
                "CSV line {} has {} fields; expected {}",
                index + 2,
                fields.len(),
                headers.len()
            ));
        }
        let row = headers
            .iter()
            .zip(fields)
            .map(|(header, value)| {
                value
                    .trim()
                    .parse::<f64>()
                    .map(|number| (header.trim().to_string(), number))
                    .map_err(|_| format!("CSV line {} field {} is not numeric", index + 2, header))
            })
            .collect::<Result<_, _>>()?;
        rows.push(row);
    }
    Ok(rows)
}

fn split_csv_line(line: &str, delimiter: char) -> Result<Vec<String>, String> {
    let mut fields = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                fields.last_mut().unwrap().push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            value if value == delimiter && !quoted => fields.push(String::new()),
            value => fields.last_mut().unwrap().push(value),
        }
    }
    if quoted {
        return Err("CSV contains an unterminated quote".into());
    }
    Ok(fields)
}

fn parse_json_lines(raw: &[u8]) -> Result<Vec<BTreeMap<String, f64>>, String> {
    let text =
        std::str::from_utf8(raw).map_err(|error| format!("JSON lines is not UTF-8: {error}"))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(index, line)| {
            let object: BTreeMap<String, serde_json::Value> = serde_json::from_str(line)
                .map_err(|error| format!("JSON line {} is invalid: {error}", index + 1))?;
            object
                .into_iter()
                .map(|(key, value)| {
                    value
                        .as_f64()
                        .map(|number| (key.clone(), number))
                        .ok_or_else(|| {
                            format!("JSON line {} field {key} is not numeric", index + 1)
                        })
                })
                .collect()
        })
        .collect()
}

fn parse_binary(
    raw: &[u8],
    record_size: usize,
    fields: &BTreeMap<String, BinaryField>,
) -> Result<Vec<BTreeMap<String, f64>>, String> {
    if record_size == 0 || !raw.len().is_multiple_of(record_size) {
        return Err("binary telemetry length is not an exact record multiple".into());
    }
    raw.chunks_exact(record_size)
        .map(|record| {
            fields
                .iter()
                .map(|(name, field)| read_binary(record, field).map(|value| (name.clone(), value)))
                .collect()
        })
        .collect()
}

fn read_binary(record: &[u8], field: &BinaryField) -> Result<f64, String> {
    let slice = |width: usize| {
        record
            .get(field.offset..field.offset + width)
            .ok_or_else(|| format!("binary field at offset {} exceeds record", field.offset))
    };
    Ok(match field.value_type {
        BinaryValueType::F64Le => f64::from_le_bytes(slice(8)?.try_into().unwrap()),
        BinaryValueType::F32Le => f32::from_le_bytes(slice(4)?.try_into().unwrap()) as f64,
        BinaryValueType::I32Le => i32::from_le_bytes(slice(4)?.try_into().unwrap()) as f64,
        BinaryValueType::U32Le => u32::from_le_bytes(slice(4)?.try_into().unwrap()) as f64,
        BinaryValueType::I16Le => i16::from_le_bytes(slice(2)?.try_into().unwrap()) as f64,
        BinaryValueType::U16Le => u16::from_le_bytes(slice(2)?.try_into().unwrap()) as f64,
    })
}

fn required_value(row: &BTreeMap<String, f64>, field: &str) -> Result<f64, String> {
    row.get(field)
        .copied()
        .ok_or_else(|| format!("missing column or field {field}"))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinSchema {
    PerfectFliteCsv,
    Rrc3Csv,
    BlueRavenCsv,
}

pub fn builtin_schema(kind: BuiltinSchema) -> ImportSchema {
    let (schema_id, device, time, altitude) = match kind {
        BuiltinSchema::PerfectFliteCsv => (
            "edge.perfectflite.csv-v1",
            "PerfectFlite",
            "Time",
            "Altitude",
        ),
        BuiltinSchema::Rrc3Csv => ("edge.rrc3.csv-v1", "Missile Works RRC3", "Time", "Altitude"),
        BuiltinSchema::BlueRavenCsv => (
            "edge.blueraven.csv-v1",
            "Featherweight Blue Raven",
            "time_s",
            "altitude_m",
        ),
    };
    ImportSchema {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        schema_id: schema_id.into(),
        format: ImportFormat::Csv {
            delimiter: ",".into(),
            has_header: true,
        },
        source: SourceSchema {
            source_id: device.to_ascii_lowercase().replace(' ', "-"),
            device: device.into(),
            firmware: None,
            export_format: "vendor CSV edge compatibility".into(),
        },
        time: TimeMapping {
            field: time.into(),
            scale_to_seconds: 1.0,
            domain: "device-elapsed".into(),
            system: TimeSystem::DeviceElapsed,
        },
        frame: FrameSchema {
            id: "launch-enu".into(),
            kind: FrameKind::EastNorthUp,
        },
        channels: vec![ChannelMapping {
            fields: vec![altitude.into()],
            id: "barometry.altitude".into(),
            kind: ChannelKind::Scalar,
            track: TrackKind::Raw,
            unit: Some(Unit::Meter),
            scale: 1.0,
            offset: 0.0,
            uncertainty: None,
            valid_field: None,
        }],
    }
}
