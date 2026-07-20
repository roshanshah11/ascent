//! Length-prefixed framing for protocol envelopes.
//!
//! A frame is a four-byte little-endian `u32` length followed by exactly that
//! many bytes of UTF-8 JSON. [`encode_frame`] produces one; [`FrameDecoder`]
//! reassembles a byte stream into whole envelopes. The decoder is fail-closed:
//! any framing, encoding, or version violation poisons it permanently.

use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::messages::{Envelope, MAX_FRAME_BYTES, PROTOCOL_VERSION};

/// A framing or decoding failure. Every variant poisons the decoder.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("frame length {len} exceeds maximum {max}")]
    FrameTooLarge { len: usize, max: usize },
    #[error("zero-length frame")]
    EmptyFrame,
    #[error("frame payload is not valid UTF-8")]
    Utf8,
    #[error("frame payload is not valid JSON: {0}")]
    Json(String),
    #[error("protocol version {found} is not supported (expected {expected})")]
    ProtocolMismatch { found: u16, expected: u16 },
    #[error("decoder is poisoned after an earlier framing violation")]
    Poisoned,
}

/// Serialize `message` and prepend its little-endian `u32` byte length.
pub fn encode_frame<T: Serialize>(message: &T) -> Result<Vec<u8>, CodecError> {
    let json = serde_json::to_vec(message).map_err(|e| CodecError::Json(e.to_string()))?;
    if json.len() > MAX_FRAME_BYTES || json.len() > u32::MAX as usize {
        return Err(CodecError::FrameTooLarge {
            len: json.len(),
            max: MAX_FRAME_BYTES,
        });
    }
    let mut out = Vec::with_capacity(4 + json.len());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&json);
    Ok(out)
}

/// Reassembles a byte stream into whole [`Envelope<T>`] values, retaining any
/// partial trailing frame across calls.
pub struct FrameDecoder<T> {
    max_frame_bytes: usize,
    buffer: Vec<u8>,
    poisoned: bool,
    _payload: PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> FrameDecoder<T> {
    /// Create a decoder that rejects frames longer than `max_frame_bytes`.
    pub fn new(max_frame_bytes: usize) -> Self {
        Self {
            max_frame_bytes,
            buffer: Vec::new(),
            poisoned: false,
            _payload: PhantomData,
        }
    }

    /// Feed bytes and return every whole envelope they complete. A partial
    /// trailing frame is retained. Any violation poisons the decoder.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Envelope<T>>, CodecError> {
        if self.poisoned {
            return Err(CodecError::Poisoned);
        }
        self.buffer.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.buffer.len() < 4 {
                break;
            }
            let len = u32::from_le_bytes([
                self.buffer[0],
                self.buffer[1],
                self.buffer[2],
                self.buffer[3],
            ]) as usize;
            if len == 0 {
                return Err(self.poison(CodecError::EmptyFrame));
            }
            if len > self.max_frame_bytes {
                return Err(self.poison(CodecError::FrameTooLarge {
                    len,
                    max: self.max_frame_bytes,
                }));
            }
            if self.buffer.len() < 4 + len {
                break;
            }
            let frame = self.buffer[4..4 + len].to_vec();
            self.buffer.drain(..4 + len);
            let text = match std::str::from_utf8(&frame) {
                Ok(text) => text,
                Err(_) => return Err(self.poison(CodecError::Utf8)),
            };
            let envelope: Envelope<T> = match serde_json::from_str(text) {
                Ok(envelope) => envelope,
                Err(e) => return Err(self.poison(CodecError::Json(e.to_string()))),
            };
            if envelope.protocol_version != PROTOCOL_VERSION {
                return Err(self.poison(CodecError::ProtocolMismatch {
                    found: envelope.protocol_version,
                    expected: PROTOCOL_VERSION,
                }));
            }
            out.push(envelope);
        }
        Ok(out)
    }

    fn poison(&mut self, error: CodecError) -> CodecError {
        self.poisoned = true;
        error
    }
}
