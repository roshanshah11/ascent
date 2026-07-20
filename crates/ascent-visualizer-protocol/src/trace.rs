//! Canonical trace reconstruction and hashing.
//!
//! The bridge computes [`trace_hash`] over the exact channel data and events it
//! is about to stream; the client reassembles the same channels and events from
//! the frames it receives and recomputes the hash. A match proves the stream
//! reconstructed the trace.
//!
//! The canonical form is a **binary** byte stream, not JSON text: floating-point
//! samples are hashed as their little-endian IEEE-754 bit patterns. This makes
//! the hash identical across Rust and the C# client without depending on
//! language-specific shortest-float formatting.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::messages::TraceEvent;

/// One named, ordered scalar channel of a trace.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Channel {
    pub name: String,
    pub samples: Vec<f64>,
}

/// SHA-256 (lowercase hex) over the canonical binary encoding of the ordered
/// channels and events. Layout (all integers little-endian):
///
/// ```text
/// u32 channel_count
///   per channel: u32 name_len, name bytes, u32 sample_count, f64-LE * sample_count
/// u32 event_count
///   per event: u32 kind_len, kind bytes, f64-LE t_s, f64-LE altitude_m, f64-LE velocity_ms
/// ```
pub fn trace_hash(channels: &[Channel], events: &[TraceEvent]) -> String {
    let mut hasher = Sha256::new();
    hasher.update((channels.len() as u32).to_le_bytes());
    for channel in channels {
        hasher.update((channel.name.len() as u32).to_le_bytes());
        hasher.update(channel.name.as_bytes());
        hasher.update((channel.samples.len() as u32).to_le_bytes());
        for sample in &channel.samples {
            hasher.update(sample.to_le_bytes());
        }
    }
    hasher.update((events.len() as u32).to_le_bytes());
    for event in events {
        hasher.update((event.kind.len() as u32).to_le_bytes());
        hasher.update(event.kind.as_bytes());
        hasher.update(event.t_s.to_le_bytes());
        hasher.update(event.altitude_m.to_le_bytes());
        hasher.update(event.velocity_ms.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}
