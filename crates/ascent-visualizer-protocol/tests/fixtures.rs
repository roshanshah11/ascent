//! Cross-language fixtures round-trip: every committed `.frame` under
//! `data/protocol/visualizer/v1/` decodes to the message its name declares, and
//! every codec-level malformed frame fails closed. These are the same bytes the
//! Unity client consumes, so drift here breaks both languages at once.

use std::path::PathBuf;

use ascent_visualizer_protocol::{
    encode_frame, error_code, trace_hash, AdvanceStateOutcome, Channel, ClientRequest, Envelope,
    FrameDecoder, ServerMessage, TraceEvent, CAP_INTERACTIVE_SESSION_V1, MAX_ADVANCE_STEPS,
    MAX_FRAME_BYTES,
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

// ---------------------------------------------------------------------------
// interactive-session/1 corpus
// ---------------------------------------------------------------------------

fn decode_client(rel: &str) -> Envelope<ClientRequest> {
    let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
    let mut out = decoder.push(&read(rel)).unwrap();
    assert_eq!(out.len(), 1, "{rel} decodes to exactly one frame");
    out.pop().unwrap()
}

fn decode_server(rel: &str) -> Envelope<ServerMessage> {
    let mut decoder = FrameDecoder::<ServerMessage>::new(MAX_FRAME_BYTES);
    let mut out = decoder.push(&read(rel)).unwrap();
    assert_eq!(out.len(), 1, "{rel} decodes to exactly one frame");
    out.pop().unwrap()
}

/// Every new session client frame decodes to its declared kind, re-encodes to
/// byte-identical bytes, and equals its `.json` sidecar (round-trip both ways).
#[test]
fn session_client_fixtures_round_trip_exactly() {
    for name in ["create_session", "advance_steps", "cancel"] {
        let frame = read(&format!("client/{name}.frame"));
        let env = decode_client(&format!("client/{name}.frame"));
        assert_eq!(env.kind, name, "{name} kind");
        assert_eq!(
            encode_frame(&env).unwrap(),
            frame,
            "{name} re-encode is exact"
        );
        let from_json: Envelope<ClientRequest> =
            serde_json::from_slice(&read(&format!("client/{name}.json"))).unwrap();
        assert_eq!(from_json, env, "{name} json sidecar matches frame");
    }
}

/// Every new session server frame decodes, re-encodes exactly, and matches its
/// `.json` sidecar.
#[test]
fn session_server_fixtures_round_trip_exactly() {
    for (file, kind) in [
        ("session_created", "session_created"),
        ("state_update", "state_update"),
        ("session_trace_manifest", "session_trace_manifest"),
        ("session_trace_chunk_time_s", "session_trace_chunk"),
        ("session_trace_chunk_position_z_m", "session_trace_chunk"),
        ("session_trace_events", "session_trace_events"),
        ("session_completed", "session_completed"),
        ("session_cancelled", "session_cancelled"),
    ] {
        let frame = read(&format!("server/{file}.frame"));
        let env = decode_server(&format!("server/{file}.frame"));
        assert_eq!(env.kind, kind, "{file} kind");
        assert_eq!(
            encode_frame(&env).unwrap(),
            frame,
            "{file} re-encode is exact"
        );
        let from_json: Envelope<ServerMessage> =
            serde_json::from_slice(&read(&format!("server/{file}.json"))).unwrap();
        assert_eq!(from_json, env, "{file} json sidecar matches frame");
    }
}

/// `AdvanceSteps` fixture stays within the sanity cap and `CreateSession`
/// carries a schema-versioned snapshot object.
#[test]
fn session_request_payloads_are_well_formed() {
    match decode_client("client/advance_steps.frame").payload {
        ClientRequest::AdvanceSteps { steps, .. } => {
            assert!((1..=MAX_ADVANCE_STEPS).contains(&steps), "steps within cap");
        }
        other => panic!("expected advance_steps, got {other:?}"),
    }
    match decode_client("client/create_session.frame").payload {
        ClientRequest::CreateSession { snapshot } => {
            assert!(
                snapshot.get("schema_version").is_some(),
                "snapshot carries schema_version"
            );
        }
        other => panic!("expected create_session, got {other:?}"),
    }
}

/// `StateUpdate` carries the Rust-issued `thrust_n` plume field and an
/// advance-only outcome; floats are i64 bit patterns that decode cleanly.
#[test]
fn state_update_fixture_carries_thrust_and_advance_outcome() {
    match decode_server("server/state_update.frame").payload {
        ServerMessage::StateUpdate {
            outcome,
            thrust_n,
            sim_time_s,
            ..
        } => {
            assert_eq!(outcome, AdvanceStateOutcome::Advanced);
            assert_eq!(f64::from_bits(thrust_n as u64), 17800.0, "thrust decodes");
            assert_eq!(f64::from_bits(sim_time_s as u64), 2.0, "sim_time decodes");
        }
        other => panic!("expected state_update, got {other:?}"),
    }
}

/// `SessionTraceEvents` delivers exactly the event set hashed into
/// `SessionCompleted.trace_hash`, so a client can verify the hash.
#[test]
fn session_trace_events_reconstruct_the_completed_hash() {
    let mut channels = Vec::new();
    for name in ["time_s", "position_z_m"] {
        let payload = decode_server(&format!("server/session_trace_chunk_{name}.frame")).payload;
        match payload {
            ServerMessage::SessionTraceChunk {
                channel,
                sequence,
                samples,
                ..
            } => {
                assert_eq!(channel, name, "fixture channel");
                assert_eq!(sequence, 0, "fixture is the first chunk");
                channels.push(Channel {
                    name: channel,
                    samples: samples
                        .into_iter()
                        .map(|bits| f64::from_bits(bits as u64))
                        .collect(),
                });
            }
            other => panic!("expected session_trace_chunk, got {other:?}"),
        }
    }
    let events = match decode_server("server/session_trace_events.frame").payload {
        ServerMessage::SessionTraceEvents { events, .. } => events,
        other => panic!("expected session_trace_events, got {other:?}"),
    };
    let reconstructed = trace_hash(&channels, &events);
    match decode_server("server/session_completed.frame").payload {
        ServerMessage::SessionCompleted { trace_hash, .. } => {
            assert_eq!(trace_hash, reconstructed, "events reconstruct the hash");
        }
        other => panic!("expected session_completed, got {other:?}"),
    }
    match decode_server("server/session_trace_manifest.frame").payload {
        ServerMessage::SessionTraceManifest { trace_hash, .. } => {
            assert_eq!(trace_hash, reconstructed, "manifest agrees with events");
        }
        other => panic!("expected session_trace_manifest, got {other:?}"),
    }
}

/// The handshake fixtures realize the capability matrix: granted, absent (pure
/// v1), and unknown-ignored. Each `HelloAck`'s negotiated set equals the
/// `matrix.json` entry, and only non-empty negotiation permits a session.
#[test]
fn handshake_matrix_matches_fixtures() {
    let matrix: serde_json::Value = serde_json::from_slice(&read("handshake/matrix.json")).unwrap();

    let cases = [
        ("granted", "hello_granted", "hello_ack_granted"),
        ("absent", "hello_absent", "hello_ack_absent"),
        (
            "unknown_ignored",
            "hello_unknown",
            "hello_ack_unknown_ignored",
        ),
    ];
    for (scenario, hello, ack) in cases {
        let offered = match decode_client(&format!("handshake/{hello}.frame")).payload {
            ClientRequest::Hello { capabilities, .. } => capabilities,
            other => panic!("{scenario}: expected hello, got {other:?}"),
        };
        let negotiated = match decode_server(&format!("handshake/{ack}.frame")).payload {
            ServerMessage::HelloAck { capabilities, .. } => capabilities,
            other => panic!("{scenario}: expected hello_ack, got {other:?}"),
        };

        let entry = &matrix[scenario];
        let want_offered: Vec<String> = entry["client_offered"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let want_negotiated: Vec<String> = entry["negotiated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(offered, want_offered, "{scenario} offered");
        assert_eq!(negotiated, want_negotiated, "{scenario} negotiated");

        // The negotiated set is the intersection: never wider than what the
        // client offered, and a session is allowed iff it is non-empty.
        assert!(
            negotiated.iter().all(|c| offered.contains(c)),
            "{scenario}: negotiated ⊆ offered"
        );
        assert_eq!(
            entry["session_allowed"].as_bool().unwrap(),
            !negotiated.is_empty(),
            "{scenario} session_allowed"
        );
    }

    // The interactive-session capability is the one negotiated when offered, and
    // the unknown string is dropped.
    assert_eq!(
        matrix["granted"]["negotiated"][0],
        CAP_INTERACTIVE_SESSION_V1
    );
    assert_eq!(
        matrix["unknown_ignored"]["client_offered"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "unknown scenario offers two capabilities"
    );
    assert_eq!(
        matrix["unknown_ignored"]["negotiated"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "unknown capability is dropped"
    );
}

/// Every erroring §4 row has an `Error` frame whose `{code, message}` matches
/// `index.json`, with a valid close/stay disposition and only the two reused
/// error codes.
#[test]
fn error_fixtures_pin_code_message_and_disposition() {
    let index: serde_json::Value = serde_json::from_slice(&read("errors/index.json")).unwrap();
    let index = index.as_object().unwrap();
    assert!(!index.is_empty(), "error index is non-empty");

    for (name, meta) in index {
        match decode_server(&format!("errors/{name}.frame")).payload {
            ServerMessage::Error { code, message } => {
                let want_code = meta["code"].as_str().unwrap();
                assert_eq!(code, want_code, "{name} code");
                assert_eq!(
                    message,
                    meta["message_prefix"].as_str().unwrap(),
                    "{name} message"
                );
                assert!(
                    code == error_code::INVALID_REQUEST || code == error_code::RUN_FAILED,
                    "{name} reuses an existing error code"
                );
                let disposition = meta["disposition"].as_str().unwrap();
                assert!(
                    disposition == "close" || disposition == "stay",
                    "{name} disposition is close|stay"
                );
            }
            other => panic!("{name}: expected error, got {other:?}"),
        }
    }

    // The MAX_ADVANCE_STEPS rejection is present, names the concrete cap, and is
    // a recoverable (stay) operational error.
    let adv = &index["advance_steps_exceeds_max"];
    assert_eq!(adv["code"], error_code::INVALID_REQUEST);
    assert!(
        adv["message_prefix"]
            .as_str()
            .unwrap()
            .contains(&MAX_ADVANCE_STEPS.to_string()),
        "exceeds-max message names the cap"
    );
    assert_eq!(adv["disposition"], "stay");
}

/// Every new error frame re-encodes byte-identically (exact codec round-trip).
#[test]
fn error_fixtures_round_trip_exactly() {
    let index: serde_json::Value = serde_json::from_slice(&read("errors/index.json")).unwrap();
    for name in index.as_object().unwrap().keys() {
        let frame = read(&format!("errors/{name}.frame"));
        let env = decode_server(&format!("errors/{name}.frame"));
        assert_eq!(
            encode_frame(&env).unwrap(),
            frame,
            "{name} re-encode is exact"
        );
    }
}
