use ascent_domain::Motor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::rocket::{Environment, Rocket};
use crate::sim::{SimConfig, SimResult};

/// Machine-readable, deterministic run summary — the Day-1 proof artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimSummary {
    pub rocket: String,
    pub motor: String,
    pub apogee_m: f64,
    pub apogee_time_s: f64,
    pub burnout_time_s: f64,
    pub burnout_velocity_ms: f64,
    pub max_velocity_ms: f64,
    pub liftoff_time_s: f64,
    pub timestep_s: f64,
    pub steps: u64,
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
            liftoff_time_s: result.liftoff_time_s,
            timestep_s: config.dt_s,
            steps: result.steps,
            input_hash: input_hash(rocket, motor, env, config),
            sim_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
