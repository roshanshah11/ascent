//! Optional RocketPy cross-validation bridge.
//!
//! This module is deliberately isolated behind `bridge-rocketpy`: native
//! simulation never invokes Python and is still the authoritative result.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ascent_domain::Motor;
use serde::{Deserialize, Serialize};

use crate::engine::SimEngine;
use crate::rocket::{DragModel, Environment, Recovery, Rocket};
use crate::sim::SimConfig;
use crate::summary::{input_hash, EventSummary, SimSummary};

const REQUEST_SCHEMA: &str = "ascent-rocketpy-request-v1";
const RESPONSE_SCHEMA: &str = "ascent-rocketpy-response-v1";

/// Injectable process configuration. The default is only for a developer who
/// deliberately enables this Cargo feature; it is not used by native runs.
#[derive(Debug, Clone)]
pub struct RocketPyProcess {
    executable: PathBuf,
    script: PathBuf,
}

impl RocketPyProcess {
    pub fn new(executable: impl Into<PathBuf>, script: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            script: script.into(),
        }
    }

    fn from_environment() -> Self {
        let executable = std::env::var_os("ASCENT_ROCKETPY_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("python3"));
        let script = std::env::var_os("ASCENT_ROCKETPY_SCRIPT")
            .map(PathBuf::from)
            .unwrap_or_else(default_script);
        Self::new(executable, script)
    }
}

fn default_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("bridges/rocketpy/run_ascent.py")
}

/// RocketPy sidecar engine. It is a comparison engine only, never a fallback
/// for `NativeEngine` and never a source of inputs for native simulation.
#[derive(Debug, Clone)]
pub struct RocketPyEngine {
    process: RocketPyProcess,
}

impl RocketPyEngine {
    pub fn from_environment() -> Self {
        Self {
            process: RocketPyProcess::from_environment(),
        }
    }

    pub fn with_process(executable: impl Into<PathBuf>, script: impl Into<PathBuf>) -> Self {
        Self {
            process: RocketPyProcess::new(executable, script),
        }
    }

    fn invoke(&self, request: &BridgeRequest<'_>) -> Result<BridgeResponse, String> {
        if !self.process.script.is_file() {
            return Err(format!(
                "RocketPy bridge script is unavailable: {}",
                self.process.script.display()
            ));
        }
        let payload = serde_json::to_vec(request)
            .map_err(|error| format!("failed to serialize RocketPy bridge request: {error}"))?;
        let mut child = Command::new(&self.process.executable)
            .arg(&self.process.script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start RocketPy bridge: {error}"))?;
        child
            .stdin
            .take()
            .ok_or_else(|| "RocketPy bridge stdin was unavailable".to_string())?
            .write_all(&payload)
            .map_err(|error| format!("failed to send RocketPy bridge request: {error}"))?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("failed to collect RocketPy bridge output: {error}"))?;
        if !output.status.success() {
            let diagnostics = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(format!(
                "RocketPy bridge exited with {}{}",
                output.status,
                if diagnostics.is_empty() {
                    String::new()
                } else {
                    format!(": {diagnostics}")
                }
            ));
        }
        let response: BridgeResponseEnvelope =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                format!(
                    "RocketPy bridge returned invalid JSON: {error}; stderr: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )
            })?;
        if response.schema_version != RESPONSE_SCHEMA {
            return Err(format!(
                "unsupported RocketPy response schema: {}",
                response.schema_version
            ));
        }
        if response.rocketpy_version.is_empty() {
            return Err("RocketPy bridge response omitted rocketpy_version".into());
        }
        let summary = serde_json::from_value(response.summary)
            .map_err(|error| format!("RocketPy bridge response has an invalid summary: {error}"))?;
        Ok(BridgeResponse {
            rocketpy_version: response.rocketpy_version,
            summary,
        })
    }
}

impl SimEngine for RocketPyEngine {
    fn id(&self) -> &'static str {
        "rocketpy-bridge"
    }

    fn version(&self) -> &'static str {
        "ascent-rocketpy-bridge-v1"
    }

    fn run(
        &self,
        rocket: &Rocket,
        motor: &Motor,
        env: &Environment,
        config: &SimConfig,
    ) -> Result<SimSummary, String> {
        let response = self.invoke(&BridgeRequest::new(rocket, motor, env, config))?;
        Ok(SimSummary {
            rocket: rocket.name.clone(),
            motor: motor.designation.clone(),
            apogee_m: response.summary.apogee_m,
            apogee_time_s: response.summary.apogee_time_s,
            burnout_time_s: response.summary.burnout_time_s,
            burnout_velocity_ms: response.summary.burnout_velocity_ms,
            max_velocity_ms: response.summary.max_velocity_ms,
            rail_exit_velocity_ms: response.summary.rail_exit_velocity_ms,
            landing_time_s: response.summary.landing_time_s,
            landing_velocity_ms: response.summary.landing_velocity_ms,
            events: response.summary.events,
            timestep_s: response.summary.timestep_s,
            steps: response.summary.steps,
            convergence: None,
            input_hash: input_hash(rocket, motor, env, config),
            sim_version: format!("rocketpy-{}", response.rocketpy_version),
        })
    }
}

#[derive(Debug, Serialize)]
struct BridgeRequest<'a> {
    schema_version: &'static str,
    rocket: RocketRequest<'a>,
    motor: MotorRequest<'a>,
    environment: &'a Environment,
    config: &'a SimConfig,
}

impl<'a> BridgeRequest<'a> {
    fn new(
        rocket: &'a Rocket,
        motor: &'a Motor,
        environment: &'a Environment,
        config: &'a SimConfig,
    ) -> Self {
        Self {
            schema_version: REQUEST_SCHEMA,
            rocket: RocketRequest {
                name: &rocket.name,
                dry_mass_kg: rocket.dry_mass_kg,
                drag: rocket.drag.as_ref(),
                recovery: rocket.recovery.as_ref(),
            },
            motor: MotorRequest {
                designation: &motor.designation,
                total_mass_kg: motor.total_mass_kg,
                propellant_mass_kg: motor.propellant_mass_kg,
                thrust_curve: &motor.thrust_curve,
            },
            environment,
            config,
        }
    }
}

/// The fields the Python adapter actually consumes. In particular, the
/// adapter does not receive motor provenance, expected values, or geometry it
/// cannot map honestly into Ascent's point-mass comparison.
#[derive(Debug, Serialize)]
struct RocketRequest<'a> {
    name: &'a str,
    dry_mass_kg: f64,
    drag: Option<&'a DragModel>,
    recovery: Option<&'a Recovery>,
}

#[derive(Debug, Serialize)]
struct MotorRequest<'a> {
    designation: &'a str,
    total_mass_kg: f64,
    propellant_mass_kg: f64,
    thrust_curve: &'a [(f64, f64)],
}

struct BridgeResponse {
    rocketpy_version: String,
    summary: BridgeSummary,
}

#[derive(Debug, Deserialize)]
struct BridgeResponseEnvelope {
    schema_version: String,
    rocketpy_version: String,
    summary: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct BridgeSummary {
    apogee_m: f64,
    apogee_time_s: f64,
    burnout_time_s: f64,
    burnout_velocity_ms: f64,
    max_velocity_ms: f64,
    rail_exit_velocity_ms: f64,
    landing_time_s: f64,
    landing_velocity_ms: f64,
    events: Vec<EventSummary>,
    timestep_s: f64,
    steps: u64,
}
