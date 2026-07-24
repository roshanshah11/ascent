//! Visualizer protocol v1 message types.
//!
//! Every frame on the wire is an [`Envelope`] whose `payload` is an internally
//! tagged message enum. The envelope's `kind` field mirrors the payload tag so
//! a reader can route without deserializing the payload. Client and server
//! kinds are closed sets — anything else is a protocol violation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wire protocol version. Bumped only on a breaking envelope/message change.
pub const PROTOCOL_VERSION: u16 = 1;

/// Hard cap on a single frame's JSON payload, in bytes (16 MiB).
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Maximum samples carried by a single [`ServerMessage::TraceChannelChunk`].
pub const MAX_CHUNK_SAMPLES: usize = 1_024;

/// Upper bound on `AdvanceSteps.steps` for one interactive-session advance.
///
/// A single request may advance the shared coordinator by at most this many
/// fixed core steps; a client time-warps by issuing more calls, never by one
/// unbounded call. `steps` outside `1..=MAX_ADVANCE_STEPS` is an operational
/// error (`INVALID_REQUEST`, connection stays). This is a protocol constant
/// pinned identically in Rust and the Unity client — not a tuning knob.
pub const MAX_ADVANCE_STEPS: u64 = 10_000;

/// Capability string for the stepped, live-controllable review session layered
/// over the v1 stdio bridge. Negotiated in [`ClientRequest::Hello`] /
/// [`ServerMessage::HelloAck`] `capabilities`; a client that did not negotiate
/// it may not send any session request.
pub const CAP_INTERACTIVE_SESSION_V1: &str = "interactive-session/1";

/// A framed protocol message: fixed envelope fields plus a typed payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope<T> {
    pub protocol_version: u16,
    pub message_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub kind: String,
    pub payload: T,
}

impl<T: Kinded> Envelope<T> {
    /// Build an envelope, deriving `kind` from the payload so the two can
    /// never disagree.
    pub fn new(message_id: impl Into<String>, request_id: Option<String>, payload: T) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            message_id: message_id.into(),
            request_id,
            kind: payload.kind().to_string(),
            payload,
        }
    }
}

/// A payload that knows its own protocol `kind` string.
pub trait Kinded {
    fn kind(&self) -> &'static str;
}

/// Requests the Unity client sends to the bridge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClientRequest {
    /// First message; negotiates the protocol version and, additively, the set
    /// of optional capabilities the client supports. An absent/empty
    /// `capabilities` is a pure v1 client. Unknown capability strings are
    /// ignored during negotiation.
    Hello {
        client: String,
        protocol_version: u16,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        capabilities: Vec<String>,
    },
    /// Ask for the catalog of runnable missions.
    ListMissions,
    /// Start a deterministic trace of one mission.
    RunMission { mission_id: String },
    /// Cancel the active run.
    CancelRun { run_id: String },
    /// Open a stepped, live-controllable review session from an inline
    /// immutable mission snapshot. Requires negotiated
    /// [`CAP_INTERACTIVE_SESSION_V1`]. The snapshot is carried opaquely at this
    /// layer (a JSON object with a `schema_version`); the bridge deserializes
    /// and validates it. See the interactive-session protocol contract.
    CreateSession { snapshot: Value },
    /// Advance an open session by a fixed count of core steps. `steps` must lie
    /// in `1..=MAX_ADVANCE_STEPS`.
    AdvanceSteps { session_id: String, steps: u64 },
    /// Drive a session to its `SessionCancelled` terminal state
    /// (idempotent-with-ack).
    Cancel { session_id: String },
    /// Shut the bridge down cleanly.
    Shutdown,
}

impl Kinded for ClientRequest {
    fn kind(&self) -> &'static str {
        match self {
            ClientRequest::Hello { .. } => "hello",
            ClientRequest::ListMissions => "list_missions",
            ClientRequest::RunMission { .. } => "run_mission",
            ClientRequest::CancelRun { .. } => "cancel_run",
            ClientRequest::CreateSession { .. } => "create_session",
            ClientRequest::AdvanceSteps { .. } => "advance_steps",
            ClientRequest::Cancel { .. } => "cancel",
            ClientRequest::Shutdown => "shutdown",
        }
    }
}

/// Messages the bridge streams back to the client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerMessage {
    HelloAck {
        server: String,
        protocol_version: u16,
        /// The intersection of client-offered and server-supported
        /// capabilities. Empty/absent means a pure v1 session.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        capabilities: Vec<String>,
    },
    MissionCatalog {
        missions: Vec<MissionEntry>,
    },
    RunStarted {
        run_id: String,
    },
    RunProgress {
        run_id: String,
        fraction: f64,
    },
    TraceManifest {
        run_id: String,
        trace_hash: String,
        sample_count: usize,
        event_count: usize,
        channels: Vec<ChannelSpec>,
    },
    TraceChannelChunk {
        run_id: String,
        channel: String,
        sequence: u32,
        /// IEEE-754 f64 samples as little-endian bit patterns (reinterpreted as
        /// i64). Transmitting bits — not decimal text — keeps the reconstructed
        /// trace hash exact across languages, independent of each side's float
        /// parser. Decode with `f64::from_bits(bits as u64)`.
        samples: Vec<i64>,
    },
    TraceEvents {
        run_id: String,
        events: Vec<TraceEvent>,
    },
    RunCompleted {
        run_id: String,
        trace_hash: String,
    },
    RunCancelled {
        run_id: String,
    },
    /// Acknowledges a valid `CreateSession`. `config_hash` is an opaque
    /// Rust-issued identity string the client stores and echoes, never parses.
    SessionCreated {
        session_id: String,
        config_hash: String,
        schema_version: u16,
    },
    /// Latest-state-only snapshot after one `AdvanceSteps`. All floating-point
    /// fields (`sim_time_s`, `position_m`, `velocity_ms`, `attitude_wxyz`,
    /// `thrust_n`) are i64 IEEE-754 bit patterns — decode with
    /// `f64::from_bits(bits as u64)`. `new_events` are only the events crossed
    /// since the previous update; the full event set arrives in
    /// [`ServerMessage::SessionTraceEvents`] at completion.
    StateUpdate {
        session_id: String,
        outcome: AdvanceStateOutcome,
        step_cursor: u64,
        sim_time_s: i64,
        position_m: [i64; 3],
        velocity_ms: [i64; 3],
        attitude_wxyz: [i64; 4],
        phase: String,
        stage_index: u32,
        /// Rust-issued active-stage propulsion magnitude in newtons, as an
        /// IEEE-754 bit pattern. Drives live plume rendering; the client must
        /// not derive propulsion physics itself. `0` between burns.
        thrust_n: i64,
        new_events: Vec<TraceEvent>,
    },
    /// Trace summary at normal completion, before the channel chunks. Distinct
    /// from the legacy `TraceManifest`; keyed by `session_id`, not `run_id`.
    SessionTraceManifest {
        session_id: String,
        trace_hash: String,
        sample_count: usize,
        event_count: usize,
        channels: Vec<ChannelSpec>,
    },
    /// One chunk of the full canonical trace, in canonical channel order after
    /// the manifest. Session-specific; does not reuse legacy
    /// `TraceChannelChunk` / its `run_id`.
    SessionTraceChunk {
        session_id: String,
        channel: String,
        sequence: u32,
        /// IEEE-754 f64 samples as bit patterns (reinterpreted as i64), as in
        /// [`ServerMessage::TraceChannelChunk`].
        samples: Vec<i64>,
    },
    /// The full, ordered flight event set for the completed run, after the
    /// channel chunks and before [`ServerMessage::SessionCompleted`]. Events are
    /// hashed into `trace_hash` alongside the channel samples, so `trace_hash`
    /// is not verifiable without them.
    SessionTraceEvents {
        session_id: String,
        events: Vec<TraceEvent>,
    },
    /// Terminates a normal completion, after the chunks and events. A
    /// hash-backed trace exists iff this was emitted.
    SessionCompleted {
        session_id: String,
        trace_hash: String,
    },
    /// Terminal state for a `Cancel`/`Shutdown` on a session that had not
    /// completed. No trace, no `trace_hash`. Re-emitted (as acknowledgment) on a
    /// repeated `Cancel`, correlated to the repeat request's `message_id`.
    SessionCancelled {
        session_id: String,
    },
    Error {
        code: String,
        message: String,
    },
}

/// Outcome of an `AdvanceSteps` reported by [`ServerMessage::StateUpdate`].
/// Cancellation is never an advance outcome — it is delivered by the separate
/// [`ServerMessage::SessionCancelled`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdvanceStateOutcome {
    /// Steps ran and the flight is still in progress.
    Advanced,
    /// The flight reached a terminal condition during (or before) this advance.
    Completed,
}

impl Kinded for ServerMessage {
    fn kind(&self) -> &'static str {
        match self {
            ServerMessage::HelloAck { .. } => "hello_ack",
            ServerMessage::MissionCatalog { .. } => "mission_catalog",
            ServerMessage::RunStarted { .. } => "run_started",
            ServerMessage::RunProgress { .. } => "run_progress",
            ServerMessage::TraceManifest { .. } => "trace_manifest",
            ServerMessage::TraceChannelChunk { .. } => "trace_channel_chunk",
            ServerMessage::TraceEvents { .. } => "trace_events",
            ServerMessage::RunCompleted { .. } => "run_completed",
            ServerMessage::RunCancelled { .. } => "run_cancelled",
            ServerMessage::SessionCreated { .. } => "session_created",
            ServerMessage::StateUpdate { .. } => "state_update",
            ServerMessage::SessionTraceManifest { .. } => "session_trace_manifest",
            ServerMessage::SessionTraceChunk { .. } => "session_trace_chunk",
            ServerMessage::SessionTraceEvents { .. } => "session_trace_events",
            ServerMessage::SessionCompleted { .. } => "session_completed",
            ServerMessage::SessionCancelled { .. } => "session_cancelled",
            ServerMessage::Error { .. } => "error",
        }
    }
}

/// One entry in a [`ServerMessage::MissionCatalog`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MissionEntry {
    pub mission_id: String,
    pub title: String,
}

/// Declares one channel present in the streamed trace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChannelSpec {
    pub name: String,
    pub sample_count: usize,
}

/// A single flight event carried by [`ServerMessage::TraceEvents`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceEvent {
    pub kind: String,
    pub t_s: f64,
    pub altitude_m: f64,
    pub velocity_ms: f64,
}

/// Stable error codes for [`ServerMessage::Error`].
pub mod error_code {
    pub const PROTOCOL_MISMATCH: &str = "protocol_mismatch";
    pub const INVALID_REQUEST: &str = "invalid_request";
    pub const MISSION_NOT_FOUND: &str = "mission_not_found";
    pub const RUN_FAILED: &str = "run_failed";
    pub const CANCELLED: &str = "cancelled";
    pub const INTERNAL: &str = "internal";
}
