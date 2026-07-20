//! Visualizer protocol v1: envelope, message kinds, framing codec, and the
//! canonical trace hash shared by the Rust bridge and the Unity client.
//!
//! Transport is length-prefixed JSON over a byte stream (stdio in the vertical
//! slice). There is no network, no file access, and no command surface here —
//! only message definitions and framing.

pub mod codec;
pub mod messages;
pub mod trace;

pub use codec::{encode_frame, CodecError, FrameDecoder};
pub use messages::{
    error_code, ChannelSpec, ClientRequest, Envelope, Kinded, MissionEntry, ServerMessage,
    TraceEvent, MAX_CHUNK_SAMPLES, MAX_FRAME_BYTES, PROTOCOL_VERSION,
};
pub use trace::{trace_hash, Channel};
