//! Studies (v0.3 Step 4): named analysis cases that live in the document.
//! A study captures *what question is being asked* of the design — one
//! flight, a dispersion, a motor trade, a stability sweep — plus the
//! engine and seed that answer it. Results carry the input hash of the
//! exact (vehicle, design, study config) that produced them, so staleness
//! is provable, not guessed: edit the rocket and every stored result that
//! no longer matches its inputs says so.

use crate::design::Design;
use ascent_domain::vehicle::Vehicle;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyId(pub u32);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StudyKind {
    SingleFlight,
    Dispersion { flights: u32 },
    MotorTrade { candidates: Vec<String> },
    StabilitySweep { param: String, from: f64, to: f64, steps: u32 },
}

/// A completed study's output. `data` is the engine's summary payload
/// (shape varies by kind); `input_hash` is the provenance anchor — the
/// content hash of the inputs at the moment the study ran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudyResults {
    pub input_hash: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Study {
    pub id: StudyId,
    pub name: String,
    pub kind: StudyKind,
    pub engine: String,
    pub seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub results: Option<StudyResults>,
}

/// The hash a fresh run of this study would stamp on its results: the
/// canonical content hash of everything the run depends on. Results are
/// current exactly when their stored hash equals this.
pub fn study_input_hash(vehicle: &Vehicle, design: &Design, study: &Study) -> String {
    ascent_sim::content_hash(&serde_json::json!({
        "vehicle": vehicle,
        "design": design,
        "kind": study.kind,
        "engine": study.engine,
        "seed": study.seed,
    }))
}

impl Study {
    /// True when stored results exist but no longer match the inputs.
    /// A study with no results is "not run", not stale.
    pub fn is_stale(&self, vehicle: &Vehicle, design: &Design) -> bool {
        match &self.results {
            None => false,
            Some(r) => r.input_hash != study_input_hash(vehicle, design, self),
        }
    }
}
