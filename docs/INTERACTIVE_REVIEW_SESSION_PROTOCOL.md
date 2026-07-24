# Interactive Review Session Protocol — Contract

Status: approved design. Additive over the existing stdio bridge. No wire break.

## 0. Scope

Adds a **stepped, live-controllable** review session wrapping the `RunSession` seam
(`CreateSession → AdvanceSteps → Cancel → Shutdown`). Pause = withhold `AdvanceSteps`.
Restart = a fresh `CreateSession` from the same snapshot.

Rust is the sole authority for physics, state, trace, and hashes. The client is a
renderer that requests steps and draws what it is told. The legacy v1 `RunMission`
batch flow is unchanged and untouched; this is a distinct additive capability.

## 1. Version negotiation / compatibility

Version is negotiated at the **frame** layer, but the frame `protocol_version` stays
`1`. The `FrameDecoder` fail-closes on any other frame version, so a hard bump would
strand v1 clients; the bump is a later, gated migration.

- **Capability handshake.** `Hello` carries an optional additive `capabilities: [..]`
  (absent ⇒ pure v1 client). The server answers `HelloAck` with the intersection it
  supports, e.g. `["interactive-session/1"]`.
- **Fail-closed gating.** A client that did not negotiate `interactive-session/1` and
  sends any session request gets `INVALID_REQUEST` ("capability not negotiated") and
  the connection closes.
- **Forward compat.** Unknown capability strings are ignored during negotiation.
  Unknown message `kind`s after negotiation are a hard `INVALID_REQUEST` + close
  (the internally-tagged enum already rejects them).
- **Later frame v2.** When the frame version is eventually bumped, this contract
  transfers verbatim; only the decoder's accepted-version set changes (accept `{1,2}`
  during migration, then drop `1`). Design now, execute later.

## 2. Client requests

All require a completed `Hello` **and** negotiated `interactive-session/1`.

| Kind | Payload | Notes |
|------|---------|-------|
| `CreateSession` | `{ snapshot: MissionSnapshot }` | **Inline immutable snapshot, always.** Carries the resolved staged vehicle + launch/env/integration inputs + provenance, so a *changed* Ascent vehicle launches without a server catalog. Server validates `schema_version` and fails closed on unknown. |
| `AdvanceSteps` | `{ session_id, steps: u64 }` | Fixed core-step count. No variable dt. Time-warp = larger `steps`; Pause = withhold. `steps` is bounded by `MAX_ADVANCE_STEPS` so one call cannot wedge the coordinator (see below). |
| `Cancel` | `{ session_id }` | Idempotent-with-ack (see §4). Drives the session to the `SessionCancelled` terminal state. |
| `Shutdown` | *(none)* | Cancels the active session, drains, exits. |

Deliberately absent from the wire: `Pause`, `Restart`, `Seek`, `SetSpeed`. Pause and
Restart are client behaviors, not server operations.

**`MAX_ADVANCE_STEPS = 10_000`.** `AdvanceSteps.steps` is a fixed core-step count in
`1..=MAX_ADVANCE_STEPS`. The cap bounds the worst-case work one request can pin on the
single coordinator thread; a client time-warps by issuing more calls, not by one
unbounded call. An `AdvanceSteps` with `steps > MAX_ADVANCE_STEPS` (or `steps == 0`) is
rejected with `INVALID_REQUEST` ("steps exceeds MAX_ADVANCE_STEPS") and the connection
**stays** — it is an operational error the client recovers from by re-requesting a
smaller count, not a protocol violation. The cap is a protocol constant, not a tuning
knob: both languages pin the same `10_000`.

## 3. Server messages

| Kind | Payload | Emitted when |
|------|---------|--------------|
| `SessionCreated` | `{ session_id, config_hash, schema_version }` | Once per valid `CreateSession`. `config_hash` is an **opaque Rust-issued identity string** — the client stores and echoes it, never parses or recomputes it. |
| `StateUpdate` | `{ session_id, outcome, step_cursor, sim_time_s, position_m, velocity_ms, attitude_wxyz, phase, stage_index, thrust_n, new_events }` | Once per `AdvanceSteps` that ran. **Latest state only:** current pose/velocity/attitude, current phase + active stage, current `thrust_n`, and only events crossed since the previous update. `outcome ∈ {advanced, completed}`. Floats as i64 IEEE-754 bit patterns. No trajectory channels here. |
| `SessionTraceManifest` | `{ session_id, trace_hash, sample_count, event_count, channels }` | Once, at normal completion, before the channel chunks. |
| `SessionTraceChunk` | `{ session_id, channel, sequence, samples }` | The full canonical trace, streamed in canonical channel order after the manifest. Session-specific message — does **not** reuse legacy `TraceChannelChunk` / its `run_id`. |
| `SessionTraceEvents` | `{ session_id, events }` | Once, after the channel chunks and before `SessionCompleted`. Delivers the **full, ordered flight event set** for the completed run so the client can reconstruct the canonical trace and recompute `trace_hash` — events are hashed into `trace_hash` alongside the channel samples, so the hash is not verifiable without them. Distinct from `StateUpdate.new_events`, which is only the incremental slice since the previous update. |
| `SessionCompleted` | `{ session_id, trace_hash }` | Once, terminating a **normal** completion after the chunks and events. A hash-backed trace exists iff this was emitted. Only path that carries the full canonical trace. |
| `SessionCancelled` | `{ session_id }` | Terminal state for a `Cancel`/`Shutdown` on a session that had not completed. **No trace, no `trace_hash`.** Never substituted by `SessionCompleted`. |

`StateUpdate.outcome` is `{advanced, completed}` only — cancellation is delivered by the
separate `SessionCancelled` message, never as an advance outcome.

`StateUpdate.thrust_n` is the **Rust-issued** instantaneous propulsion magnitude (newtons)
of the active stage at `step_cursor`, an i64 IEEE-754 bit pattern like the other floats.
It exists so the client can drive live plume rendering (intensity, length, opacity) from
authoritative physics. The client **must not** derive propulsion from mass, motor curves,
or timers itself: Rust owns the burn model, and the plume is a pure function of the
`thrust_n` it is told. A stage between burns reports `thrust_n = 0`.

## 4. Ordering & error rules

```
Hello ─▶ HelloAck(caps ⊇ interactive-session/1)
CreateSession ─▶ SessionCreated
AdvanceSteps ─▶ StateUpdate{advanced}        (0..N)
AdvanceSteps ─▶ StateUpdate{completed}
             ─▶ SessionTraceManifest ─▶ SessionTraceChunk* ─▶ SessionTraceEvents ─▶ SessionCompleted
   ── or ──
Cancel ─▶ SessionCancelled
```

**Ordering invariants (server-guaranteed):**
1. `SessionCreated` precedes every message for its `session_id`.
2. Exactly one `SessionCreated`; at most one terminal (`SessionCompleted` xor `SessionCancelled`) per session.
3. Every non-erroring `AdvanceSteps` produces exactly one `StateUpdate`, in request order; each `StateUpdate`'s `request_id` echoes its `AdvanceSteps` `message_id`.
4. After a terminal message, no further `StateUpdate` for that session.
5. The coordinator is the sole writer; frames never interleave.

**One active run, globally.** The bridge permits one active run of any kind at a time —
a legacy `RunMission` batch run and an interactive session share one slot.
FSM: `AwaitingHello → Idle → Busy`, entered by either `RunMission` or `CreateSession`,
returning to `Idle` on `RunCompleted`/`RunCancelled` or `SessionCompleted`/`SessionCancelled`.

**Cancellation is idempotent-with-ack.** A session has exactly **one terminal state
transition**: the first `Cancel` on an active session drives it into `SessionCancelled`
and that transition happens once. A repeated `Cancel` for that (now-terminal) session
does **not** re-transition any state — the session was already cancelled — but the server
still **re-emits `SessionCancelled`** as a pure acknowledgment, correlated to the repeat
request's `message_id`, never a silent no-op. Each emitted `SessionCancelled` echoes the
`message_id` of the `Cancel` it answers, so a client can match acknowledgment to request
even though only the first one changed state. `Cancel` never produces a `StateUpdate`.

**Error rules** (fail-closed; reuse existing `error_code`):

| Condition | Code | Connection |
|-----------|------|------------|
| Any request before `Hello` | `INVALID_REQUEST` | close |
| Session request without negotiated capability | `INVALID_REQUEST` | close |
| `CreateSession` with unknown `schema_version` | `INVALID_REQUEST` (names schema mismatch) | close |
| `CreateSession` or `RunMission` while `Busy` | `INVALID_REQUEST` "a run is already active" | stay |
| `AdvanceSteps` for unknown / non-active `session_id` | `INVALID_REQUEST` | stay |
| `AdvanceSteps` on a completed session | `INVALID_REQUEST` | stay |
| `AdvanceSteps` with `steps > MAX_ADVANCE_STEPS` or `steps == 0` | `INVALID_REQUEST` "steps exceeds MAX_ADVANCE_STEPS" | stay |
| `Cancel` on active session | *(not an error)* → `SessionCancelled` | stay |
| Repeated `Cancel` on cancelled session | *(not an error)* → re-emit `SessionCancelled` | stay |
| Duplicate `message_id` | `INVALID_REQUEST` "duplicate message id" | close |
| Framing / decode violation | `INVALID_REQUEST` "framing violation" | close |
| Stepper returns `Err` mid-advance | `RUN_FAILED` | end session, stay |

Protocol/handshake violations **close** the connection; in-session operational errors
return an error and **keep** it so the client can recover.

## 5. Fixtures (implement first)

Small, checked-in, language-agnostic data files loaded by both the Rust bridge tests
and the Unity client suite:

1. **Frame corpus** — one small `.frame` fixture per new `kind` (`CreateSession`,
   `AdvanceSteps`, `Cancel`, `SessionCreated`, `StateUpdate`, `SessionTraceManifest`,
   `SessionTraceChunk`, `SessionTraceEvents`, `SessionCompleted`, `SessionCancelled`):
   exact length-prefixed bytes ↔ decoded-JSON sidecar; round-trip both directions. The
   `CreateSession` fixture uses a **small synthetic `MissionSnapshot`**, not the full
   flight.
2. **Handshake matrix** — capability granted / absent-then-rejected / unknown-ignored,
   as `Hello`/`HelloAck` fixture pairs plus a `matrix.json` naming the negotiated set for
   each scenario.
3. **Error fixtures** — one `Error` frame per erroring §4 row, pinning `{code,
   message-prefix}` and close/stay in an `index.json` (including the `MAX_ADVANCE_STEPS`
   rejection).

No giant golden transcript. No Unity-side `config_hash` recomputation — `config_hash`
is an opaque echoed string.

## 6. Bridge subprocess tests (after fixtures)

Black-box tests spawning the real bridge binary over stdio:

1. **Handshake gating** — `CreateSession` before `Hello`, and without the capability, are rejected fail-closed.
2. **Full stepped run** — `Hello → CreateSession → AdvanceSteps` in small chunks to completion. Assert: one `SessionCreated`; one `StateUpdate` per `AdvanceSteps` carrying latest-state-only (no channels); monotone `step_cursor`/`sim_time_s`; on completion `SessionTraceManifest` → chunks → exactly one `SessionCompleted`. **The expected `trace_hash` is generated by running the reference trace directly via Rust inside the test and compared to the wire value — no hard-coded constant.**
3. **Chunk-invariance** — `steps=1` vs `steps=large` yields identical accumulated trajectory and identical `SessionCompleted.trace_hash`.
4. **Pause** — withhold `AdvanceSteps`, then resume; run continues bit-identically; no unsolicited `StateUpdate` while idle.
5. **Restart** — after completion, `CreateSession` again with the same snapshot; new `session_id`, same opaque `config_hash`, identical final `trace_hash`.
6. **Cancel mid-run** — `Cancel` after some steps ⇒ `SessionCancelled`, no `trace_hash`, no channels, no `SessionCompleted`. A repeated `Cancel` re-emits `SessionCancelled`.
7. **Global single-run enforcement** — `RunMission` then `CreateSession` ⇒ rejected; `CreateSession` then `RunMission` ⇒ rejected; the first run is unaffected.
8. **Fail-closed decoder** — an oversized/garbage frame ⇒ `INVALID_REQUEST` "framing violation" + exit; no further frames processed.
9. **Shutdown** — `Shutdown` mid-session cancels, drains, exits 0; no `StateUpdate` after.
10. **Backward-compat guard** — a pure v1 client (no capability, `RunMission` batch flow) still completes unchanged against the same binary.

## Implementation order

Fixtures (§5) first, then bridge runtime code (§6). Capability negotiation stays under
frame v1.
