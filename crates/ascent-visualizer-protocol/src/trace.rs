//! Canonical trace reconstruction and hashing.
//!
//! The bridge computes [`trace_hash`] over the exact channel data and events it
//! is about to stream; the client reassembles the same channels and events from
//! the frames it receives and recomputes the hash. A match proves the stream
//! reconstructed the trace byte-for-byte.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::messages::TraceEvent;

/// One named, ordered scalar channel of a trace.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Channel {
    pub name: String,
    pub samples: Vec<f64>,
}

/// SHA-256 (lowercase hex) over the canonical JSON of the ordered channels and
/// events. Deterministic: identical channels and events always hash the same.
pub fn trace_hash(channels: &[Channel], events: &[TraceEvent]) -> String {
    #[derive(Serialize)]
    struct Canonical<'a> {
        channels: &'a [Channel],
        events: &'a [TraceEvent],
    }
    let bytes =
        serde_json::to_vec(&Canonical { channels, events }).expect("trace hash inputs serialize");
    format!("{:x}", Sha256::digest(bytes))
}
