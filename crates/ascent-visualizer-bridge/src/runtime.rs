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
use ascent_visualizer_protocol::messages::error_code;
use ascent_visualizer_protocol::{
    encode_frame, Channel, ChannelSpec, ClientRequest, Envelope, FrameDecoder, MissionEntry,
    ServerMessage, TraceEvent, MAX_CHUNK_SAMPLES, MAX_FRAME_BYTES,
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
    Ok(Trace {
        channels,
        events,
        hash,
        sample_count: idx.len(),
    })
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
    let mut msg_counter: u64 = 0;
    let mut run_counter: u64 = 0;

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
                            protocol_version, ..
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
                        write_frame(
                            &mut writer,
                            ServerMessage::HelloAck {
                                server: "ascent-visualizer-bridge".into(),
                                protocol_version: ascent_visualizer_protocol::PROTOCOL_VERSION,
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
                            "hello required before any other request",
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
                        if mission_id != BLACK_BRANT_IX_MISSION_ID {
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
                samples: chunk.to_vec(),
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
