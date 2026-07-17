use ascent_domain::Motor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::rocket::{Environment, Rocket};
use crate::sim::{ConvergenceReport, SimConfig, SimResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSummary {
    pub kind: String,
    pub t_s: f64,
    pub altitude_m: f64,
    pub velocity_ms: f64,
}

/// Machine-readable, deterministic run summary — the proof artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimSummary {
    pub rocket: String,
    pub motor: String,
    pub apogee_m: f64,
    pub apogee_time_s: f64,
    pub burnout_time_s: f64,
    pub burnout_velocity_ms: f64,
    pub max_velocity_ms: f64,
    pub rail_exit_velocity_ms: f64,
    pub landing_time_s: f64,
    pub landing_velocity_ms: f64,
    pub events: Vec<EventSummary>,
    pub timestep_s: f64,
    pub steps: u64,
    /// Halving the timestep moved apogee by less than max(0.5 m, 0.1%).
    pub convergence: Option<ConvergenceReport>,
    /// SHA-256 over the canonical JSON of (rocket, motor, environment, config).
    pub input_hash: String,
    pub sim_version: String,
}

/// SHA-256 of the canonical serialized inputs. Same inputs → same hash.
pub fn input_hash(
    rocket: &Rocket,
    motor: &Motor,
    env: &Environment,
    config: &SimConfig,
) -> String {
    let canonical = serde_json::json!({
        "rocket": rocket,
        "motor": motor,
        "environment": env,
        "config": config,
    });
    let bytes = serde_json::to_vec(&canonical).expect("inputs serialize");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

impl SimSummary {
    pub fn from_result(
        result: &SimResult,
        rocket: &Rocket,
        motor: &Motor,
        env: &Environment,
        config: &SimConfig,
    ) -> Self {
        Self {
            rocket: rocket.name.clone(),
            motor: motor.designation.clone(),
            apogee_m: result.apogee_m,
            apogee_time_s: result.apogee_time_s,
            burnout_time_s: result.burnout_time_s,
            burnout_velocity_ms: result.burnout_velocity_ms,
            max_velocity_ms: result.max_velocity_ms,
            rail_exit_velocity_ms: result.rail_exit_velocity_ms,
            landing_time_s: result.landing_time_s,
            landing_velocity_ms: result.landing_velocity_ms,
            events: result
                .events
                .iter()
                .map(|e| EventSummary {
                    kind: format!("{:?}", e.kind),
                    t_s: e.t,
                    altitude_m: e.altitude_m,
                    velocity_ms: e.velocity_ms,
                })
                .collect(),
            timestep_s: config.dt_s,
            steps: result.steps,
            convergence: None,
            input_hash: input_hash(rocket, motor, env, config),
            sim_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn with_convergence(mut self, report: ConvergenceReport) -> Self {
        self.convergence = Some(report);
        self
    }
}
