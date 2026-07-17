use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Liftoff,
    RailExit,
    Burnout,
    Apogee,
    RecoveryDeploy,
    Landing,
}

/// A flight event located by interpolating the crossing between two
/// integration steps (never just snapped to the step grid).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    pub t: f64,
    pub altitude_m: f64,
    pub velocity_ms: f64,
}
