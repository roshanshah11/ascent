//! Re-entrant staged reduced-rotational run driven from an immutable mission snapshot.
//!
//! This is the pure-Rust seam for an interactive review: a [`MissionSnapshot`]
//! captures the immutable input facts of a flight, and a [`RunSession`] steps
//! the *same* canonical integrator ([`crate::sixdof::simulate_sixdof_staged`])
//! forward in fixed-count chunks. Because both paths share one
//! `StagedStepper`, a session stepped to completion is bit-for-bit identical to
//! the batch trace. There is no protocol, bridge, or transport here — only the
//! Rust API a live control surface would later wrap.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use ascent_domain::reference::ReferenceMission;

use crate::reference_trace::mission_to_sixdof_stages;
use crate::rocket::{Environment, Recovery};
use crate::sim::SimConfig;
use crate::sixdof::{
    FlightPhase, SixDofLaunch, SixDofResult, SixDofStage, StagedStepper, StepStatus, Wind3DProfile,
};

/// Schema version of the canonical [`MissionSnapshot`] serialization used by
/// [`MissionSnapshot::config_hash`]. Bump this whenever the set or meaning of
/// the immutable snapshot fields changes, so a `config_hash` is only ever
/// compared against another produced under the same schema.
pub const SNAPSHOT_SCHEMA_VERSION: u16 = 1;

/// A provenance reference: which source artifact backs the mission, by id and
/// content digest. Snapshots carry references, never the full evidence graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub id: String,
    pub sha256: String,
}

/// Provenance references binding a snapshot back to its reference mission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionProvenance {
    pub mission_id: String,
    pub sources: Vec<SourceRef>,
}

/// The immutable input facts of one flight.
///
/// Contains only inputs: the resolved per-stage flight vehicle (mass, motor,
/// geometry, drag — a pure derivation of the reference vehicle's part tree,
/// captured once at construction), the launch / environment / integration
/// inputs, and provenance references. It deliberately does **not** store the
/// temporal stage schedule (separation/ignition timing) or any flight events —
/// [`RunSession`] derives those while stepping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionSnapshot {
    pub schema_version: u16,
    /// Per-stage flight vehicle in burn order.
    pub stages: Vec<SixDofStage>,
    pub recovery: Option<Recovery>,
    pub wind: Wind3DProfile,
    pub launch: SixDofLaunch,
    pub environment: Environment,
    pub config: SimConfig,
    pub provenance: MissionProvenance,
}

impl MissionSnapshot {
    /// Build the snapshot for a reference mission. The staged vehicle is
    /// derived from the mission's part tree via [`mission_to_sixdof_stages`].
    /// This is the single source of the fixed reference inputs (launch, wind,
    /// environment, integration config, recovery); the canonical batch path
    /// [`crate::reference_trace::trace_reference_mission`] builds this snapshot
    /// and runs its fields, so batch and stepped runs share identical inputs.
    pub fn from_reference_mission(mission: &ReferenceMission) -> Result<Self, String> {
        let stages = mission_to_sixdof_stages(mission)?;
        Ok(Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            stages,
            recovery: Some(Recovery::single(1.5, 40.0)),
            wind: Wind3DProfile::calm(),
            launch: SixDofLaunch::vertical(),
            environment: Environment::default(),
            config: SimConfig {
                dt_s: 0.02,
                max_time_s: 6_000.0,
            },
            provenance: MissionProvenance {
                mission_id: mission.mission_id.0.clone(),
                sources: mission
                    .sources
                    .iter()
                    .map(|source| SourceRef {
                        id: source.id.clone(),
                        sha256: source.sha256.clone(),
                    })
                    .collect(),
            },
        })
    }

    /// SHA-256 over a defined, schema-versioned canonical serialization of the
    /// simulation-affecting inputs.
    ///
    /// Canonicalization is key-sorted JSON: `serde_json`'s object map is a
    /// `BTreeMap` (the crate is built without `preserve_order`), so the whole
    /// tree is emitted with sorted keys. The digest therefore depends only on
    /// the field *values* and [`SNAPSHOT_SCHEMA_VERSION`], never on Rust struct
    /// field declaration order. Provenance references are metadata, not inputs
    /// to the numerical model, and are excluded.
    pub fn config_hash(&self) -> String {
        let canonical = serde_json::json!({
            "schema_version": self.schema_version,
            "stages": self.stages,
            "recovery": self.recovery,
            "wind": self.wind,
            "launch": self.launch,
            "environment": self.environment,
            "config": self.config,
        });
        let bytes = serde_json::to_vec(&canonical).expect("snapshot inputs serialize");
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        format!("{:x}", hasher.finalize())
    }
}

/// Result of a single [`RunSession::advance`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvanceOutcome {
    /// Steps ran and the flight is still in progress.
    Advanced,
    /// The flight reached a terminal condition during (or before) this call;
    /// [`RunSession::result`] now returns the final trace.
    Completed,
    /// The session was cancelled; no steps ran.
    Cancelled,
}

/// Latest-state-only view of a [`RunSession`] after an advance, mirroring the
/// fields a live control surface streams to a renderer. All values come
/// straight from the shared integrator — the caller never re-derives physics.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionState {
    /// Core steps executed so far.
    pub step_cursor: u64,
    pub sim_time_s: f64,
    pub position_m: [f64; 3],
    pub velocity_ms: [f64; 3],
    pub attitude_wxyz: [f64; 4],
    pub phase: FlightPhase,
    /// Active stage index in burn order.
    pub stage_index: u32,
    /// Active-stage propulsion magnitude (newtons); `0` between burns.
    pub thrust_n: f64,
}

/// One derived flight event, stringified as the canonical trace summary does
/// (`{:?}` over the event kind), so a session's incremental events and its
/// completed trace events use one vocabulary.
#[derive(Debug, Clone, PartialEq)]
pub struct FlightEvent {
    pub kind: String,
    pub t_s: f64,
    pub altitude_m: f64,
    pub velocity_ms: f64,
}

/// A live-controllable staged reduced-rotational run.
///
/// The control surface is intentionally minimal and matches the four
/// operations a first live seam needs:
/// - [`RunSession::create`] — `CreateSession(snapshot)`.
/// - [`RunSession::advance`] — `AdvanceSteps(fixed_step_count)`. Time only
///   advances by requesting more fixed steps; there is no variable `dt`. Pause
///   is simply not calling `advance`; restart is a fresh `create` from the same
///   snapshot.
/// - [`RunSession::cancel`] — `Cancel`. Further `advance` calls are no-ops.
/// - [`RunSession::shutdown`] — `Shutdown`. Consumes and drops the session.
pub struct RunSession {
    stepper: StagedStepper,
    config_hash: String,
    cancelled: bool,
    completed: bool,
}

impl RunSession {
    /// `CreateSession(snapshot)`: derive the staged run from immutable inputs.
    ///
    /// Fails closed on an unrecognized snapshot schema version — an unknown
    /// version means the immutable-input contract differs from what this build
    /// can faithfully replay, so it must not be run rather than silently
    /// reinterpreted.
    pub fn create(snapshot: &MissionSnapshot) -> Result<Self, String> {
        if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported MissionSnapshot schema version {} (this build supports {})",
                snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION
            ));
        }
        let stepper = StagedStepper::new(
            &snapshot.stages,
            snapshot.recovery.clone(),
            &snapshot.wind,
            &snapshot.launch,
            &snapshot.environment,
            &snapshot.config,
        )?;
        Ok(Self {
            stepper,
            config_hash: snapshot.config_hash(),
            cancelled: false,
            completed: false,
        })
    }

    /// The snapshot's `config_hash`, captured at creation.
    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    /// `AdvanceSteps(fixed_step_count)`: drive the shared integrator forward by
    /// up to `steps` fixed core steps, stopping early if the flight completes.
    pub fn advance(&mut self, steps: u64) -> Result<AdvanceOutcome, String> {
        if self.cancelled {
            return Ok(AdvanceOutcome::Cancelled);
        }
        for _ in 0..steps {
            if self.completed {
                break;
            }
            if matches!(self.stepper.step()?, StepStatus::Finished) {
                self.completed = true;
            }
        }
        Ok(if self.completed {
            AdvanceOutcome::Completed
        } else {
            AdvanceOutcome::Advanced
        })
    }

    /// Whether the flight has reached a terminal condition.
    pub fn is_complete(&self) -> bool {
        self.completed
    }

    /// Latest-state-only view for streaming after an advance. Reads the current
    /// integrator state directly — no history clone, no re-derivation.
    pub fn state(&self) -> SessionState {
        let sample = self.stepper.latest_sample();
        SessionState {
            step_cursor: self.stepper.step_cursor(),
            sim_time_s: sample.time_s,
            position_m: sample.position_m,
            velocity_ms: sample.velocity_ms,
            attitude_wxyz: sample.attitude_wxyz,
            phase: sample.phase,
            stage_index: self.stepper.stage_index() as u32,
            thrust_n: self.stepper.thrust_n(),
        }
    }

    /// The full ordered flight-event set derived so far. Callers slice off the
    /// tail beyond what they last saw to obtain the incremental events crossed
    /// since the previous advance.
    pub fn events(&self) -> Vec<FlightEvent> {
        self.stepper
            .events()
            .iter()
            .map(|e| FlightEvent {
                kind: format!("{:?}", e.kind),
                t_s: e.t,
                altitude_m: e.altitude_m,
                velocity_ms: e.velocity_ms,
            })
            .collect()
    }

    /// `Cancel`: abandon the run. Subsequent `advance` calls are no-ops.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Whether the session has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// The trajectory assembled so far. After completion this is the final
    /// trace; before completion it is the partial history to that point.
    pub fn result(&self) -> SixDofResult {
        self.stepper.finalize()
    }

    /// `Shutdown`: consume and drop the session.
    pub fn shutdown(self) {}
}
