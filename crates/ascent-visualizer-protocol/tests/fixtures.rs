//! Cross-language fixtures round-trip: every committed `.frame` under
//! `data/protocol/visualizer/v1/` decodes to the message its name declares, and
//! every codec-level malformed frame fails closed. These are the same bytes the
//! Unity client consumes, so drift here breaks both languages at once.

use std::path::PathBuf;

use ascent_visualizer_protocol::{
    trace_hash, Channel, ClientRequest, FrameDecoder, ServerMessage, TraceEvent, MAX_FRAME_BYTES,
};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/protocol/visualizer/v1")
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(fixtures().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

#[test]
fn client_fixtures_decode_to_declared_kind() {
    for name in [
        "hello",
        "list_missions",
        "run_mission",
        "cancel_run",
        "shutdown",
    ] {
        let bytes = read(&format!("client/{name}.frame"));
        let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
        let decoded = decoder.push(&bytes).unwrap();
        assert_eq!(decoded.len(), 1, "{name}");
        assert_eq!(decoded[0].kind, name, "{name} kind");
    }
}

#[test]
fn server_fixtures_decode_to_declared_kind() {
    let kinds = [
        "hello_ack",
        "mission_catalog",
        "run_started",
        "run_progress",
        "trace_manifest",
        "trace_channel_chunk",
        "trace_events",
        "run_completed",
        "run_cancelled",
        "error",
    ];
    for name in kinds {
        let bytes = read(&format!("server/{name}.frame"));
        let mut decoder = FrameDecoder::<ServerMessage>::new(MAX_FRAME_BYTES);
        let decoded = decoder.push(&bytes).unwrap();
        assert_eq!(decoded.len(), 1, "{name}");
        assert_eq!(decoded[0].kind, name, "{name} kind");
    }
}

#[test]
fn codec_level_malformed_fixtures_fail_closed() {
    for name in [
        "zero_length",
        "oversize_header",
        "bad_json",
        "bad_utf8",
        "wrong_version",
    ] {
        let bytes = read(&format!("malformed/{name}.frame"));
        let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
        assert!(decoder.push(&bytes).is_err(), "{name} must be rejected");
        assert!(decoder.push(b"x").is_err(), "{name} must stay poisoned");
    }
}

#[test]
fn sequence_level_fixtures_are_well_formed_frames() {
    // Duplicate id and out-of-order are valid frames rejected by the bridge's
    // state machine, not the codec — they must still decode cleanly here.
    for name in ["duplicate_message_id", "out_of_order"] {
        let bytes = read(&format!("malformed/{name}.frame"));
        let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
        assert!(decoder.push(&bytes).is_ok(), "{name} is a valid frame");
    }
}

#[test]
fn hash_invalid_manifest_fixture_disagrees_with_its_chunks() {
    // The reference reconstruction the client would compute for the chunk
    // fixture; the hash-invalid manifest must not match it.
    let channels = vec![
        Channel {
            name: "time_s".into(),
            samples: vec![0.0, 0.02, 0.04],
        },
        Channel {
            name: "position_z_m".into(),
            samples: vec![0.0, 1.5, 6.0],
        },
    ];
    let events = vec![
        TraceEvent {
            kind: "Liftoff".into(),
            t_s: 0.2,
            altitude_m: 0.0,
            velocity_ms: 12.0,
        },
        TraceEvent {
            kind: "Landing".into(),
            t_s: 3900.0,
            altitude_m: 0.0,
            velocity_ms: 8.0,
        },
    ];
    let good = trace_hash(&channels, &events);

    let bytes = read("server/trace_manifest_hash_invalid.frame");
    let mut decoder = FrameDecoder::<ServerMessage>::new(MAX_FRAME_BYTES);
    let decoded = decoder.push(&bytes).unwrap();
    match &decoded[0].payload {
        ServerMessage::TraceManifest { trace_hash, .. } => {
            assert_ne!(trace_hash, &good, "hash-invalid fixture must not match");
        }
        other => panic!("expected trace_manifest, got {other:?}"),
    }
}
