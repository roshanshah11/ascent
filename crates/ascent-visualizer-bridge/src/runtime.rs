//! Read-only supervised bridge runtime.
//!
//! The bridge speaks visualizer protocol v1 over stdio. It loads exactly one
//! in-memory reference mission, runs the existing staged 6-DOF trace, and
//! streams a deterministic, hash-valid result. It accepts no path, URL, Ascent
//! command, or document mutation — the only input surface is the closed set of
//! [`ClientRequest`] kinds.

use std::collections::HashSet;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use ascent_domain::reference::{black_brant_ix_reference, BLACK_BRANT_IX_MISSION_ID};
use ascent_sim::reference_trace::trace_reference_mission;
use ascent_sim::run_session::{
    AdvanceOutcome, MissionSnapshot, RunSession, SNAPSHOT_SCHEMA_VERSION,
};
use ascent_sim::sixdof::{FlightPhase, SixDofResult};
use ascent_visualizer_protocol::messages::error_code;
use ascent_visualizer_protocol::{
    encode_frame, AdvanceStateOutcome, Channel, ChannelSpec, ClientRequest, Envelope, FrameDecoder,
    MissionEntry, ServerMessage, TraceEvent, CAP_INTERACTIVE_SESSION_V1, MAX_ADVANCE_STEPS,
    MAX_CHUNK_SAMPLES, MAX_FRAME_BYTES,
};

/// Upper bound on streamed samples per channel. The full flight is decimated to
/// this many points (last sample always retained) so a run stays bounded while
/// still spanning launch to landing.
const MAX_TRACE_SAMPLES: usize = 4_096;

/// Canonical mission title shown in the catalog.
const MISSION_TITLE: &str = "NASA Black Brant IX (reference)";

/// A fully reconstructed, hashable trace ready to stream.
struct Trace {
    channels: Vec<Channel>,
    events: Vec<TraceEvent>,
    hash: String,
    sample_count: usize,
}

/// Load the reference mission and build its deterministic streamed trace.
fn build_trace() -> Result<Trace, String> {
    let mission = black_brant_ix_reference().map_err(|e| e.to_string())?;
    let result = trace_reference_mission(&mission)?;
    Ok(assemble_trace(&result))
}

/// Decimate a completed 6-DOF result into the canonical streamed trace
/// (channels in canonical order, events, and the hash over both). Shared by the
/// legacy batch run and the interactive session so a session stepped to
/// completion streams a byte-identical, identically-hashed trace.
fn assemble_trace(result: &SixDofResult) -> Trace {
    let history = &result.history;
    let stride = history.len().div_ceil(MAX_TRACE_SAMPLES).max(1);
    let mut idx: Vec<usize> = (0..history.len()).step_by(stride).collect();
    if let Some(&last) = idx.last() {
        if last != history.len() - 1 {
            idx.push(history.len() - 1);
        }
    }

    let pick = |f: &dyn Fn(&ascent_sim::sixdof::SixDofSample) -> f64| -> Vec<f64> {
        idx.iter().map(|&i| f(&history[i])).collect()
    };
    let channels = vec![
        Channel {
            name: "time_s".into(),
            samples: pick(&|s| s.time_s),
        },
        Channel {
            name: "position_x_m".into(),
            samples: pick(&|s| s.position_m[0]),
        },
        Channel {
            name: "position_y_m".into(),
            samples: pick(&|s| s.position_m[1]),
        },
        Channel {
            name: "position_z_m".into(),
            samples: pick(&|s| s.position_m[2]),
        },
        Channel {
            name: "velocity_x_ms".into(),
            samples: pick(&|s| s.velocity_ms[0]),
        },
        Channel {
            name: "velocity_y_ms".into(),
            samples: pick(&|s| s.velocity_ms[1]),
        },
        Channel {
            name: "velocity_z_ms".into(),
            samples: pick(&|s| s.velocity_ms[2]),
        },
    ];
    let events: Vec<TraceEvent> = result
        .summary
        .events
        .iter()
        .map(|e| TraceEvent {
            kind: e.kind.clone(),
            t_s: e.t_s,
            altitude_m: e.altitude_m,
            velocity_ms: e.velocity_ms,
        })
        .collect();
    let hash = ascent_visualizer_protocol::trace_hash(&channels, &events);
    Trace {
        channels,
        events,
        hash,
        sample_count: idx.len(),
    }
}

/// The single mission this bridge can run.
fn mission_catalog() -> Vec<MissionEntry> {
    vec![MissionEntry {
        mission_id: BLACK_BRANT_IX_MISSION_ID.to_string(),
        title: MISSION_TITLE.to_string(),
    }]
}

/// Events funneled to the single-threaded coordinator.
enum Ev {
    Client(Box<Envelope<ClientRequest>>),
    ClientError(String),
    Emit(Box<Envelope<ServerMessage>>),
    RunDone(String),
    Eof,
}

/// Coordinator negotiation / run state.
#[derive(PartialEq)]
enum State {
    AwaitingHello,
    Idle,
    Running,
}

struct ActiveRun {
    run_id: String,
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// Terminal disposition of an interactive session. Retained after the session
/// leaves the active slot so later `AdvanceSteps`/`Cancel` for that id are
/// answered precisely (idempotent cancel ack, completed-session rejection)
/// rather than as an unknown id.
#[derive(PartialEq)]
enum Terminal {
    Completed,
    Cancelled,
    Failed,
}

/// The one interactive session the bridge tracks. Stepped inline on the
/// coordinator thread — the sole frame writer — so `AdvanceSteps` replies stay
/// in request order and never interleave.
struct SessionSlot {
    session_id: String,
    run: RunSession,
    /// Count of events already delivered via `StateUpdate.new_events`.
    reported_events: usize,
    /// `None` while the session is live; `Some` once terminal.
    terminal: Option<Terminal>,
}

/// Canonical wire vocabulary for a flight phase. Rust owns the phase; the client
/// renders the string it is told.
fn phase_str(phase: FlightPhase) -> &'static str {
    match phase {
        FlightPhase::Pad => "pad",
        FlightPhase::Rail => "rail",
        FlightPhase::Ascent => "ascent",
        FlightPhase::Descent => "descent",
        FlightPhase::Grounded => "grounded",
    }
}

/// Serve the protocol over the given reader/writer until shutdown or EOF.
///
/// `reader` must be owned (it is moved onto a dedicated thread). All protocol
/// frames are written to `writer` by this thread alone, so stdout carries only
/// frames and never interleaves.
pub fn serve<R, W>(reader: R, mut writer: W) -> std::io::Result<()>
where
    R: Read + Send + 'static,
    W: Write,
{
    let (tx, rx) = mpsc::channel::<Ev>();

    // Reader thread: decode frames, fail closed on any framing violation.
    let reader_tx = tx.clone();
    thread::spawn(move || {
        let mut reader = reader;
        let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    let _ = reader_tx.send(Ev::Eof);
                    return;
                }
                Ok(n) => match decoder.push(&buf[..n]) {
                    Ok(envelopes) => {
                        for env in envelopes {
                            if reader_tx.send(Ev::Client(Box::new(env))).is_err() {
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = reader_tx.send(Ev::ClientError(e.to_string()));
                        return;
                    }
                },
                Err(_) => {
                    let _ = reader_tx.send(Ev::Eof);
                    return;
                }
            }
        }
    });

    let mut state = State::AwaitingHello;
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut active: Option<ActiveRun> = None;
    let mut session: Option<SessionSlot> = None;
    let mut msg_counter: u64 = 0;
    let mut run_counter: u64 = 0;
    let mut session_counter: u64 = 0;
    // Set once the client negotiates `interactive-session/1` in its Hello.
    let mut negotiated_session = false;

    let next_id = |counter: &mut u64| {
        *counter += 1;
        format!("srv-{counter}")
    };

    let write_frame = |writer: &mut W,
                       msg: ServerMessage,
                       request_id: Option<String>,
                       counter: &mut u64|
     -> std::io::Result<()> {
        let env = Envelope::new(next_id(counter), request_id, msg);
        let frame = encode_frame(&env).expect("server frame encodes");
        writer.write_all(&frame)?;
        writer.flush()
    };

    while let Ok(ev) = rx.recv() {
        match ev {
            Ev::Emit(env) => {
                let frame = encode_frame(&env).expect("server frame encodes");
                writer.write_all(&frame)?;
                writer.flush()?;
            }
            Ev::RunDone(run_id) => {
                if active.as_ref().is_some_and(|a| a.run_id == run_id) {
                    let run = active.take().unwrap();
                    let _ = run.handle.join();
                    state = State::Idle;
                }
            }
            Ev::Eof => {
                if let Some(run) = active.take() {
                    run.cancel.store(true, Ordering::SeqCst);
                    let _ = run.handle.join();
                }
                break;
            }
            Ev::ClientError(reason) => {
                write_frame(
                    &mut writer,
                    ServerMessage::Error {
                        code: error_code::INVALID_REQUEST.into(),
                        message: format!("framing violation: {reason}"),
                    },
                    None,
                    &mut msg_counter,
                )?;
                if let Some(run) = active.take() {
                    run.cancel.store(true, Ordering::SeqCst);
                    let _ = run.handle.join();
                }
                break;
            }
            Ev::Client(env) => {
                let request_id = Some(env.message_id.clone());
                if !seen_ids.insert(env.message_id.clone()) {
                    write_frame(
                        &mut writer,
                        ServerMessage::Error {
                            code: error_code::INVALID_REQUEST.into(),
                            message: "duplicate message id".into(),
                        },
                        request_id,
                        &mut msg_counter,
                    )?;
                    if let Some(run) = active.take() {
                        run.cancel.store(true, Ordering::SeqCst);
                        let _ = run.handle.join();
                    }
                    break;
                }

                let error = |writer: &mut W, code: &str, message: &str, counter: &mut u64| {
                    write_frame(
                        writer,
                        ServerMessage::Error {
                            code: code.into(),
                            message: message.into(),
                        },
                        request_id.clone(),
                        counter,
                    )
                };

                match (&state, env.payload) {
                    (
                        State::AwaitingHello,
                        ClientRequest::Hello {
                            protocol_version,
                            capabilities: offered,
                            ..
                        },
                    ) => {
                        if protocol_version != ascent_visualizer_protocol::PROTOCOL_VERSION {
                            error(
                                &mut writer,
                                error_code::PROTOCOL_MISMATCH,
                                "unsupported protocol version",
                                &mut msg_counter,
                            )?;
                            break;
                        }
                        // Additive capability handshake: intersect the client's
                        // offer with the one session capability this build
                        // supports. Unknown capability strings are ignored.
                        negotiated_session =
                            offered.iter().any(|c| c == CAP_INTERACTIVE_SESSION_V1);
                        let negotiated = if negotiated_session {
                            vec![CAP_INTERACTIVE_SESSION_V1.to_string()]
                        } else {
                            Vec::new()
                        };
                        write_frame(
                            &mut writer,
                            ServerMessage::HelloAck {
                                server: "ascent-visualizer-bridge".into(),
                                protocol_version: ascent_visualizer_protocol::PROTOCOL_VERSION,
                                capabilities: negotiated,
                            },
                            request_id,
                            &mut msg_counter,
                        )?;
                        state = State::Idle;
                    }
                    (State::AwaitingHello, _) => {
                        error(
                            &mut writer,
                            error_code::INVALID_REQUEST,
                            "request received before Hello",
                            &mut msg_counter,
                        )?;
                        break;
                    }
                    (_, ClientRequest::Hello { .. }) => {
                        error(
                            &mut writer,
                            error_code::INVALID_REQUEST,
                            "hello already completed",
                            &mut msg_counter,
                        )?;
                        break;
                    }
                    (_, ClientRequest::ListMissions) => {
                        write_frame(
                            &mut writer,
                            ServerMessage::MissionCatalog {
                                missions: mission_catalog(),
                            },
                            request_id,
                            &mut msg_counter,
                        )?;
                    }
                    (State::Idle, ClientRequest::RunMission { mission_id }) => {
                        if session.as_ref().is_some_and(|s| s.terminal.is_none()) {
                            // A live interactive session holds the single run slot.
                            error(
                                &mut writer,
                                error_code::INVALID_REQUEST,
                                "a run is already active",
                                &mut msg_counter,
                            )?;
                        } else if mission_id != BLACK_BRANT_IX_MISSION_ID {
                            error(
                                &mut writer,
                                error_code::MISSION_NOT_FOUND,
                                "unknown mission id",
                                &mut msg_counter,
                            )?;
                        } else {
                            run_counter += 1;
                            let run_id = format!("run-{run_counter}");
                            let cancel = Arc::new(AtomicBool::new(false));
                            let worker_tx = tx.clone();
                            let worker_cancel = cancel.clone();
                            let worker_run_id = run_id.clone();
                            let worker_req = request_id.clone();
                            let handle = thread::spawn(move || {
                                stream_run(worker_run_id, worker_req, worker_cancel, worker_tx);
                            });
                            active = Some(ActiveRun {
                                run_id,
                                cancel,
                                handle,
                            });
                            state = State::Running;
                        }
                    }
                    (State::Running, ClientRequest::RunMission { .. }) => {
                        error(
                            &mut writer,
                            error_code::INVALID_REQUEST,
                            "a run is already active",
                            &mut msg_counter,
                        )?;
                    }
                    (State::Running, ClientRequest::CancelRun { .. }) => {
                        if let Some(run) = active.as_ref() {
                            run.cancel.store(true, Ordering::SeqCst);
                        }
                    }
                    (State::Idle, ClientRequest::CancelRun { .. }) => {
                        error(
                            &mut writer,
                            error_code::INVALID_REQUEST,
                            "no active run to cancel",
                            &mut msg_counter,
                        )?;
                    }
                    (_, ClientRequest::CreateSession { snapshot }) => {
                        if !negotiated_session {
                            error(
                                &mut writer,
                                error_code::INVALID_REQUEST,
                                "capability not negotiated: interactive-session/1",
                                &mut msg_counter,
                            )?;
                            break;
                        }
                        let session_busy = session.as_ref().is_some_and(|s| s.terminal.is_none());
                        if active.is_some() || session_busy {
                            error(
                                &mut writer,
                                error_code::INVALID_REQUEST,
                                "a run is already active",
                                &mut msg_counter,
                            )?;
                        } else if snapshot.get("schema_version").and_then(|v| v.as_u64())
                            != Some(SNAPSHOT_SCHEMA_VERSION as u64)
                        {
                            // Validate schema before a full deserialize: an
                            // unknown schema need not even shape-match, and must
                            // fail closed rather than be reinterpreted.
                            error(
                                &mut writer,
                                error_code::INVALID_REQUEST,
                                "unsupported snapshot schema_version",
                                &mut msg_counter,
                            )?;
                            break;
                        } else {
                            match serde_json::from_value::<MissionSnapshot>(snapshot)
                                .map_err(|e| e.to_string())
                                .and_then(|snap| RunSession::create(&snap))
                            {
                                Ok(run) => {
                                    session_counter += 1;
                                    let session_id = format!("sess-{session_counter}");
                                    let config_hash = run.config_hash().to_string();
                                    write_frame(
                                        &mut writer,
                                        ServerMessage::SessionCreated {
                                            session_id: session_id.clone(),
                                            config_hash,
                                            schema_version: SNAPSHOT_SCHEMA_VERSION,
                                        },
                                        request_id,
                                        &mut msg_counter,
                                    )?;
                                    session = Some(SessionSlot {
                                        session_id,
                                        run,
                                        reported_events: 0,
                                        terminal: None,
                                    });
                                }
                                Err(_) => {
                                    error(
                                        &mut writer,
                                        error_code::INVALID_REQUEST,
                                        "invalid snapshot payload",
                                        &mut msg_counter,
                                    )?;
                                    break;
                                }
                            }
                        }
                    }
                    (_, ClientRequest::AdvanceSteps { session_id, steps }) => {
                        if !negotiated_session {
                            error(
                                &mut writer,
                                error_code::INVALID_REQUEST,
                                "capability not negotiated: interactive-session/1",
                                &mut msg_counter,
                            )?;
                            break;
                        }
                        if !session.as_ref().is_some_and(|s| s.session_id == session_id) {
                            error(
                                &mut writer,
                                error_code::INVALID_REQUEST,
                                "unknown session id",
                                &mut msg_counter,
                            )?;
                        } else {
                            let slot = session.as_mut().unwrap();
                            match slot.terminal {
                                Some(Terminal::Completed) => error(
                                    &mut writer,
                                    error_code::INVALID_REQUEST,
                                    "session already completed",
                                    &mut msg_counter,
                                )?,
                                // Cancelled / failed: the id is known but no
                                // longer active.
                                Some(_) => error(
                                    &mut writer,
                                    error_code::INVALID_REQUEST,
                                    "unknown session id",
                                    &mut msg_counter,
                                )?,
                                None if steps == 0 || steps > MAX_ADVANCE_STEPS => error(
                                    &mut writer,
                                    error_code::INVALID_REQUEST,
                                    &format!(
                                        "steps exceeds MAX_ADVANCE_STEPS ({MAX_ADVANCE_STEPS})"
                                    ),
                                    &mut msg_counter,
                                )?,
                                None => match slot.run.advance(steps) {
                                    Err(reason) => {
                                        slot.terminal = Some(Terminal::Failed);
                                        error(
                                            &mut writer,
                                            error_code::RUN_FAILED,
                                            &format!(
                                                "stepper returned an error mid-advance: {reason}"
                                            ),
                                            &mut msg_counter,
                                        )?;
                                    }
                                    Ok(outcome) => {
                                        let now = slot.run.state();
                                        let all = slot.run.events();
                                        let new_events: Vec<TraceEvent> = all
                                            [slot.reported_events..]
                                            .iter()
                                            .map(|e| TraceEvent {
                                                kind: e.kind.clone(),
                                                t_s: e.t_s,
                                                altitude_m: e.altitude_m,
                                                velocity_ms: e.velocity_ms,
                                            })
                                            .collect();
                                        slot.reported_events = all.len();
                                        let wire_outcome =
                                            if matches!(outcome, AdvanceOutcome::Completed) {
                                                AdvanceStateOutcome::Completed
                                            } else {
                                                AdvanceStateOutcome::Advanced
                                            };
                                        write_frame(
                                            &mut writer,
                                            ServerMessage::StateUpdate {
                                                session_id: session_id.clone(),
                                                outcome: wire_outcome,
                                                step_cursor: now.step_cursor,
                                                sim_time_s: now.sim_time_s.to_bits() as i64,
                                                position_m: bits3(now.position_m),
                                                velocity_ms: bits3(now.velocity_ms),
                                                attitude_wxyz: bits4(now.attitude_wxyz),
                                                phase: phase_str(now.phase).to_string(),
                                                stage_index: now.stage_index,
                                                thrust_n: now.thrust_n.to_bits() as i64,
                                                new_events,
                                            },
                                            request_id.clone(),
                                            &mut msg_counter,
                                        )?;
                                        if matches!(outcome, AdvanceOutcome::Completed) {
                                            let trace = assemble_trace(&slot.run.result());
                                            stream_session_trace(
                                                &mut writer,
                                                &session_id,
                                                &trace,
                                                &request_id,
                                                &mut msg_counter,
                                            )?;
                                            slot.terminal = Some(Terminal::Completed);
                                        }
                                    }
                                },
                            }
                        }
                    }
                    (_, ClientRequest::Cancel { session_id }) => {
                        if !negotiated_session {
                            error(
                                &mut writer,
                                error_code::INVALID_REQUEST,
                                "capability not negotiated: interactive-session/1",
                                &mut msg_counter,
                            )?;
                            break;
                        }
                        if !session.as_ref().is_some_and(|s| s.session_id == session_id) {
                            error(
                                &mut writer,
                                error_code::INVALID_REQUEST,
                                "unknown session id",
                                &mut msg_counter,
                            )?;
                        } else {
                            let slot = session.as_mut().unwrap();
                            match slot.terminal {
                                // First cancel of a live session: the single
                                // terminal transition. Repeated cancel of an
                                // already-cancelled session: re-emit as a pure
                                // ack, correlated to this request.
                                None | Some(Terminal::Cancelled) => {
                                    if slot.terminal.is_none() {
                                        slot.run.cancel();
                                        slot.terminal = Some(Terminal::Cancelled);
                                    }
                                    write_frame(
                                        &mut writer,
                                        ServerMessage::SessionCancelled {
                                            session_id: session_id.clone(),
                                        },
                                        request_id,
                                        &mut msg_counter,
                                    )?;
                                }
                                // Completed / failed: not a cancellable state.
                                Some(_) => error(
                                    &mut writer,
                                    error_code::INVALID_REQUEST,
                                    "unknown session id",
                                    &mut msg_counter,
                                )?,
                            }
                        }
                    }
                    (_, ClientRequest::Shutdown) => {
                        if let Some(run) = active.take() {
                            run.cancel.store(true, Ordering::SeqCst);
                            let _ = run.handle.join();
                        }
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Worker body for one run: emit `run_started`, build and stream the trace in
/// canonical channel order, then `run_completed` — or `run_cancelled` if the
/// cancel flag is observed. Every message is sent to the coordinator, which is
/// the sole writer.
fn stream_run(run_id: String, request_id: Option<String>, cancel: Arc<AtomicBool>, tx: Sender<Ev>) {
    let mut seq = 0u64;
    let mut emit = |msg: ServerMessage| {
        seq += 1;
        let env = Envelope::new(format!("{run_id}-{seq}"), request_id.clone(), msg);
        tx.send(Ev::Emit(Box::new(env))).is_ok()
    };

    if !emit(ServerMessage::RunStarted {
        run_id: run_id.clone(),
    }) {
        return;
    }

    let trace = match build_trace() {
        Ok(trace) => trace,
        Err(reason) => {
            emit(ServerMessage::Error {
                code: error_code::RUN_FAILED.into(),
                message: reason,
            });
            let _ = tx.send(Ev::RunDone(run_id));
            return;
        }
    };

    if cancel.load(Ordering::SeqCst) {
        emit(ServerMessage::RunCancelled {
            run_id: run_id.clone(),
        });
        let _ = tx.send(Ev::RunDone(run_id));
        return;
    }

    let _ = emit(ServerMessage::RunProgress {
        run_id: run_id.clone(),
        fraction: 0.5,
    });
    emit(ServerMessage::TraceManifest {
        run_id: run_id.clone(),
        trace_hash: trace.hash.clone(),
        sample_count: trace.sample_count,
        event_count: trace.events.len(),
        channels: trace
            .channels
            .iter()
            .map(|c| ChannelSpec {
                name: c.name.clone(),
                sample_count: c.samples.len(),
            })
            .collect(),
    });

    for channel in &trace.channels {
        for (sequence, chunk) in channel.samples.chunks(MAX_CHUNK_SAMPLES).enumerate() {
            if cancel.load(Ordering::SeqCst) {
                emit(ServerMessage::RunCancelled {
                    run_id: run_id.clone(),
                });
                let _ = tx.send(Ev::RunDone(run_id));
                return;
            }
            emit(ServerMessage::TraceChannelChunk {
                run_id: run_id.clone(),
                channel: channel.name.clone(),
                sequence: sequence as u32,
                samples: chunk.iter().map(|sample| sample.to_bits() as i64).collect(),
            });
        }
    }

    emit(ServerMessage::TraceEvents {
        run_id: run_id.clone(),
        events: trace.events.clone(),
    });
    emit(ServerMessage::RunCompleted {
        run_id: run_id.clone(),
        trace_hash: trace.hash,
    });
    let _ = tx.send(Ev::RunDone(run_id));
}

/// Reinterpret a 3-vector of `f64` as IEEE-754 bit patterns for the wire.
fn bits3(v: [f64; 3]) -> [i64; 3] {
    [
        v[0].to_bits() as i64,
        v[1].to_bits() as i64,
        v[2].to_bits() as i64,
    ]
}

/// Reinterpret a 4-vector of `f64` (a quaternion) as IEEE-754 bit patterns.
fn bits4(v: [f64; 4]) -> [i64; 4] {
    [
        v[0].to_bits() as i64,
        v[1].to_bits() as i64,
        v[2].to_bits() as i64,
        v[3].to_bits() as i64,
    ]
}

/// Stream a completed session's trace: manifest, then channel chunks in
/// canonical order, then the full event set, then `SessionCompleted`. Written
/// inline on the coordinator thread (the sole writer), so these frames follow
/// the completing `StateUpdate` in order and share its `msg_counter` sequence.
/// Mirrors [`stream_run`] but uses the session-keyed message family, so a
/// session stepped to completion streams the same bytes as the batch run.
fn stream_session_trace<W: Write>(
    writer: &mut W,
    session_id: &str,
    trace: &Trace,
    request_id: &Option<String>,
    counter: &mut u64,
) -> std::io::Result<()> {
    let emit = |writer: &mut W, msg: ServerMessage, counter: &mut u64| -> std::io::Result<()> {
        *counter += 1;
        let env = Envelope::new(format!("srv-{counter}"), request_id.clone(), msg);
        let frame = encode_frame(&env).expect("server frame encodes");
        writer.write_all(&frame)?;
        writer.flush()
    };

    emit(
        writer,
        ServerMessage::SessionTraceManifest {
            session_id: session_id.to_string(),
            trace_hash: trace.hash.clone(),
            sample_count: trace.sample_count,
            event_count: trace.events.len(),
            channels: trace
                .channels
                .iter()
                .map(|c| ChannelSpec {
                    name: c.name.clone(),
                    sample_count: c.samples.len(),
                })
                .collect(),
        },
        counter,
    )?;

    for channel in &trace.channels {
        for (sequence, chunk) in channel.samples.chunks(MAX_CHUNK_SAMPLES).enumerate() {
            emit(
                writer,
                ServerMessage::SessionTraceChunk {
                    session_id: session_id.to_string(),
                    channel: channel.name.clone(),
                    sequence: sequence as u32,
                    samples: chunk.iter().map(|sample| sample.to_bits() as i64).collect(),
                },
                counter,
            )?;
        }
    }

    emit(
        writer,
        ServerMessage::SessionTraceEvents {
            session_id: session_id.to_string(),
            events: trace.events.clone(),
        },
        counter,
    )?;
    emit(
        writer,
        ServerMessage::SessionCompleted {
            session_id: session_id.to_string(),
            trace_hash: trace.hash.clone(),
        },
        counter,
    )
}
