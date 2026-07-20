use crate::document::{DecisionMetadata, JournalOp, JOURNAL_VERSION};
use crate::{Command, Document};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A presentation-only annotation. Story edits never enter the engineering
/// journal or alter a checkpoint document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoryBeat {
    pub review_time_s: f64,
    pub camera_preset: String,
    pub briefing: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CinemaCheckpoint {
    pub journal_line: usize,
    pub author: String,
    pub source: String,
    pub intent: String,
    pub canonical_command: String,
    pub affected_requirements: Vec<String>,
    pub evidence_hashes: Vec<String>,
    pub before_metrics: Value,
    pub after_metrics: Value,
    pub journal_prefix: String,
    pub document_bytes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignCinema {
    pub checkpoints: Vec<CinemaCheckpoint>,
    pub final_document_bytes: String,
    #[serde(default)]
    pub story: Vec<StoryBeat>,
}

impl CampaignCinema {
    pub fn from_journal(jsonl: &str) -> Result<Self, String> {
        let lines: Vec<&str> = jsonl.lines().collect();
        let header = lines.first().ok_or("empty journal")?;
        let header_value: Value = serde_json::from_str(header)
            .map_err(|error| format!("line 1: bad journal header: {error}"))?;
        let version = header_value
            .get("journal_version")
            .and_then(Value::as_u64)
            .ok_or("line 1: missing journal_version")?;
        if version != u64::from(JOURNAL_VERSION) {
            return Err(format!(
                "journal version {version} not supported (this build reads {JOURNAL_VERSION})"
            ));
        }

        let mut checkpoints = Vec::new();
        for index in 1..lines.len() {
            if lines[index].trim().is_empty() {
                continue;
            }
            let op: JournalOp = serde_json::from_str(lines[index])
                .map_err(|error| format!("line {}: {error}", index + 1))?;
            if let JournalOp::Dispatch { command, metadata } = op {
                let before_prefix = journal_prefix(&lines, index);
                let after_prefix = journal_prefix(&lines, index + 1);
                let before = Document::replay(&before_prefix)?;
                let after = Document::replay(&after_prefix)?;
                checkpoints.push(CinemaCheckpoint {
                    journal_line: index + 1,
                    author: metadata.author,
                    source: metadata.source,
                    intent: metadata.intent,
                    canonical_command: command.to_text(),
                    affected_requirements: metadata.affected_requirements,
                    evidence_hashes: metadata.evidence_hashes,
                    before_metrics: metrics(&before),
                    after_metrics: metrics(&after),
                    journal_prefix: after_prefix,
                    document_bytes: after.canonical_bytes(),
                });
            }
        }
        let final_document_bytes = Document::replay(jsonl)?.canonical_bytes();
        Ok(Self {
            checkpoints,
            final_document_bytes,
            story: Vec::new(),
        })
    }

    pub fn branch_from(
        &self,
        checkpoint: usize,
        command: Command,
        metadata: DecisionMetadata,
    ) -> Result<Document, String> {
        let selected = self
            .checkpoints
            .get(checkpoint)
            .ok_or_else(|| format!("checkpoint {checkpoint} does not exist"))?;
        let mut branch = Document::replay(&selected.journal_prefix)?;
        branch.dispatch_with_metadata(command, metadata)?;
        Ok(branch)
    }
}

fn journal_prefix(lines: &[&str], end_exclusive: usize) -> String {
    let mut prefix = lines[..end_exclusive].join("\n");
    prefix.push('\n');
    prefix
}

fn metrics(document: &Document) -> Value {
    json!({
        "cd": document.design.cd,
        "motor_designation": document.design.motor_designation,
        "part_count": count_parts(&document.vehicle.parts),
        "study_count": document.studies.len(),
        "telemetry_source_count": document.telemetry.len(),
    })
}

fn count_parts(parts: &[ascent_domain::vehicle::Part]) -> usize {
    parts
        .iter()
        .map(|part| 1 + count_parts(&part.children))
        .sum()
}
