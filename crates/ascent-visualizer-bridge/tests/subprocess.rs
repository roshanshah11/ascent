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

#[test]
fn bridge_streams_deterministic_trace_and_supports_cancel_and_shutdown() {
    let mut bridge = Bridge::spawn();

    // Negotiate.
    bridge.send(
        "c-hello",
        ClientRequest::Hello {
            client: "test".into(),
            protocol_version: 1,
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
