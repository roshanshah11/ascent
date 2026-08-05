//! One-command, self-contained export of the executed NDRT 2020 comparison.
//!
//! This is the user-facing face of [`crate::ndrt_2020`] — it adds no physics,
//! no new case, and no tolerance of its own. It loads the canonical checked-in
//! case, verifies every consumed source against its pinned hash, runs the
//! authoritative simulator through [`ndrt_2020::run_comparison`], and then
//! **verifies that the freshly recomputed [`ComparisonArtifact`] is byte-for-byte
//! the artifact attached to the case**. Any integrity, execution, or
//! artifact-match failure is an error — never a quietly downgraded result.
//!
//! The rendered files are pure functions of (case JSON, telemetry CSV, motor
//! `.eng`) plus one explicitly allowed timestamp, which appears in
//! `manifest.json` and nowhere else. Two runs of the same sources into two
//! clean directories therefore produce identical evidence apart from that
//! single field.
//!
//! The export never promotes the evidence label. It reports the case's declared
//! rung (`flight_data_available`) and its credibility caveats verbatim.

use std::fmt::Write as _;

use ascent_domain::evidence::EvidenceLevel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ndrt_2020::{self, NdrtComparison, CASE_ID};
use crate::validation::{ComparedMetric, ComparisonArtifact, MetricKind, ValidationCase};

/// Selector a user types on the command line for this comparison.
pub const CASE_SELECTOR: &str = "ndrt-2020";

/// The canonical checked-in case, embedded so the command is self-contained and
/// cannot be pointed at a different, unpinned specification.
pub const CANONICAL_CASE_JSON: &[u8] =
    include_bytes!("../../../data/validation/cases/ndrt-2020-flight.json");

/// The measured RAVEN telemetry, embedded from the hash-pinned fixture.
pub const CANONICAL_TELEMETRY_CSV: &[u8] =
    include_bytes!("../../../data/validation/ndrt-2020/fullscale_2-23_raven1_crop.csv");

/// The flight motor's RASP `.eng` source, embedded from the pinned fixture.
pub const CANONICAL_MOTOR_ENG: &[u8] =
    include_bytes!("../../../data/validation/ndrt-2020/Cesaroni_4895L1395-P.eng");

/// Repository-relative path of the motor source, for the provenance manifest.
pub const MOTOR_ENG_PATH: &str = "data/validation/ndrt-2020/Cesaroni_4895L1395-P.eng";

/// SHA-256 pin for the motor `.eng`. The validation case pins the *measured*
/// fixture (the CSV) itself; the motor is the second consumed source and is
/// pinned here, matching `docs/validation/NDRT_2020_SOURCE_SPEC.md`.
pub const MOTOR_ENG_SHA256: &str =
    "d60b9e6f8d4bb4621a58e98873f69bfa8ef49efe4b0f78fdf345aa52c50b47ce";

/// Repository-relative path of the canonical case specification.
pub const CASE_PATH: &str = "data/validation/cases/ndrt-2020-flight.json";

/// The upstream RocketPaper source packet, exactly as pinned in
/// `docs/validation/NDRT_2020_SOURCE_SPEC.md`: `(path, sha256, role)`.
///
/// Six files are pinned; only two of them — the telemetry CSV and the motor
/// `.eng` — are *consumed* as simulation inputs. The rest are provenance and
/// units references (and, in the case of `env_23.nc`, an atmosphere Ascent
/// cannot read). The distinction matters: this command re-verifies the bytes it
/// actually consumes, and reports the remaining packet members as declared pins
/// carried from the source spec rather than as checks it performed.
pub const SOURCE_PACKET: [(&str, &str, &str); 6] = [
    (
        "data/validation/ndrt-2020/fullscale_2-23_raven1_crop.csv",
        "1cc15862ddb3367d09225037cfe8dd1f68687d1d2292ef7ecd5f2a344736d3f6",
        "measured RAVEN altimeter telemetry (the comparison fixture)",
    ),
    (
        MOTOR_ENG_PATH,
        MOTOR_ENG_SHA256,
        "RASP thrust curve (motor input)",
    ),
    (
        "data/validation/ndrt-2020/NDRT_ROCKETPY.ipynb",
        "e01c71b8f9307f94a66a61ab27dbb9fbc7140f2f26af5795035d4dde1f752153",
        "upstream RocketPy analysis (units / apogee-row reference)",
    ),
    (
        "data/validation/ndrt-2020/README.md",
        "bfe65d7f28186bad1607afbd83d25949f81aa54d0dc968a4703727b950d38615",
        "channel description; states apogee is at CSV row 343",
    ),
    (
        "data/validation/ndrt-2020/env_23.nc",
        "c6c378c762b3bb5200f604d4c176ab24c66a4e040531acb194d7efbc217d07c2",
        "ERA5 atmosphere sounding (not consumable by Ascent)",
    ),
    (
        "data/validation/ndrt-2020/ROCKETPAPER_LICENSE.txt",
        "37954555faa0cd7bec1fc4b05fd51cc40abe97294c387d0b1890019cbf76ec3b",
        "upstream MIT license",
    ),
];

/// File names written into the export directory. Fixed, so a reader (and a
/// test) always knows where each piece of evidence lives.
pub const ARTIFACT_FILE: &str = "comparison-artifact.json";
pub const SUMMARY_FILE: &str = "SUMMARY.md";
pub const MANIFEST_FILE: &str = "manifest.json";
pub const CASE_FILE: &str = "validation-case.json";

/// One rendered file of the export directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFile {
    pub name: &'static str,
    pub bytes: Vec<u8>,
}

/// A source the comparison consumed, with the hash it was verified against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRecord {
    /// Repository-relative path of the source.
    pub path: String,
    pub sha256: String,
    pub role: String,
}

/// One member of the pinned upstream source packet.
///
/// `consumed_and_verified` separates the two very different claims this export
/// makes: a file the run actually read and re-hashed, versus a file whose pin is
/// carried verbatim from `docs/validation/NDRT_2020_SOURCE_SPEC.md` and was
/// **not** re-checked here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PacketRecord {
    pub path: String,
    pub sha256: String,
    pub role: String,
    pub consumed_and_verified: bool,
}

/// Serializable metric result, split out so `manifest.json` carries the
/// pass/fail table without a reader having to re-derive it from the artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricRecord {
    pub id: String,
    pub unit: String,
    pub role: String,
    pub measured: f64,
    pub simulated: f64,
    pub abs_error: f64,
    /// Relative error, or `null` where there is no baseline to be relative to.
    ///
    /// `normalized_altitude_rmse` is measured against a zero baseline — it is
    /// already a dimensionless normalized ratio, so a relative error against
    /// zero is meaningless and is reported as absent rather than as `0`. The
    /// stored artifact value is untouched; this is presentation only.
    pub rel_error: Option<f64>,
    pub tolerance: f64,
    pub pass: bool,
}

/// Whether a relative error is meaningful for this metric. It is not when the
/// measured baseline is zero, i.e. the metric is already a normalized ratio.
fn displayable_rel_error(metric: &ComparedMetric) -> Option<f64> {
    (metric.measured != 0.0).then_some(metric.rel_error)
}

impl MetricRecord {
    fn of(metric: &ComparedMetric) -> Self {
        Self {
            id: metric.id.clone(),
            unit: metric.unit.clone(),
            role: match metric.kind {
                MetricKind::Primary => "primary_flight_metric",
                MetricKind::InputConsistency => "input_consistency_check",
            }
            .to_string(),
            measured: metric.measured,
            simulated: metric.simulated,
            abs_error: metric.abs_error,
            rel_error: displayable_rel_error(metric),
            tolerance: metric.tolerance,
            pass: metric.pass,
        }
    }
}

/// The provenance manifest written as `manifest.json`. The only non-deterministic
/// field in the whole export is `generated_at_unix_s`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportManifest {
    pub schema_version: u16,
    /// Unix seconds at export time. The single explicitly allowed source of
    /// run-to-run variation in this directory.
    pub generated_at_unix_s: u64,
    pub command: String,
    pub case_id: String,
    pub case_path: String,
    /// Spec-only hash of the case the comparison was run against.
    pub case_hash: String,
    /// SHA-256 of the canonical case file as loaded (spec plus attached artifact).
    pub case_file_sha256: String,
    /// The case's declared evidence rung, reported verbatim. The export never
    /// promotes it.
    pub evidence_level: String,
    pub model_version: String,
    /// SHA-256 of the canonical simulation input that produced the outputs.
    pub input_hash: String,
    /// Every file of the pinned upstream source packet, consumed or not.
    pub source_packet: Vec<PacketRecord>,
    /// How many files the upstream packet pins in total (`source_packet.len()`).
    pub source_packet_files_pinned: usize,
    /// How many files this run actually consumed and hash-verified. Strictly
    /// smaller than the packet: the packet also carries provenance-only files,
    /// and this run additionally verifies the canonical case, which is not a
    /// packet member.
    pub simulation_inputs_verified: usize,
    /// The files this run consumed and verified — the `simulation_inputs_verified`
    /// set, with the hash each was checked against.
    pub sources: Vec<SourceRecord>,
    /// Named hashes the artifact itself records, for cross-checking.
    pub artifact_source_hashes: std::collections::BTreeMap<String, String>,
    /// True iff the freshly recomputed artifact equals the one attached to the
    /// canonical case. Always true in a successful export — a mismatch is an
    /// error, and this field records the check having been made.
    pub artifact_matches_canonical: bool,
    pub overall_pass: bool,
    pub primary_metrics_passed: usize,
    pub primary_metrics_total: usize,
    pub input_checks_passed: usize,
    pub input_checks_total: usize,
    pub metrics: Vec<MetricRecord>,
    pub timestep_s: f64,
    pub compared_samples: usize,
    pub credibility_caveats: Vec<String>,
    pub assumptions_and_limitations: Vec<String>,
    pub known_mismatches: Vec<String>,
    pub validity_domain: Vec<String>,
    pub uncertainty: Vec<String>,
}

/// Everything a successful export produced: the rendered files, the executed
/// comparison behind them, and a concise terminal summary.
#[derive(Debug, Clone)]
pub struct FlightComparisonExport {
    pub comparison: NdrtComparison,
    pub manifest: ExportManifest,
    pub files: Vec<ExportFile>,
    pub terminal_summary: String,
}

impl FlightComparisonExport {
    /// Overall flight-validation result, from the primary metrics only.
    pub fn pass(&self) -> bool {
        self.comparison.artifact.pass
    }

    /// Look up a rendered file by name.
    pub fn file(&self, name: &str) -> Option<&ExportFile> {
        self.files.iter().find(|file| file.name == name)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The serde wire name of an evidence rung (`flight_data_available`), so the
/// export reports exactly the token the case file carries.
fn evidence_label(level: &EvidenceLevel) -> String {
    serde_json::to_value(level)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{level:?}"))
}

/// Describe how a freshly recomputed artifact differs from the attached one, in
/// terms a reader can act on. Only called when the two disagree.
fn describe_artifact_mismatch(fresh: &ComparisonArtifact, attached: &ComparisonArtifact) -> String {
    let mut differences: Vec<String> = Vec::new();
    let mut note = |field: &str, fresh: String, attached: String| {
        if fresh != attached {
            differences.push(format!("{field}: recomputed {fresh}, attached {attached}"));
        }
    };
    note(
        "model_version",
        fresh.model_version.clone(),
        attached.model_version.clone(),
    );
    note(
        "case_hash",
        fresh.case_hash.clone(),
        attached.case_hash.clone(),
    );
    note(
        "input_hash",
        fresh.input_hash.clone(),
        attached.input_hash.clone(),
    );
    note("pass", fresh.pass.to_string(), attached.pass.to_string());
    for (name, hash) in &fresh.source_hashes {
        let attached_hash = attached
            .source_hashes
            .get(name)
            .cloned()
            .unwrap_or_else(|| "<absent>".to_string());
        note(
            &format!("source_hashes.{name}"),
            hash.clone(),
            attached_hash,
        );
    }
    for metric in &fresh.metrics {
        match attached.metrics.iter().find(|m| m.id == metric.id) {
            Some(other) if other == metric => {}
            Some(other) => differences.push(format!(
                "metric {}: recomputed simulated={} abs_error={} pass={}, \
                 attached simulated={} abs_error={} pass={}",
                metric.id,
                metric.simulated,
                metric.abs_error,
                metric.pass,
                other.simulated,
                other.abs_error,
                other.pass
            )),
            None => differences.push(format!("metric {}: absent from attached", metric.id)),
        }
    }
    for metric in &attached.metrics {
        if !fresh.metrics.iter().any(|m| m.id == metric.id) {
            differences.push(format!("metric {}: absent from recomputed", metric.id));
        }
    }
    if differences.is_empty() {
        differences.push("artifacts differ in a field not itemized above".to_string());
    }
    differences.join("; ")
}

/// Run the canonical export against the embedded, hash-pinned sources.
pub fn run_canonical_export(generated_at_unix_s: u64) -> Result<FlightComparisonExport, String> {
    run_export(
        CANONICAL_CASE_JSON,
        CANONICAL_TELEMETRY_CSV,
        CANONICAL_MOTOR_ENG,
        generated_at_unix_s,
    )
}

/// Load, verify, execute, recompute, cross-check, and render.
///
/// Fails — never downgrades — on a case that will not load under the evidence
/// policy, a case that is not the NDRT case, a source whose bytes do not match
/// its pinned hash, a simulator or comparison error, a missing attached
/// artifact, or a recomputed artifact that disagrees with the attached one.
pub fn run_export(
    case_json: &[u8],
    telemetry_csv: &[u8],
    motor_eng: &[u8],
    generated_at_unix_s: u64,
) -> Result<FlightComparisonExport, String> {
    // 1. Load the canonical case. `from_canonical_bytes` enforces the schema,
    //    canonical encoding, and the evidence policy at the declared rung.
    let case = ValidationCase::from_canonical_bytes(case_json)
        .map_err(|error| format!("canonical case failed to load: {error}"))?;

    // 2. Case identity, then every consumed source against its pin.
    if case.case_id != CASE_ID {
        return Err(format!(
            "case identity mismatch: expected {CASE_ID}, got {}",
            case.case_id
        ));
    }
    case.verify_fixture_bytes(telemetry_csv)
        .map_err(|error| format!("source integrity failure: {error}"))?;
    let eng_hash = sha256_hex(motor_eng);
    if eng_hash != MOTOR_ENG_SHA256 {
        return Err(format!(
            "source integrity failure: hash mismatch for {MOTOR_ENG_PATH}: \
             expected {MOTOR_ENG_SHA256}, got {eng_hash}"
        ));
    }
    let case_hash = case.case_hash()?;

    // 3-4. Execute the authoritative simulator and recompute the artifact.
    let comparison = ndrt_2020::run_comparison(telemetry_csv, motor_eng, &case_hash)
        .map_err(|error| format!("comparison execution failed: {error}"))?;
    let artifact = &comparison.artifact;
    artifact
        .validate()
        .map_err(|error| format!("recomputed artifact is invalid: {error}"))?;

    // 5. Cross-check against the artifact attached to the canonical case.
    let attached = case.comparison.as_ref().ok_or_else(|| {
        format!(
            "canonical case {CASE_ID} carries no attached comparison artifact to verify against"
        )
    })?;
    if attached != artifact {
        return Err(format!(
            "artifact validation failure: recomputed comparison does not match the \
             artifact attached to {CASE_PATH} — {}",
            describe_artifact_mismatch(artifact, attached)
        ));
    }

    // 6. Render the self-contained result directory.
    let (primary_passed, primary_total) = comparison.primary_metrics();
    let (checks_passed, checks_total) = comparison.input_checks();
    let evidence_level = evidence_label(&case.evidence_level);

    // The files this run read and re-hashed. Two are packet members; the third,
    // the canonical case, is Ascent's own spec and is not part of the upstream
    // packet — hence the two counts never coincide.
    let verified_sources = vec![
        SourceRecord {
            path: case.fixture.path.clone(),
            sha256: case.fixture.sha256.clone(),
            role: format!(
                "measured telemetry — {} ({}, upstream {})",
                case.fixture.source_url, case.fixture.license, case.fixture.upstream_revision
            ),
        },
        SourceRecord {
            path: MOTOR_ENG_PATH.to_string(),
            sha256: eng_hash,
            role: "imported RASP motor thrust curve".to_string(),
        },
        SourceRecord {
            path: CASE_PATH.to_string(),
            sha256: sha256_hex(case_json),
            role: "canonical validation case (spec plus attached artifact)".to_string(),
        },
    ];
    let source_packet: Vec<PacketRecord> = SOURCE_PACKET
        .iter()
        .map(|(path, sha256, role)| PacketRecord {
            path: (*path).to_string(),
            sha256: (*sha256).to_string(),
            role: (*role).to_string(),
            consumed_and_verified: verified_sources.iter().any(|source| source.path == *path),
        })
        .collect();

    let manifest = ExportManifest {
        schema_version: ascent_domain::evidence::EVIDENCE_SCHEMA_VERSION,
        generated_at_unix_s,
        command: format!("ascent-cli compare-flight {CASE_SELECTOR} --output <directory>"),
        case_id: case.case_id.clone(),
        case_path: CASE_PATH.to_string(),
        case_hash: case_hash.clone(),
        case_file_sha256: sha256_hex(case_json),
        evidence_level: evidence_level.clone(),
        model_version: artifact.model_version.clone(),
        input_hash: artifact.input_hash.clone(),
        source_packet_files_pinned: source_packet.len(),
        simulation_inputs_verified: verified_sources.len(),
        source_packet,
        sources: verified_sources,
        artifact_source_hashes: artifact.source_hashes.clone(),
        artifact_matches_canonical: true,
        overall_pass: artifact.pass,
        primary_metrics_passed: primary_passed,
        primary_metrics_total: primary_total,
        input_checks_passed: checks_passed,
        input_checks_total: checks_total,
        metrics: artifact.metrics.iter().map(MetricRecord::of).collect(),
        timestep_s: comparison.timestep_s,
        compared_samples: comparison.compared_samples,
        credibility_caveats: case.caveats.clone(),
        assumptions_and_limitations: artifact.known_limitations.clone(),
        known_mismatches: case.known_mismatches.clone(),
        validity_domain: case.validity_domain.clone(),
        uncertainty: case.uncertainty.clone(),
    };

    let mut artifact_bytes = serde_json::to_vec_pretty(artifact).map_err(|e| e.to_string())?;
    artifact_bytes.push(b'\n');
    let mut manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    manifest_bytes.push(b'\n');

    let files = vec![
        ExportFile {
            name: ARTIFACT_FILE,
            bytes: artifact_bytes,
        },
        ExportFile {
            name: SUMMARY_FILE,
            bytes: render_summary(&case, &comparison, &manifest).into_bytes(),
        },
        ExportFile {
            name: MANIFEST_FILE,
            bytes: manifest_bytes,
        },
        ExportFile {
            name: CASE_FILE,
            bytes: case_json.to_vec(),
        },
    ];

    let terminal_summary = render_terminal_summary(&comparison, &manifest);

    Ok(FlightComparisonExport {
        comparison,
        manifest,
        files,
        terminal_summary,
    })
}

/// Fixed numeric formatting, shared by the Markdown summary and its test, so
/// "the summary matches the artifact" is a mechanical check rather than a
/// judgement call.
fn fmt_value(value: f64) -> String {
    format!("{value:.6}")
}

fn fmt_pct(value: f64) -> String {
    format!("{:.3} %", value * 100.0)
}

/// Relative error for display. A metric measured against a zero baseline is
/// already a normalized ratio, so there is no relative error to show and the
/// cell reads `N/A` rather than a misleading `0.000 %`.
fn fmt_rel_error(metric: &ComparedMetric) -> String {
    match displayable_rel_error(metric) {
        Some(rel_error) => fmt_pct(rel_error),
        None => "N/A".to_string(),
    }
}

fn verdict(pass: bool) -> &'static str {
    if pass {
        "PASS"
    } else {
        "FAIL"
    }
}

fn metric_rows(out: &mut String, artifact: &ComparisonArtifact, kind: MetricKind) {
    out.push_str(
        "| Metric | Unit | Measured | Simulated | Abs error | Rel error | Tolerance | Result |\n\
         |--------|------|----------|-----------|-----------|-----------|-----------|--------|\n",
    );
    for metric in artifact.metrics.iter().filter(|m| m.kind == kind) {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {} | {} | {} | **{}** |",
            metric.id,
            metric.unit,
            fmt_value(metric.measured),
            fmt_value(metric.simulated),
            fmt_value(metric.abs_error),
            fmt_rel_error(metric),
            fmt_value(metric.tolerance),
            verdict(metric.pass),
        );
    }
}

fn bullets(out: &mut String, items: &[String]) {
    if items.is_empty() {
        out.push_str("_None recorded._\n");
        return;
    }
    for item in items {
        let _ = writeln!(out, "- {item}");
    }
}

/// Render the human-readable report. Every number comes from the artifact, and
/// nothing here varies between runs — the timestamp lives in `manifest.json`.
fn render_summary(
    case: &ValidationCase,
    comparison: &NdrtComparison,
    manifest: &ExportManifest,
) -> String {
    let artifact = &comparison.artifact;
    let mut out = String::new();

    let _ = writeln!(out, "# {}\n", case.title);
    let _ = writeln!(
        out,
        "Executed, non-calibrated comparison produced by \
         `ascent-cli compare-flight {CASE_SELECTOR}`. It re-ran Ascent's authoritative \
         simulator (`{}`) against the hash-pinned measured flight and verified the result \
         against the artifact attached to `{}`.\n",
        artifact.model_version, CASE_PATH
    );

    let _ = writeln!(
        out,
        "## Result\n\n**Overall: {}** — {}/{} primary flight metrics passed. \
         Input-consistency checks ({}/{} passed) are reported separately and never gate \
         this result.\n",
        verdict(artifact.pass),
        manifest.primary_metrics_passed,
        manifest.primary_metrics_total,
        manifest.input_checks_passed,
        manifest.input_checks_total,
    );

    out.push_str(
        "## Identity, configuration, and hashes\n\n| Field | Value |\n|-------|-------|\n",
    );
    let _ = writeln!(out, "| Case id | `{}` |", manifest.case_id);
    let _ = writeln!(out, "| Case spec hash | `{}` |", manifest.case_hash);
    let _ = writeln!(
        out,
        "| Case file SHA-256 | `{}` |",
        manifest.case_file_sha256
    );
    let _ = writeln!(out, "| Model version | `{}` |", manifest.model_version);
    let _ = writeln!(out, "| Config / input hash | `{}` |", manifest.input_hash);
    let _ = writeln!(out, "| Simulation timestep | {} s |", manifest.timestep_s);
    let _ = writeln!(
        out,
        "| Compared samples (launch→apogee) | {} |",
        manifest.compared_samples
    );
    let _ = writeln!(
        out,
        "| Recomputed artifact matches canonical | {} |\n",
        manifest.artifact_matches_canonical
    );

    let _ = writeln!(
        out,
        "## Source provenance\n\n\
         Two counts, deliberately kept apart:\n\n\
         - **Full source packet pinned: {} files.** Every file of the upstream NDRT packet \
           carries a SHA-256 in `docs/validation/NDRT_2020_SOURCE_SPEC.md`. Most are \
           provenance and units references; `env_23.nc` cannot be read by Ascent at all.\n\
         - **Simulation inputs consumed and verified: {} files.** Only these were read and \
           re-hashed by this run.\n\n\
         The pins for packet files this run did not consume are carried verbatim from the \
         source spec and were **not** re-verified here.\n",
        manifest.source_packet_files_pinned, manifest.simulation_inputs_verified
    );

    let _ = writeln!(
        out,
        "### Simulation inputs consumed and verified ({})\n",
        manifest.simulation_inputs_verified
    );
    out.push_str("| Source | SHA-256 | Role |\n|--------|---------|------|\n");
    for source in &manifest.sources {
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} |",
            source.path, source.sha256, source.role
        );
    }
    out.push('\n');

    let _ = writeln!(
        out,
        "### Full source packet pinned ({})\n",
        manifest.source_packet_files_pinned
    );
    out.push_str(
        "| File | SHA-256 | Role | Consumed and verified this run |\n\
         |------|---------|------|-------------------------------|\n",
    );
    for entry in &manifest.source_packet {
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} | {} |",
            entry.path,
            entry.sha256,
            entry.role,
            if entry.consumed_and_verified {
                "yes"
            } else {
                "no — declared pin"
            }
        );
    }
    out.push('\n');
    out.push_str("Hashes the artifact itself records:\n\n| Name | SHA-256 |\n|------|---------|\n");
    for (name, hash) in &manifest.artifact_source_hashes {
        let _ = writeln!(out, "| `{name}` | `{hash}` |");
    }
    out.push('\n');

    let _ = writeln!(
        out,
        "## Primary flight metrics — {}/{} pass\n\n\
         These three metrics, and only these, determine the overall result.\n",
        manifest.primary_metrics_passed, manifest.primary_metrics_total
    );
    metric_rows(&mut out, artifact, MetricKind::Primary);
    let _ = writeln!(
        out,
        "\n`normalized_altitude_rmse` is already a dimensionless normalized ratio — the \
         launch→apogee RMSE divided by the measured apogee — so there is no baseline for a \
         relative error and none is reported:\n\n\
         - normalized RMSE: {}\n\
         - threshold: {}\n\
         - relative error: N/A\n",
        fmt_pct(comparison.normalized_rmse),
        fmt_pct(
            artifact
                .metrics
                .iter()
                .find(|m| m.id == "normalized_altitude_rmse")
                .map(|m| m.tolerance)
                .unwrap_or(f64::NAN)
        ),
    );
    out.push('\n');

    let _ = writeln!(
        out,
        "## Input-consistency checks — {}/{} pass\n\n\
         Self-checks on the imported inputs. Verified and reported, but **excluded** from \
         the flight-validation pass/fail: an input-consistency check can neither force nor \
         block the overall result.\n",
        manifest.input_checks_passed, manifest.input_checks_total
    );
    metric_rows(&mut out, artifact, MetricKind::InputConsistency);
    out.push('\n');

    out.push_str("## Measured versus simulated\n\n| Quantity | Measured | Simulated |\n|----------|----------|-----------|\n");
    let _ = writeln!(
        out,
        "| Apogee (m AGL) | {} | {} |",
        fmt_value(comparison.measured_apogee_agl_m),
        fmt_value(comparison.simulated_apogee_agl_m)
    );
    let _ = writeln!(
        out,
        "| Time to apogee (s) | {} | {} |",
        fmt_value(comparison.measured_time_to_apogee_s),
        fmt_value(comparison.simulated_time_to_apogee_s)
    );
    let _ = writeln!(
        out,
        "| Burnout (s) | {} (imported motor) | {} |",
        fmt_value(comparison.motor_burn_time_s),
        fmt_value(comparison.simulated_burnout_time_s)
    );
    let _ = writeln!(
        out,
        "| Altitude RMSE, launch→apogee (m) | — | {} ({} of measured apogee) |\n",
        fmt_value(comparison.rmse_m),
        fmt_pct(comparison.normalized_rmse)
    );

    let _ = writeln!(
        out,
        "## Evidence label\n\n\
         Declared evidence level: **`{}`**. This export reports the label the case \
         declares; running the comparison does not promote it, and a passing artifact does \
         not by itself grant `flight_validated`.\n",
        manifest.evidence_level
    );
    out.push_str("### Credibility caveats\n\n");
    bullets(&mut out, &manifest.credibility_caveats);
    out.push('\n');

    out.push_str("## Assumptions and limitations\n\n");
    bullets(&mut out, &manifest.assumptions_and_limitations);
    out.push_str("\n### Known mismatches\n\n");
    bullets(&mut out, &manifest.known_mismatches);
    out.push_str("\n### Validity domain\n\n");
    bullets(&mut out, &manifest.validity_domain);
    out.push_str("\n### Uncertainty\n\n");
    bullets(&mut out, &manifest.uncertainty);

    out
}

/// The concise terminal report. Same facts, one screen.
fn render_terminal_summary(comparison: &NdrtComparison, manifest: &ExportManifest) -> String {
    let artifact = &comparison.artifact;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} — executed comparison: {}",
        manifest.case_id,
        verdict(artifact.pass)
    );
    let _ = writeln!(
        out,
        "  evidence level      {} (not promoted by this run)",
        manifest.evidence_level
    );
    let _ = writeln!(out, "  model               {}", manifest.model_version);
    let _ = writeln!(out, "  case hash           {}", manifest.case_hash);
    let _ = writeln!(out, "  config/input hash   {}", manifest.input_hash);
    let _ = writeln!(
        out,
        "  full source packet pinned: {} files (declared pins, NDRT_2020_SOURCE_SPEC.md)",
        manifest.source_packet_files_pinned
    );
    let _ = writeln!(
        out,
        "  simulation inputs consumed and verified: {} files (hashes matched their pins)",
        manifest.simulation_inputs_verified
    );
    let _ = writeln!(
        out,
        "  artifact match      recomputed comparison equals the canonical attached artifact"
    );
    let _ = writeln!(
        out,
        "\n  primary flight metrics — {}/{} pass (these decide the result)",
        manifest.primary_metrics_passed, manifest.primary_metrics_total
    );
    for metric in artifact
        .metrics
        .iter()
        .filter(|m| m.kind == MetricKind::Primary)
    {
        let _ = writeln!(
            out,
            "    [{}] {:<26} abs {:>12.6} {:<8} tol {:.6}",
            verdict(metric.pass),
            metric.id,
            metric.abs_error,
            metric.unit,
            metric.tolerance
        );
    }
    let _ = writeln!(
        out,
        "\n  input-consistency checks — {}/{} pass (reported, never gating)",
        manifest.input_checks_passed, manifest.input_checks_total
    );
    for metric in artifact
        .metrics
        .iter()
        .filter(|m| m.kind == MetricKind::InputConsistency)
    {
        let _ = writeln!(
            out,
            "    [{}] {:<26} abs {:>12.6} {:<8} tol {:.6}",
            verdict(metric.pass),
            metric.id,
            metric.abs_error,
            metric.unit,
            metric.tolerance
        );
    }
    out.push_str("\n  credibility caveats\n");
    for caveat in &manifest.credibility_caveats {
        let _ = writeln!(out, "    - {caveat}");
    }
    out
}
