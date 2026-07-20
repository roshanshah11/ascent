//! Visualizer protocol v1 message types.
//!
//! Every frame on the wire is an [`Envelope`] whose `payload` is an internally
//! tagged message enum. The envelope's `kind` field mirrors the payload tag so
//! a reader can route without deserializing the payload. Client and server
//! kinds are closed sets — anything else is a protocol violation.

use serde::{Deserialize, Serialize};

/// Wire protocol version. Bumped only on a breaking envelope/message change.
pub const PROTOCOL_VERSION: u16 = 1;

/// Hard cap on a single frame's JSON payload, in bytes (16 MiB).
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Maximum samples carried by a single [`ServerMessage::TraceChannelChunk`].
pub const MAX_CHUNK_SAMPLES: usize = 1_024;

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
    /// First message; negotiates the protocol version.
    Hello {
        client: String,
        protocol_version: u16,
    },
    /// Ask for the catalog of runnable missions.
    ListMissions,
    /// Start a deterministic trace of one mission.
    RunMission { mission_id: String },
    /// Cancel the active run.
    CancelRun { run_id: String },
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
        samples: Vec<f64>,
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
    Error {
        code: String,
        message: String,
    },
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
