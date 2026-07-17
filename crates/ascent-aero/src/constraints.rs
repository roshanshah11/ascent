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

    pub fn check(&self, q: &FlightQuantities) -> Vec<CheckResult> {
        self.rules
            .iter()
            .filter_map(|rule| {
                let measured = match rule.quantity.as_str() {
                    "rail_exit_velocity_ms" => q.rail_exit_velocity_ms,
                    "min_stability_calibers" => q.min_stability_calibers,
                    "fin_span_calibers" => q.fin_span_calibers,
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
