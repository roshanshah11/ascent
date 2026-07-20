use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Liftoff,
    RailExit,
    Burnout,
    StageSeparation,
    StageIgnition,
    Apogee,
    RecoveryDeploy,
    /// Dual-deploy main opening (descending through the configured
    /// altitude, or at apogee when apogee is already below it).
    /// `RecoveryDeploy` remains the apogee-side deployment event.
    MainDeploy,
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
