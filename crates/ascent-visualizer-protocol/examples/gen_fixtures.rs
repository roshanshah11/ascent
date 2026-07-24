//! Regenerate the cross-language protocol fixtures under
//! `data/protocol/visualizer/v1/`. Run from the repo root:
//!
//! ```bash
//! cargo run -p ascent-visualizer-protocol --example gen_fixtures
//! ```
//!
//! Every message kind gets a readable `.json` envelope and a canonical `.frame`
//! (the exact bytes on the wire). Malformed cases are raw `.frame` bytes the
//! Unity decoder must reject. All ids are fixed so the output is deterministic.

use std::fs;
use std::path::Path;

use ascent_visualizer_protocol::messages::error_code;
use ascent_visualizer_protocol::{
    encode_frame, trace_hash, AdvanceStateOutcome, Channel, ChannelSpec, ClientRequest, Envelope,
    MissionEntry, ServerMessage, TraceEvent, CAP_INTERACTIVE_SESSION_V1, MAX_ADVANCE_STEPS,
    MAX_FRAME_BYTES,
};
use serde_json::json;

/// Encode an `f64` as the i64 IEEE-754 bit pattern carried on the wire.
fn bits(x: f64) -> i64 {
    x.to_bits() as i64
}

fn write_client(dir: &Path, name: &str, message_id: &str, req: ClientRequest) {
    let env = Envelope::new(message_id, None, req);
    write_envelope(dir, name, &env);
}

fn write_server(dir: &Path, name: &str, message_id: &str, msg: ServerMessage) {
    let env = Envelope::new(message_id, Some("req-1".into()), msg);
    write_envelope(dir, name, &env);
}

fn write_envelope<T: serde::Serialize>(dir: &Path, name: &str, env: &Envelope<T>) {
    fs::create_dir_all(dir).unwrap();
    let json = serde_json::to_vec_pretty(env).unwrap();
    fs::write(dir.join(format!("{name}.json")), &json).unwrap();
    let frame = encode_frame(env).unwrap();
    fs::write(dir.join(format!("{name}.frame")), &frame).unwrap();
}

fn write_raw(dir: &Path, name: &str, bytes: &[u8]) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(name), bytes).unwrap();
}

/// Write a pretty-printed JSON sidecar (matrix / index metadata, not a frame).
fn write_json(dir: &Path, name: &str, value: &serde_json::Value) {
    fs::create_dir_all(dir).unwrap();
    let json = serde_json::to_vec_pretty(value).unwrap();
    fs::write(dir.join(name), &json).unwrap();
}

fn main() {
    let root = Path::new("data/protocol/visualizer/v1");
    let client = root.join("client");
    let server = root.join("server");
    let malformed = root.join("malformed");

    // --- client kinds ---
    write_client(
        &client,
        "hello",
        "c-hello",
        ClientRequest::Hello {
            client: "unity".into(),
            protocol_version: 1,
            capabilities: Vec::new(),
        },
    );
    write_client(
        &client,
        "list_missions",
        "c-list",
        ClientRequest::ListMissions,
    );
    write_client(
        &client,
        "run_mission",
        "c-run",
        ClientRequest::RunMission {
            mission_id: "nasa.black-brant-ix.reference".into(),
        },
    );
    write_client(
        &client,
        "cancel_run",
        "c-cancel",
        ClientRequest::CancelRun {
            run_id: "run-1".into(),
        },
    );
    write_client(&client, "shutdown", "c-shutdown", ClientRequest::Shutdown);

    // --- server kinds ---
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
    let hash = trace_hash(&channels, &events);

    write_server(
        &server,
        "hello_ack",
        "s-hello",
        ServerMessage::HelloAck {
            server: "ascent-visualizer-bridge".into(),
            protocol_version: 1,
            capabilities: Vec::new(),
        },
    );
    write_server(
        &server,
        "mission_catalog",
        "s-catalog",
        ServerMessage::MissionCatalog {
            missions: vec![MissionEntry {
                mission_id: "nasa.black-brant-ix.reference".into(),
                title: "NASA Black Brant IX (reference)".into(),
            }],
        },
    );
    write_server(
        &server,
        "run_started",
        "s-started",
        ServerMessage::RunStarted {
            run_id: "run-1".into(),
        },
    );
    write_server(
        &server,
        "run_progress",
        "s-progress",
        ServerMessage::RunProgress {
            run_id: "run-1".into(),
            fraction: 0.5,
        },
    );
    write_server(
        &server,
        "trace_manifest",
        "s-manifest",
        ServerMessage::TraceManifest {
            run_id: "run-1".into(),
            trace_hash: hash.clone(),
            sample_count: 3,
            event_count: events.len(),
            channels: channels
                .iter()
                .map(|c| ChannelSpec {
                    name: c.name.clone(),
                    sample_count: c.samples.len(),
                })
                .collect(),
        },
    );
    write_server(
        &server,
        "trace_channel_chunk",
        "s-chunk",
        ServerMessage::TraceChannelChunk {
            run_id: "run-1".into(),
            channel: "time_s".into(),
            sequence: 0,
            samples: vec![
                0.0_f64.to_bits() as i64,
                0.02_f64.to_bits() as i64,
                0.04_f64.to_bits() as i64,
            ],
        },
    );
    write_server(
        &server,
        "trace_events",
        "s-events",
        ServerMessage::TraceEvents {
            run_id: "run-1".into(),
            events: events.clone(),
        },
    );
    write_server(
        &server,
        "run_completed",
        "s-completed",
        ServerMessage::RunCompleted {
            run_id: "run-1".into(),
            trace_hash: hash.clone(),
        },
    );
    write_server(
        &server,
        "run_cancelled",
        "s-cancelled",
        ServerMessage::RunCancelled {
            run_id: "run-2".into(),
        },
    );
    write_server(
        &server,
        "error",
        "s-error",
        ServerMessage::Error {
            code: error_code::MISSION_NOT_FOUND.into(),
            message: "unknown mission id".into(),
        },
    );

    // A manifest whose hash does not match the chunk data: the client must
    // reject it after reconstruction.
    write_server(
        &server,
        "trace_manifest_hash_invalid",
        "s-manifest-bad",
        ServerMessage::TraceManifest {
            run_id: "run-1".into(),
            trace_hash: "0".repeat(64),
            sample_count: 3,
            event_count: events.len(),
            channels: channels
                .iter()
                .map(|c| ChannelSpec {
                    name: c.name.clone(),
                    sample_count: c.samples.len(),
                })
                .collect(),
        },
    );

    // --- malformed / fail-closed cases (raw frame bytes) ---
    write_raw(&malformed, "zero_length.frame", &0u32.to_le_bytes());

    let mut oversize = (MAX_FRAME_BYTES as u32 + 1).to_le_bytes().to_vec();
    oversize.push(b'x');
    write_raw(&malformed, "oversize_header.frame", &oversize);

    let bad_json = b"{not json";
    let mut bad_json_frame = (bad_json.len() as u32).to_le_bytes().to_vec();
    bad_json_frame.extend_from_slice(bad_json);
    write_raw(&malformed, "bad_json.frame", &bad_json_frame);

    let bad_utf8 = [0xffu8, 0xfe, 0xfd];
    let mut bad_utf8_frame = (bad_utf8.len() as u32).to_le_bytes().to_vec();
    bad_utf8_frame.extend_from_slice(&bad_utf8);
    write_raw(&malformed, "bad_utf8.frame", &bad_utf8_frame);

    let mut wrong_version = Envelope::new(
        "c-badver",
        None,
        ClientRequest::Hello {
            client: "unity".into(),
            protocol_version: 1,
            capabilities: Vec::new(),
        },
    );
    wrong_version.protocol_version = 999;
    write_raw(
        &malformed,
        "wrong_version.frame",
        &encode_frame(&wrong_version).unwrap(),
    );

    // Duplicate message id: two frames sharing one id (bridge rejects the 2nd).
    let dup = Envelope::new("c-dup", None, ClientRequest::ListMissions);
    let mut duplicate = encode_frame(&dup).unwrap();
    duplicate.extend_from_slice(&encode_frame(&dup).unwrap());
    write_raw(&malformed, "duplicate_message_id.frame", &duplicate);

    // Out-of-order: run_mission before hello (valid frame, invalid sequence).
    let ooo = Envelope::new(
        "c-ooo",
        None,
        ClientRequest::RunMission {
            mission_id: "nasa.black-brant-ix.reference".into(),
        },
    );
    write_raw(
        &malformed,
        "out_of_order.frame",
        &encode_frame(&ooo).unwrap(),
    );

    // --- interactive-session/1 corpus (§5.1) ---
    // The snapshot is a small *synthetic* MissionSnapshot, not the full flight:
    // opaque at the protocol layer, carrying only a `schema_version` the bridge
    // validates.
    let snapshot = json!({
        "schema_version": 1,
        "stages": [{ "name": "synthetic-sustainer", "dry_mass_kg": 100.0 }],
        "recovery": null,
        "wind": { "kind": "calm" },
        "launch": { "kind": "vertical" },
        "environment": { "kind": "default" },
        "config": { "dt_s": 0.02, "max_time_s": 6000.0 },
        "provenance": { "mission_id": "synthetic.fixture", "sources": [] }
    });
    write_client(
        &client,
        "create_session",
        "c-create",
        ClientRequest::CreateSession { snapshot },
    );
    write_client(
        &client,
        "advance_steps",
        "c-advance",
        ClientRequest::AdvanceSteps {
            session_id: "sess-1".into(),
            steps: 100,
        },
    );
    write_client(
        &client,
        "cancel",
        "c-cancel-session",
        ClientRequest::Cancel {
            session_id: "sess-1".into(),
        },
    );

    write_server(
        &server,
        "session_created",
        "s-session-created",
        ServerMessage::SessionCreated {
            session_id: "sess-1".into(),
            config_hash: "0".repeat(64),
            schema_version: 1,
        },
    );
    write_server(
        &server,
        "state_update",
        "s-state-update",
        ServerMessage::StateUpdate {
            session_id: "sess-1".into(),
            outcome: AdvanceStateOutcome::Advanced,
            step_cursor: 100,
            sim_time_s: bits(2.0),
            position_m: [bits(0.0), bits(0.0), bits(1234.5)],
            velocity_ms: [bits(0.0), bits(0.0), bits(310.25)],
            attitude_wxyz: [bits(1.0), bits(0.0), bits(0.0), bits(0.0)],
            phase: "boost".into(),
            stage_index: 0,
            thrust_n: bits(17800.0),
            new_events: vec![TraceEvent {
                kind: "Liftoff".into(),
                t_s: 0.2,
                altitude_m: 0.0,
                velocity_ms: 12.0,
            }],
        },
    );
    write_server(
        &server,
        "session_trace_manifest",
        "s-session-manifest",
        ServerMessage::SessionTraceManifest {
            session_id: "sess-1".into(),
            trace_hash: hash.clone(),
            sample_count: 3,
            event_count: events.len(),
            channels: channels
                .iter()
                .map(|c| ChannelSpec {
                    name: c.name.clone(),
                    sample_count: c.samples.len(),
                })
                .collect(),
        },
    );
    write_server(
        &server,
        "session_trace_chunk_time_s",
        "s-session-chunk-time-s",
        ServerMessage::SessionTraceChunk {
            session_id: "sess-1".into(),
            channel: "time_s".into(),
            sequence: 0,
            samples: vec![bits(0.0), bits(0.02), bits(0.04)],
        },
    );
    write_server(
        &server,
        "session_trace_chunk_position_z_m",
        "s-session-chunk-position-z-m",
        ServerMessage::SessionTraceChunk {
            session_id: "sess-1".into(),
            channel: "position_z_m".into(),
            sequence: 0,
            samples: vec![bits(0.0), bits(1.5), bits(6.0)],
        },
    );
    write_server(
        &server,
        "session_trace_events",
        "s-session-events",
        ServerMessage::SessionTraceEvents {
            session_id: "sess-1".into(),
            events: events.clone(),
        },
    );
    write_server(
        &server,
        "session_completed",
        "s-session-completed",
        ServerMessage::SessionCompleted {
            session_id: "sess-1".into(),
            trace_hash: hash.clone(),
        },
    );
    write_server(
        &server,
        "session_cancelled",
        "s-session-cancelled",
        ServerMessage::SessionCancelled {
            session_id: "sess-1".into(),
        },
    );

    // --- handshake capability matrix (§5.2) ---
    let handshake = root.join("handshake");
    let cap = CAP_INTERACTIVE_SESSION_V1;
    write_client(
        &handshake,
        "hello_granted",
        "h-granted",
        ClientRequest::Hello {
            client: "unity".into(),
            protocol_version: 1,
            capabilities: vec![cap.into()],
        },
    );
    write_server(
        &handshake,
        "hello_ack_granted",
        "h-ack-granted",
        ServerMessage::HelloAck {
            server: "ascent-visualizer-bridge".into(),
            protocol_version: 1,
            capabilities: vec![cap.into()],
        },
    );
    write_client(
        &handshake,
        "hello_absent",
        "h-absent",
        ClientRequest::Hello {
            client: "unity".into(),
            protocol_version: 1,
            capabilities: Vec::new(),
        },
    );
    write_server(
        &handshake,
        "hello_ack_absent",
        "h-ack-absent",
        ServerMessage::HelloAck {
            server: "ascent-visualizer-bridge".into(),
            protocol_version: 1,
            capabilities: Vec::new(),
        },
    );
    write_client(
        &handshake,
        "hello_unknown",
        "h-unknown",
        ClientRequest::Hello {
            client: "unity".into(),
            protocol_version: 1,
            capabilities: vec![cap.into(), "future-capability/9".into()],
        },
    );
    write_server(
        &handshake,
        "hello_ack_unknown_ignored",
        "h-ack-unknown",
        ServerMessage::HelloAck {
            server: "ascent-visualizer-bridge".into(),
            protocol_version: 1,
            capabilities: vec![cap.into()],
        },
    );
    // Scenario matrix: what each side offered vs. the negotiated intersection.
    let matrix = json!({
        "granted": {
            "client_offered": [cap],
            "negotiated": [cap],
            "session_allowed": true
        },
        "absent": {
            "client_offered": [],
            "negotiated": [],
            "session_allowed": false
        },
        "unknown_ignored": {
            "client_offered": [cap, "future-capability/9"],
            "negotiated": [cap],
            "session_allowed": true
        }
    });
    write_json(&handshake, "matrix.json", &matrix);

    // --- error fixtures: one Error frame per erroring §4 row (§5.3) ---
    let errors = root.join("errors");
    // (name, code, message, disposition) — message pins the prefix the bridge emits.
    let error_rows: &[(&str, &str, &str, &str)] = &[
        (
            "before_hello",
            error_code::INVALID_REQUEST,
            "request received before Hello",
            "close",
        ),
        (
            "capability_not_negotiated",
            error_code::INVALID_REQUEST,
            "capability not negotiated: interactive-session/1",
            "close",
        ),
        (
            "unknown_schema_version",
            error_code::INVALID_REQUEST,
            "unsupported snapshot schema_version",
            "close",
        ),
        (
            "run_already_active",
            error_code::INVALID_REQUEST,
            "a run is already active",
            "stay",
        ),
        (
            "unknown_session",
            error_code::INVALID_REQUEST,
            "unknown session id",
            "stay",
        ),
        (
            "advance_on_completed",
            error_code::INVALID_REQUEST,
            "session already completed",
            "stay",
        ),
        (
            "advance_steps_exceeds_max",
            error_code::INVALID_REQUEST,
            "steps exceeds MAX_ADVANCE_STEPS",
            "stay",
        ),
        (
            "duplicate_message_id",
            error_code::INVALID_REQUEST,
            "duplicate message id",
            "close",
        ),
        (
            "framing_violation",
            error_code::INVALID_REQUEST,
            "framing violation",
            "close",
        ),
        (
            "stepper_failed",
            error_code::RUN_FAILED,
            "stepper returned an error mid-advance",
            "stay",
        ),
    ];
    let mut index = serde_json::Map::new();
    for (name, code, message, disposition) in error_rows {
        // The exceeds-max message names the concrete cap so the client can
        // surface it; keep the pinned prefix stable for the test.
        let message = if *name == "advance_steps_exceeds_max" {
            format!("{message} ({MAX_ADVANCE_STEPS})")
        } else {
            (*message).to_string()
        };
        write_server(
            &errors,
            name,
            &format!("s-err-{name}"),
            ServerMessage::Error {
                code: (*code).into(),
                message: message.clone(),
            },
        );
        index.insert(
            (*name).to_string(),
            json!({
                "code": code,
                "message_prefix": message,
                "disposition": disposition
            }),
        );
    }
    write_json(&errors, "index.json", &serde_json::Value::Object(index));

    println!("wrote fixtures under {}", root.display());
}
