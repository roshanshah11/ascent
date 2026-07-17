//! Flight-rules constraint checks.
//!
//! Rules load from a rule-pack JSON (`data/rules/irec-2026.json`, produced
//! with per-rule citations — Codex task A1). Until that pack lands, a
//! built-in default set carries the values stated in the build plan
//! (docs/FULL_BUILD_RESEARCH.md §step-4), each marked with its provisional
//! source so no number floats free. Same schema either way: when the cited
//! pack arrives it replaces the defaults with zero code changes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    Gte,
    Lte,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub description: String,
    /// Which measured quantity this rule binds to.
    pub quantity: String,
    pub comparator: Comparator,
    pub value: f64,
    pub units: String,
    pub citation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulePack {
    pub name: String,
    pub rules: Vec<Rule>,
}

/// Measured quantities a rule can bind to. Extend as checks grow.
#[derive(Debug, Clone, Copy)]
pub struct FlightQuantities {
    pub rail_exit_velocity_ms: f64,
    pub min_stability_calibers: f64,
    pub fin_span_calibers: f64,
    /// IREC states stability bands as percent of rocket length.
    pub stability_pct_len_at_launch: f64,
    pub stability_pct_len_min: f64,
    pub stability_pct_len_max: f64,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub rule_id: String,
    pub description: String,
    pub citation: String,
    pub measured: f64,
    pub required: f64,
    pub comparator: Comparator,
    pub pass: bool,
}

impl RulePack {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Plan-stated defaults, used until the cited IREC 2026 pack lands.
    pub fn builtin_defaults() -> Self {
        let src = "docs/FULL_BUILD_RESEARCH.md#step-4 (provisional; replace with cited IREC 2026 pack, Codex task A1)";
        RulePack {
            name: "builtin-defaults".into(),
            rules: vec![
                Rule {
                    id: "rail-exit-velocity".into(),
                    description: "Minimum rail-exit velocity".into(),
                    quantity: "rail_exit_velocity_ms".into(),
                    comparator: Comparator::Gte,
                    value: 25.0,
                    units: "m/s".into(),
                    citation: src.into(),
                },
                Rule {
                    id: "stability-minimum".into(),
                    description: "Minimum static stability margin over the burn".into(),
                    quantity: "min_stability_calibers".into(),
                    comparator: Comparator::Gte,
                    value: 1.5,
                    units: "calibers".into(),
                    citation: src.into(),
                },
                Rule {
                    id: "fin-span-minimum".into(),
                    description: "Minimum fin span".into(),
                    quantity: "fin_span_calibers".into(),
                    comparator: Comparator::Gte,
                    value: 0.8,
                    units: "calibers".into(),
                    citation: src.into(),
                },
            ],
        }
    }

    /// Load the evaluable subset of the cited IREC 2026 rule pack
    /// (data/rules/irec-2026.json, Codex task A1). That pack's schema is
    /// richer than this engine (structured values, applicability categories,
    /// citation objects); this extracts only rules v0.1 can measure, mapped
    /// by explicit rule id, with the citation flattened to one line.
    /// Unmapped rules are skipped, never guessed at.
    pub fn from_irec_json(json: &str) -> Result<Self, serde_json::Error> {
        let raw: serde_json::Value = serde_json::from_str(json)?;
        // rule id -> quantity name this engine measures
        let id_map: &[(&str, &str)] = &[
            ("irec-2026-rail-departure-velocity-minimum", "rail_exit_velocity_ms"),
            ("irec-2026-fin-span-minimum", "fin_span_calibers"),
            ("irec-2026-stability-minimum-subsonic", "stability_pct_len_min"),
            ("irec-2026-stability-maximum-at-launch", "stability_pct_len_at_launch"),
            ("irec-2026-stability-maximum-through-flight", "stability_pct_len_max"),
        ];
        let mut rules = Vec::new();
        for rule in raw["rules"].as_array().into_iter().flatten() {
            let id = rule["id"].as_str().unwrap_or_default();
            let Some((_, quantity)) = id_map.iter().find(|(k, _)| *k == id) else {
                continue;
            };
            let comparator = match rule["comparator"].as_str() {
                Some("gte") => Comparator::Gte,
                Some("lte") => Comparator::Lte,
                _ => continue, // non-threshold comparator: not evaluable here
            };
            let Some(value) = rule["value"]["number"].as_f64() else {
                continue; // non-scalar value: not evaluable here
            };
            let citation = {
                let c = &rule["citation"];
                format!(
                    "{} {} §{}",
                    c["document_name"].as_str().unwrap_or("IREC 2026"),
                    c["version"].as_str().unwrap_or(""),
                    c["section"].as_str().unwrap_or("?")
                )
            };
            rules.push(Rule {
                id: id.to_string(),
                description: rule["description"].as_str().unwrap_or_default().to_string(),
                quantity: quantity.to_string(),
                comparator,
                value,
                units: rule["units"].as_str().unwrap_or_default().to_string(),
                citation,
            });
        }
        Ok(RulePack {
            name: raw["id"].as_str().unwrap_or("irec-2026").to_string(),
            rules,
        })
    }

    pub fn check(&self, q: &FlightQuantities) -> Vec<CheckResult> {
        self.rules
            .iter()
            .filter_map(|rule| {
                let measured = match rule.quantity.as_str() {
                    "rail_exit_velocity_ms" => q.rail_exit_velocity_ms,
                    "min_stability_calibers" => q.min_stability_calibers,
                    "fin_span_calibers" => q.fin_span_calibers,
                    "stability_pct_len_at_launch" => q.stability_pct_len_at_launch,
                    "stability_pct_len_min" => q.stability_pct_len_min,
                    "stability_pct_len_max" => q.stability_pct_len_max,
                    _ => return None, // quantity not yet measured by this engine
                };
                let pass = match rule.comparator {
                    Comparator::Gte => measured >= rule.value,
                    Comparator::Lte => measured <= rule.value,
                };
                Some(CheckResult {
                    rule_id: rule.id.clone(),
                    description: rule.description.clone(),
                    citation: rule.citation.clone(),
                    measured,
                    required: rule.value,
                    comparator: rule.comparator,
                    pass,
                })
            })
            .collect()
    }
}
