//! Subprocess contract: spawn `ascent-visualizer-bridge --stdio`, negotiate,
//! run a mission to completion while reconstructing and hash-validating the
//! streamed trace, cancel a second run, run a third, and shut down cleanly.
//! stdout must carry only frames; stderr must never contain protocol bytes.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::process::{Child, ChildStdout, Command, Stdio};

use ascent_visualizer_protocol::{
    encode_frame, trace_hash, Channel, ClientRequest, Envelope, FrameDecoder, ServerMessage,
    MAX_FRAME_BYTES,
};

struct Bridge {
    child: Child,
    stdout: FrameReader,
}

struct FrameReader {
    inner: ChildStdout,
    decoder: FrameDecoder<ServerMessage>,
    pending: std::collections::VecDeque<Envelope<ServerMessage>>,
}

impl FrameReader {
    fn next(&mut self) -> Envelope<ServerMessage> {
        loop {
            if let Some(env) = self.pending.pop_front() {
                return env;
            }
            let mut buf = [0u8; 8192];
            let n = self.inner.read(&mut buf).expect("read child stdout");
            assert!(n > 0, "bridge closed stdout before expected frame");
            for env in self.decoder.push(&buf[..n]).expect("decode server frame") {
                self.pending.push_back(env);
            }
        }
    }

    /// Read frames until one whose kind is `kind`, returning everything up to
    /// and including it.
    fn drain_until(&mut self, kind: &str) -> Vec<Envelope<ServerMessage>> {
        let mut out = Vec::new();
        loop {
            let env = self.next();
            let is_target = env.kind == kind;
            out.push(env);
            if is_target {
                return out;
            }
        }
    }
}

impl Bridge {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ascent-visualizer-bridge"))
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn bridge");
        let stdout = child.stdout.take().unwrap();
        Bridge {
            child,
            stdout: FrameReader {
                inner: stdout,
                decoder: FrameDecoder::new(MAX_FRAME_BYTES),
                pending: Default::default(),
            },
        }
    }

    fn send(&mut self, message_id: &str, request: ClientRequest) {
        let env = Envelope::new(message_id, None, request);
        let frame = encode_frame(&env).unwrap();
        self.child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&frame)
            .unwrap();
        self.child.stdin.as_mut().unwrap().flush().unwrap();
    }
}

const MISSION_ID: &str = "nasa.black-brant-ix.reference";

const CAP_SESSION: &str = "interactive-session/1";

/// The canonical flight-phase vocabulary the bridge emits in `StateUpdate`.
const PHASES: &[&str] = &["pad", "rail", "ascent", "descent", "grounded"];

/// The immutable reference snapshot, serialized exactly as a client would carry
/// it in `CreateSession`. Built from the same `MissionSnapshot` the batch path
/// runs, so a stepped session created from it is bit-identical to the batch run.
fn reference_snapshot() -> (serde_json::Value, String) {
    let mission = ascent_domain::reference::black_brant_ix_reference().expect("reference mission");
    let snapshot = ascent_sim::MissionSnapshot::from_reference_mission(&mission).expect("snapshot");
    let config_hash = snapshot.config_hash();
    let value = serde_json::to_value(&snapshot).expect("snapshot serializes");
    (value, config_hash)
}

/// Send `Hello` offering `capabilities` and return the capabilities the bridge
/// echoes in its `HelloAck` (the negotiated intersection).
fn negotiate(bridge: &mut Bridge, capabilities: Vec<String>) -> Vec<String> {
    bridge.send(
        "c-hello",
        ClientRequest::Hello {
            client: "test".into(),
            protocol_version: 1,
            capabilities,
        },
    );
    match bridge.stdout.next().payload {
        ServerMessage::HelloAck { capabilities, .. } => capabilities,
        other => panic!("expected hello_ack, got {other:?}"),
    }
}

/// A reconstructed session trace: the completion frames a session emits after
/// its final `StateUpdate`, collapsed into hashable channels + events.
struct SessionTrace {
    manifest_hash: String,
    completed_hash: String,
    channels: Vec<Channel>,
    events: Vec<ascent_visualizer_protocol::TraceEvent>,
}

/// Advance a live session in `AdvanceSteps` chunks until it completes, then
/// collect the manifest → chunks → events → completed frames. Returns the
/// monotone `step_cursor` sequence seen across the `StateUpdate`s and the
/// reconstructed completion trace.
fn drive_to_completion(
    bridge: &mut Bridge,
    session_id: &str,
    step: u64,
) -> (Vec<u64>, SessionTrace) {
    let mut cursors = Vec::new();
    let mut advance_id = 0u64;
    loop {
        advance_id += 1;
        bridge.send(
            &format!("c-adv-{session_id}-{advance_id}"),
            ClientRequest::AdvanceSteps {
                session_id: session_id.into(),
                steps: step,
            },
        );
        let update = bridge.stdout.next();
        let completed = match update.payload {
            ServerMessage::StateUpdate {
                step_cursor,
                phase,
                outcome,
                ..
            } => {
                cursors.push(step_cursor);
                assert!(PHASES.contains(&phase.as_str()), "unknown phase {phase:?}");
                matches!(
                    outcome,
                    ascent_visualizer_protocol::AdvanceStateOutcome::Completed
                )
            }
            other => panic!("expected state_update, got {other:?}"),
        };
        if completed {
            break;
        }
    }

    // On completion the manifest → chunks → events → completed frames follow the
    // final StateUpdate, in canonical order.
    let mut manifest_hash = None;
    let mut completed_hash = None;
    let mut samples: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut channel_order: Vec<String> = Vec::new();
    let mut events = Vec::new();
    for env in bridge.stdout.drain_until("session_completed") {
        match env.payload {
            ServerMessage::SessionTraceManifest { trace_hash, .. } => {
                manifest_hash = Some(trace_hash)
            }
            ServerMessage::SessionTraceChunk {
                channel,
                sequence,
                samples: chunk,
                ..
            } => {
                let entry = samples.entry(channel.clone()).or_default();
                assert_eq!(
                    sequence as usize * 1024,
                    entry.len(),
                    "chunk sequence for {channel} is contiguous from zero"
                );
                if !channel_order.contains(&channel) {
                    channel_order.push(channel);
                }
                entry.extend(chunk.into_iter().map(|bits| f64::from_bits(bits as u64)));
            }
            ServerMessage::SessionTraceEvents { events: e, .. } => events = e,
            ServerMessage::SessionCompleted { trace_hash, .. } => completed_hash = Some(trace_hash),
            other => panic!("unexpected frame during completion: {other:?}"),
        }
    }
    let channels = channel_order
        .iter()
        .map(|name| Channel {
            name: name.clone(),
            samples: samples[name].clone(),
        })
        .collect();
    (
        cursors,
        SessionTrace {
            manifest_hash: manifest_hash.expect("manifest emitted"),
            completed_hash: completed_hash.expect("completed emitted"),
            channels,
            events,
        },
    )
}

#[test]
fn bridge_streams_deterministic_trace_and_supports_cancel_and_shutdown() {
    let mut bridge = Bridge::spawn();

    // Negotiate.
    bridge.send(
        "c-hello",
        ClientRequest::Hello {
            client: "test".into(),
            protocol_version: 1,
            capabilities: Vec::new(),
        },
    );
    let ack = bridge.stdout.next();
    assert_eq!(ack.kind, "hello_ack");

    // Catalog holds exactly the one reference mission.
    bridge.send("c-list", ClientRequest::ListMissions);
    let catalog = bridge.stdout.next();
    match catalog.payload {
        ServerMessage::MissionCatalog { missions } => {
            assert_eq!(missions.len(), 1);
            assert_eq!(missions[0].mission_id, MISSION_ID);
        }
        other => panic!("expected mission_catalog, got {other:?}"),
    }

    // Run 1: reconstruct every channel + events, validate the trace hash.
    bridge.send(
        "c-run1",
        ClientRequest::RunMission {
            mission_id: MISSION_ID.into(),
        },
    );
    let frames = bridge.stdout.drain_until("run_completed");

    let mut manifest_hash = None;
    let mut completed_hash = None;
    let mut channels: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut channel_order: Vec<String> = Vec::new();
    let mut events = Vec::new();
    let mut started = false;
    for env in frames {
        match env.payload {
            ServerMessage::RunStarted { .. } => started = true,
            ServerMessage::TraceManifest { trace_hash, .. } => manifest_hash = Some(trace_hash),
            ServerMessage::TraceChannelChunk {
                channel,
                sequence,
                samples,
                ..
            } => {
                let entry = channels.entry(channel.clone()).or_default();
                assert_eq!(
                    sequence as usize * 1024,
                    entry.len(),
                    "chunk sequence for {channel} is contiguous from zero"
                );
                if !channel_order.contains(&channel) {
                    channel_order.push(channel);
                }
                entry.extend(samples.into_iter().map(|bits| f64::from_bits(bits as u64)));
            }
            ServerMessage::TraceEvents { events: e, .. } => events = e,
            ServerMessage::RunCompleted { trace_hash, .. } => completed_hash = Some(trace_hash),
            _ => {}
        }
    }
    assert!(started, "run_started must precede the trace");
    let reconstructed: Vec<Channel> = channel_order
        .iter()
        .map(|name| Channel {
            name: name.clone(),
            samples: channels[name].clone(),
        })
        .collect();
    let recomputed = trace_hash(&reconstructed, &events);
    assert_eq!(
        manifest_hash.as_deref(),
        Some(recomputed.as_str()),
        "manifest hash matches reconstruction"
    );
    assert_eq!(
        completed_hash, manifest_hash,
        "completed hash matches manifest"
    );
    // Launch-to-landing events are present.
    let kinds: Vec<&str> = events.iter().map(|e| e.kind.as_str()).collect();
    assert!(
        kinds.iter().any(|k| k.contains("Liftoff")),
        "events: {kinds:?}"
    );
    assert!(
        kinds.iter().any(|k| k.contains("Landing")),
        "events: {kinds:?}"
    );

    // Run 2: cancel it.
    bridge.send(
        "c-run2",
        ClientRequest::RunMission {
            mission_id: MISSION_ID.into(),
        },
    );
    bridge.send(
        "c-cancel",
        ClientRequest::CancelRun {
            run_id: "run-2".into(),
        },
    );
    let cancelled = bridge.stdout.drain_until("run_cancelled");
    assert_eq!(cancelled.last().unwrap().kind, "run_cancelled");

    // Run 3: completes normally again.
    bridge.send(
        "c-run3",
        ClientRequest::RunMission {
            mission_id: MISSION_ID.into(),
        },
    );
    let frames3 = bridge.stdout.drain_until("run_completed");
    assert_eq!(frames3.last().unwrap().kind, "run_completed");

    // Clean shutdown.
    bridge.send("c-shutdown", ClientRequest::Shutdown);
    let status = bridge.child.wait().expect("bridge exits");
    assert!(status.success(), "bridge should exit cleanly");

    let mut stderr = String::new();
    bridge
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .ok();
    assert!(
        !stderr.contains("protocol_version"),
        "stderr must never contain protocol bytes: {stderr:?}"
    );
}

#[test]
fn malformed_input_fails_closed() {
    let mut bridge = Bridge::spawn();
    // A frame whose declared length exceeds the maximum: the decoder must
    // reject it and the bridge must emit an error, then exit.
    let oversize = (MAX_FRAME_BYTES as u32 + 1).to_le_bytes();
    bridge
        .child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&oversize)
        .unwrap();
    bridge
        .child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"x")
        .unwrap();
    bridge.child.stdin.as_mut().unwrap().flush().unwrap();

    let err = bridge.stdout.next();
    assert_eq!(err.kind, "error");
    drop(bridge.child.stdin.take());
    let status = bridge.child.wait().expect("bridge exits");
    assert!(status.success());
}

#[test]
fn request_before_hello_is_rejected() {
    let mut bridge = Bridge::spawn();
    bridge.send(
        "c-early",
        ClientRequest::RunMission {
            mission_id: MISSION_ID.into(),
        },
    );
    let err = bridge.stdout.next();
    match err.payload {
        ServerMessage::Error { code, .. } => assert_eq!(code, "invalid_request"),
        other => panic!("expected error, got {other:?}"),
    }
    drop(bridge.child.stdin.take());
    bridge.child.wait().ok();
}

#[test]
fn session_request_without_capability_is_rejected_and_closes() {
    let mut bridge = Bridge::spawn();
    // Offer no capabilities: the ack must be empty and any session request
    // must be refused as un-negotiated, then the bridge closes.
    assert!(negotiate(&mut bridge, Vec::new()).is_empty());

    bridge.send(
        "c-create",
        ClientRequest::CreateSession {
            snapshot: serde_json::json!({ "schema_version": 1 }),
        },
    );
    match bridge.stdout.next().payload {
        ServerMessage::Error { code, message } => {
            assert_eq!(code, "invalid_request");
            assert_eq!(message, "capability not negotiated: interactive-session/1");
        }
        other => panic!("expected error, got {other:?}"),
    }
    let status = bridge
        .child
        .wait()
        .expect("bridge exits after rejected session request");
    assert!(status.success());
}

#[test]
fn capability_is_negotiated_and_unknown_capabilities_ignored() {
    let mut bridge = Bridge::spawn();
    // Offer the real capability plus a bogus one: the ack echoes only the
    // supported capability; unknown strings are silently dropped.
    let acked = negotiate(
        &mut bridge,
        vec![CAP_SESSION.into(), "made-up-capability/9".into()],
    );
    assert_eq!(acked, vec![CAP_SESSION.to_string()]);

    bridge.send("c-shutdown", ClientRequest::Shutdown);
    assert!(bridge.child.wait().expect("bridge exits").success());
}

#[test]
fn stepped_session_matches_the_batch_reference_trace() {
    let mut bridge = Bridge::spawn();
    assert_eq!(
        negotiate(&mut bridge, vec![CAP_SESSION.into()]),
        vec![CAP_SESSION.to_string()]
    );

    // Batch run first: capture the canonical trace hash from the legacy path.
    bridge.send(
        "c-batch",
        ClientRequest::RunMission {
            mission_id: MISSION_ID.into(),
        },
    );
    let batch_hash = bridge
        .stdout
        .drain_until("run_completed")
        .into_iter()
        .find_map(|env| match env.payload {
            ServerMessage::RunCompleted { trace_hash, .. } => Some(trace_hash),
            _ => None,
        })
        .expect("batch run_completed");

    // Now create a session from the reference snapshot and step it to the end.
    let (snapshot, expected_config_hash) = reference_snapshot();
    bridge.send("c-create", ClientRequest::CreateSession { snapshot });
    let session_id = match bridge.stdout.next().payload {
        ServerMessage::SessionCreated {
            session_id,
            config_hash,
            schema_version,
        } => {
            assert_eq!(
                config_hash, expected_config_hash,
                "config_hash is snapshot-issued"
            );
            assert_eq!(schema_version, 1);
            session_id
        }
        other => panic!("expected session_created, got {other:?}"),
    };

    let (cursors, trace) = drive_to_completion(&mut bridge, &session_id, 10_000);

    // step_cursor is strictly monotone across advances.
    assert!(
        cursors.windows(2).all(|w| w[1] > w[0]),
        "step cursors must strictly increase: {cursors:?}"
    );

    // The reconstructed trace hashes to the manifest and completed hashes, and
    // to the batch trace — stepped and batch are bit-identical.
    let recomputed = trace_hash(&trace.channels, &trace.events);
    assert_eq!(
        recomputed, trace.manifest_hash,
        "manifest hash matches reconstruction"
    );
    assert_eq!(
        trace.completed_hash, trace.manifest_hash,
        "completed hash matches manifest"
    );
    assert_eq!(
        trace.completed_hash, batch_hash,
        "stepped trace equals batch trace"
    );

    // Launch-to-landing events survive into the completion trace.
    let kinds: Vec<&str> = trace.events.iter().map(|e| e.kind.as_str()).collect();
    assert!(
        kinds.iter().any(|k| k.contains("Liftoff")),
        "events: {kinds:?}"
    );
    assert!(
        kinds.iter().any(|k| k.contains("Landing")),
        "events: {kinds:?}"
    );

    bridge.send("c-shutdown", ClientRequest::Shutdown);
    assert!(bridge.child.wait().expect("bridge exits").success());
}

#[test]
fn chunk_size_does_not_change_the_trace_hash() {
    // Two sessions over the same snapshot, advanced with different step sizes,
    // must produce distinct ids, the same config_hash, and the same trace_hash.
    let mut bridge = Bridge::spawn();
    assert_eq!(
        negotiate(&mut bridge, vec![CAP_SESSION.into()]),
        vec![CAP_SESSION.to_string()]
    );
    let (snapshot, config_hash) = reference_snapshot();

    let mut complete = |step: u64, create_id: &str| -> (String, String) {
        bridge.send(
            create_id,
            ClientRequest::CreateSession {
                snapshot: snapshot.clone(),
            },
        );
        let (session_id, cfg) = match bridge.stdout.next().payload {
            ServerMessage::SessionCreated {
                session_id,
                config_hash,
                ..
            } => (session_id, config_hash),
            other => panic!("expected session_created, got {other:?}"),
        };
        assert_eq!(cfg, config_hash);
        let (_, trace) = drive_to_completion(&mut bridge, &session_id, step);
        (session_id, trace.completed_hash)
    };

    let (id_big, hash_big) = complete(10_000, "c-create-big");
    let (id_small, hash_small) = complete(2_500, "c-create-small");

    assert_ne!(id_big, id_small, "each session gets a fresh id");
    assert_eq!(
        hash_big, hash_small,
        "trace hash is invariant to advance chunk size"
    );

    bridge.send("c-shutdown", ClientRequest::Shutdown);
    assert!(bridge.child.wait().expect("bridge exits").success());
}

#[test]
fn advance_emits_exactly_one_update_and_then_pauses() {
    let mut bridge = Bridge::spawn();
    assert_eq!(
        negotiate(&mut bridge, vec![CAP_SESSION.into()]),
        vec![CAP_SESSION.to_string()]
    );
    let (snapshot, _) = reference_snapshot();
    bridge.send("c-create", ClientRequest::CreateSession { snapshot });
    let session_id = match bridge.stdout.next().payload {
        ServerMessage::SessionCreated { session_id, .. } => session_id,
        other => panic!("expected session_created, got {other:?}"),
    };

    // A single small advance yields exactly one StateUpdate (not completed).
    bridge.send(
        "c-adv",
        ClientRequest::AdvanceSteps {
            session_id: session_id.clone(),
            steps: 100,
        },
    );
    match bridge.stdout.next().payload {
        ServerMessage::StateUpdate {
            step_cursor,
            outcome,
            ..
        } => {
            assert_eq!(step_cursor, 100);
            assert!(matches!(
                outcome,
                ascent_visualizer_protocol::AdvanceStateOutcome::Advanced
            ));
        }
        other => panic!("expected state_update, got {other:?}"),
    }

    // No unsolicited frames follow: the very next frame answers a later request.
    bridge.send("c-list", ClientRequest::ListMissions);
    assert_eq!(
        bridge.stdout.next().kind,
        "mission_catalog",
        "session must pause between advances"
    );

    bridge.send("c-shutdown", ClientRequest::Shutdown);
    assert!(bridge.child.wait().expect("bridge exits").success());
}

#[test]
fn cancel_is_terminal_and_repeated_cancel_reacks() {
    let mut bridge = Bridge::spawn();
    assert_eq!(
        negotiate(&mut bridge, vec![CAP_SESSION.into()]),
        vec![CAP_SESSION.to_string()]
    );
    let (snapshot, _) = reference_snapshot();
    bridge.send("c-create", ClientRequest::CreateSession { snapshot });
    let session_id = match bridge.stdout.next().payload {
        ServerMessage::SessionCreated { session_id, .. } => session_id,
        other => panic!("expected session_created, got {other:?}"),
    };

    // Advance a little, then cancel mid-flight.
    bridge.send(
        "c-adv",
        ClientRequest::AdvanceSteps {
            session_id: session_id.clone(),
            steps: 500,
        },
    );
    assert_eq!(bridge.stdout.next().kind, "state_update");

    bridge.send(
        "c-cancel-1",
        ClientRequest::Cancel {
            session_id: session_id.clone(),
        },
    );
    let first = bridge.stdout.next();
    assert_eq!(first.kind, "session_cancelled");
    assert_eq!(first.request_id.as_deref(), Some("c-cancel-1"));

    // A repeated cancel re-acks, correlated to the repeat's message id.
    bridge.send(
        "c-cancel-2",
        ClientRequest::Cancel {
            session_id: session_id.clone(),
        },
    );
    let second = bridge.stdout.next();
    assert_eq!(second.kind, "session_cancelled");
    assert_eq!(second.request_id.as_deref(), Some("c-cancel-2"));

    // A cancelled session no longer advances.
    bridge.send(
        "c-adv-after",
        ClientRequest::AdvanceSteps {
            session_id,
            steps: 100,
        },
    );
    match bridge.stdout.next().payload {
        ServerMessage::Error { code, .. } => assert_eq!(code, "invalid_request"),
        other => panic!("expected error, got {other:?}"),
    }

    bridge.send("c-shutdown", ClientRequest::Shutdown);
    assert!(bridge.child.wait().expect("bridge exits").success());
}

#[test]
fn a_live_session_holds_the_single_run_slot() {
    let mut bridge = Bridge::spawn();
    assert_eq!(
        negotiate(&mut bridge, vec![CAP_SESSION.into()]),
        vec![CAP_SESSION.to_string()]
    );
    let (snapshot, _) = reference_snapshot();
    bridge.send(
        "c-create",
        ClientRequest::CreateSession {
            snapshot: snapshot.clone(),
        },
    );
    match bridge.stdout.next().payload {
        ServerMessage::SessionCreated { .. } => {}
        other => panic!("expected session_created, got {other:?}"),
    }

    // A batch run is refused while a session is live.
    bridge.send(
        "c-batch",
        ClientRequest::RunMission {
            mission_id: MISSION_ID.into(),
        },
    );
    match bridge.stdout.next().payload {
        ServerMessage::Error { code, message } => {
            assert_eq!(code, "invalid_request");
            assert_eq!(message, "a run is already active");
        }
        other => panic!("expected error, got {other:?}"),
    }

    // A second session is likewise refused.
    bridge.send("c-create-2", ClientRequest::CreateSession { snapshot });
    match bridge.stdout.next().payload {
        ServerMessage::Error { code, message } => {
            assert_eq!(code, "invalid_request");
            assert_eq!(message, "a run is already active");
        }
        other => panic!("expected error, got {other:?}"),
    }

    bridge.send("c-shutdown", ClientRequest::Shutdown);
    assert!(bridge.child.wait().expect("bridge exits").success());
}

#[test]
fn advance_step_bounds_are_enforced() {
    let mut bridge = Bridge::spawn();
    assert_eq!(
        negotiate(&mut bridge, vec![CAP_SESSION.into()]),
        vec![CAP_SESSION.to_string()]
    );
    let (snapshot, _) = reference_snapshot();
    bridge.send("c-create", ClientRequest::CreateSession { snapshot });
    let session_id = match bridge.stdout.next().payload {
        ServerMessage::SessionCreated { session_id, .. } => session_id,
        other => panic!("expected session_created, got {other:?}"),
    };

    for (mid, steps) in [("c-zero", 0u64), ("c-over", 10_001)] {
        bridge.send(
            mid,
            ClientRequest::AdvanceSteps {
                session_id: session_id.clone(),
                steps,
            },
        );
        match bridge.stdout.next().payload {
            ServerMessage::Error { code, message } => {
                assert_eq!(code, "invalid_request");
                assert!(message.contains("MAX_ADVANCE_STEPS"), "message: {message}");
            }
            other => panic!("expected error for steps={steps}, got {other:?}"),
        }
    }

    // The session survived the rejected advances and still steps normally.
    bridge.send(
        "c-adv-ok",
        ClientRequest::AdvanceSteps {
            session_id,
            steps: 100,
        },
    );
    assert_eq!(bridge.stdout.next().kind, "state_update");

    bridge.send("c-shutdown", ClientRequest::Shutdown);
    assert!(bridge.child.wait().expect("bridge exits").success());
}

#[test]
fn shutdown_mid_session_exits_cleanly() {
    let mut bridge = Bridge::spawn();
    assert_eq!(
        negotiate(&mut bridge, vec![CAP_SESSION.into()]),
        vec![CAP_SESSION.to_string()]
    );
    let (snapshot, _) = reference_snapshot();
    bridge.send("c-create", ClientRequest::CreateSession { snapshot });
    let session_id = match bridge.stdout.next().payload {
        ServerMessage::SessionCreated { session_id, .. } => session_id,
        other => panic!("expected session_created, got {other:?}"),
    };
    bridge.send(
        "c-adv",
        ClientRequest::AdvanceSteps {
            session_id,
            steps: 250,
        },
    );
    assert_eq!(bridge.stdout.next().kind, "state_update");

    bridge.send("c-shutdown", ClientRequest::Shutdown);
    let status = bridge.child.wait().expect("bridge exits mid-session");
    assert!(status.success());
}
