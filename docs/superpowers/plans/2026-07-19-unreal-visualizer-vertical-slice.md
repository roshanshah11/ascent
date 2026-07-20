# Unreal Mission Visualizer Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a polished Unreal Engine 5.8 client that launches Ascent's Rust core, runs an evidence-graded NASA Black Brant IX reference mission, and delivers realistic engineering playback plus cinematic export.

**Architecture:** Rust remains authoritative and runs in a supervised `ascent-visualizer-bridge` child process. A versioned framed-stdio protocol streams deterministic `FlightTrace` data to a read-only Unreal client, which owns playback, rendering, inspection, cameras, and export.

**Tech Stack:** Rust 1.97, Serde JSON, SHA-256, Unreal Engine 5.8 C++, Slate/UMG, Niagara, Procedural Mesh Component, Movie Render Queue, Unreal Automation Framework.

## Global Constraints

- Begin only after the active v0.6 work is consolidated and its required gates pass from a clean snapshot.
- Preserve Rust as the only engineering and physics authority.
- Do not delete or redesign Tauri, React, or `ascent-mcp`.
- Do not add runtime network access.
- Unreal render and playback code dispatches zero Ascent commands.
- All NASA vehicle data and imagery must record provenance, rights, and evidence grade.
- Interactive mode must run on the M3 Pro with 18 GB memory; cinematic mode may render offline.
- All repository shell commands use the `rtk` prefix unless an RTK limitation requires one raw fallback.

---

## Step 1 — Establish the evidence-graded Black Brant IX reference

**Files:**

- Create: `data/reference/nasa-black-brant-ix/manifest.json`
- Create: `data/reference/nasa-black-brant-ix/vehicle.json`
- Create: `data/reference/nasa-black-brant-ix/SOURCES.md`
- Create: `examples/nasa-black-brant-ix.ascent`
- Create: `crates/ascent-domain/tests/black_brant_ix.rs`
- Modify: `crates/ascent-domain/src/vehicle.rs`

**Interfaces:**

- Consumes: existing `Vehicle`, `Part`, `PartKind`, stage-coupler, motor, and evidence types.
- Produces: `pub fn black_brant_ix_reference() -> Result<Vehicle, String>` and a deterministic `.ascent` reference project.

- [ ] **Step 1: Freeze authoritative sources before entering values**

Use the NASA Sounding Rockets User Handbook as the primary configuration source, official NASA imagery for markings and proportions, and a bounded USGS 3DEP White Sands terrain source. `manifest.json` must record `source_url`, `retrieved_at`, `sha256`, `rights`, `fields`, and `grade`, where `grade` is exactly `authoritative`, `derived`, or `approximate`.

- [ ] **Step 2: Write the failing fixture tests**

Test that the reference has two ordered stages, one separating coupler, positive finite dimensions and masses, valid source hashes, an evidence grade for every engineering field, and a stable serialized representation.

Run:

```bash
rtk cargo test -p ascent-domain --test black_brant_ix
```

Expected: FAIL because `black_brant_ix_reference` and its fixtures do not exist.

- [ ] **Step 3: Implement the reference constructor and checked-in project**

Map sourced geometry and mass values directly. Derive only values whose formulas and inputs are recorded in `SOURCES.md`. Use documented approximate motor curves when public authoritative curves are unavailable and state that the simulated trajectory is a reference scenario, not a historical NASA flight.

- [ ] **Step 4: Add deterministic staged-flight coverage**

Run the reference through existing staged 6-DOF seams and assert the ordered presence of ignition, rail exit, first-stage burnout, separation, second-stage ignition, second-stage burnout, apogee, recovery, and landing events.

Run:

```bash
rtk cargo test -p ascent-domain -p ascent-sim
```

Expected: PASS, including byte-identical canonical trace bytes across two identical runs.

- [ ] **Step 5: Commit the evidence reference**

```bash
rtk git add data/reference/nasa-black-brant-ix examples/nasa-black-brant-ix.ascent crates/ascent-domain/src/vehicle.rs crates/ascent-domain/tests/black_brant_ix.rs
rtk git commit -m "feat: add evidence-graded Black Brant IX reference"
```

**Out of scope:** orbital flight, TVC, powered landing, proprietary motor data, or historical-flight equivalence claims.

---

## Step 2 — Implement the Rust visualizer protocol and bridge

**Files:**

- Create: `crates/ascent-visualizer-protocol/src/lib.rs`
- Create: `crates/ascent-visualizer-protocol/tests/codec.rs`
- Create: `crates/ascent-visualizer-bridge/src/main.rs`
- Create: `crates/ascent-visualizer-bridge/tests/subprocess.rs`
- Create: `data/protocol/visualizer/v1/`
- Modify: `Cargo.toml`

**Interfaces:**

- Produces: `Envelope`, `ClientRequest`, `ServerMessage`, `FrameDecoder`, and the `ascent-visualizer-bridge --stdio` binary.
- Frame: four-byte little-endian payload length followed by UTF-8 JSON; maximum payload 16 MiB.
- Trace chunk: at most 1,024 samples for one channel, with deterministic `sequence` numbering.

- [ ] **Step 1: Write failing codec tests**

Cover a partial length prefix, partial payload, multiple frames in one buffer, zero length, oversized length, malformed UTF-8, malformed JSON, and exact round-trip serialization.

```bash
rtk cargo test -p ascent-visualizer-protocol
```

Expected: FAIL because the crate and codec do not exist.

- [ ] **Step 2: Define the complete v1 envelope**

The serialized envelope fields are exactly:

```rust
pub struct Envelope<T> {
    pub protocol_version: u16,
    pub message_id: String,
    pub request_id: Option<String>,
    pub kind: String,
    pub payload: T,
}
```

Client kinds are `hello`, `list_missions`, `run_mission`, `cancel_run`, and `shutdown`. Server kinds are `hello_ack`, `mission_catalog`, `run_started`, `run_progress`, `trace_manifest`, `trace_channel_chunk`, `trace_events`, `run_completed`, `run_cancelled`, and `error`.

- [ ] **Step 3: Implement and test framing**

The decoder retains incomplete bytes between reads, emits all complete frames, and permanently rejects the current connection after a framing violation. stdout carries frames only; stderr carries diagnostics.

- [ ] **Step 4: Write the failing subprocess contract test**

Spawn the bridge, negotiate protocol v1, list the one mission id `nasa.black-brant-ix.reference`, run it, collect progress and chunks, reconstruct the trace, verify the canonical SHA-256, test cancellation, and request shutdown.

- [ ] **Step 5: Implement the read-only bridge runtime**

Load the checked-in reference into memory, execute the existing staged 6-DOF path, validate the resulting `FlightTrace`, and emit channels in canonical order with 1,024-sample chunks. Stable error codes are `protocol_mismatch`, `invalid_request`, `mission_not_found`, `run_failed`, `cancelled`, and `internal`.

- [ ] **Step 6: Run scoped and workspace tests**

```bash
rtk cargo test -p ascent-visualizer-protocol -p ascent-visualizer-bridge
rtk cargo test --workspace
```

Expected: PASS; the reference project and journal remain byte-identical before and after a bridge run.

- [ ] **Step 7: Commit the bridge**

```bash
rtk git add Cargo.toml Cargo.lock crates/ascent-visualizer-protocol crates/ascent-visualizer-bridge data/protocol/visualizer/v1
rtk git commit -m "feat: add Unreal visualizer bridge protocol"
```

**Out of scope:** TCP, WebSocket, MCP reuse, arbitrary filesystem browsing, project mutation, or compression.

---

## Step 3 — Scaffold Unreal and supervise the sidecar

**Files:**

- Create: `visualizer/AscentUnreal/AscentUnreal.uproject`
- Create: `visualizer/AscentUnreal/Source/AscentUnreal/AscentUnreal.Build.cs`
- Create: `visualizer/AscentUnreal/Source/AscentUnreal/Bridge/AscentBridgeSubsystem.h`
- Create: `visualizer/AscentUnreal/Source/AscentUnreal/Bridge/AscentBridgeSubsystem.cpp`
- Create: `visualizer/AscentUnreal/Source/AscentUnrealTests/`
- Modify: `.gitignore`

**Interfaces:**

- Produces: `UAscentBridgeSubsystem : UGameInstanceSubsystem`.
- Public operations: `StartBridge()`, `ListMissions()`, `RunMission(FString MissionId)`, `CancelRun()`, and `StopBridge()`.
- State: `Stopped`, `Starting`, `Ready`, `Running`, `Cancelling`, and `Failed`.

- [ ] **Step 1: Install the approved Unreal skill stack**

```bash
rtk proxy skills add quodsoler/unreal-engine-skills@ue-project-context -g -y
rtk proxy skills add quodsoler/unreal-engine-skills@ue-cpp-foundations -g -y
rtk proxy skills add quodsoler/unreal-engine-skills@ue-module-build-system -g -y
rtk proxy skills add quodsoler/unreal-engine-skills@ue-testing-debugging -g -y
rtk proxy skills add quodsoler/unreal-engine-skills@ue-materials-rendering -g -y
rtk proxy skills add quodsoler/unreal-engine-skills@ue-niagara-effects -g -y
rtk proxy skills add quodsoler/unreal-engine-skills@ue-sequencer-cinematics -g -y
rtk proxy skills add quodsoler/unreal-engine-skills@ue-ui-umg-slate -g -y
```

- [ ] **Step 2: Install Unreal Engine 5.8 and create the C++ project**

Use a blank desktop project without Starter Content. Enable `Json`, `JsonUtilities`, `Projects`, `Slate`, `SlateCore`, `UMG`, `Niagara`, `MovieRenderPipelineCore`, and `ProceduralMeshComponent`. Ignore `Binaries/`, `DerivedDataCache/`, `Intermediate/`, and `Saved/`.

- [ ] **Step 3: Add failing protocol-fixture automation tests**

Load every JSON fixture under `data/protocol/visualizer/v1/` and assert the exact protocol version, kind, request linkage, and payload fields.

- [ ] **Step 4: Implement safe child-process ownership**

Use `FPlatformProcess::CreateProc` directly without a shell. Resolve the packaged bridge beside the app and allow one explicit development override in project settings. Read stdout frames and stderr on worker threads, then marshal immutable events to the game thread.

- [ ] **Step 5: Implement lifecycle guards**

Handshake timeout is five seconds. Shutdown timeout is two seconds. Unexpected exit triggers one restart. Increment a process-generation counter on every start and ignore messages from older generations. Unreal shutdown always terminates or reaps the child.

- [ ] **Step 6: Run headless automation**

```bash
rtk proxy "/Users/Shared/Epic Games/UE_5.8/Engine/Binaries/Mac/UnrealEditor-Cmd" visualizer/AscentUnreal/AscentUnreal.uproject -unattended -nop4 -NullRHI -ExecCmds="Automation RunTests Ascent.Bridge;Quit"
```

Expected: PASS for fixture parsing, handshake, forced exit, one restart, graceful shutdown, and orphan-process checks.

- [ ] **Step 7: Commit the Unreal process shell**

```bash
rtk git add .gitignore visualizer/AscentUnreal
rtk git commit -m "feat: scaffold supervised Unreal visualizer"
```

**Out of scope:** installers, signing, remote bridges, or MCP integration.

---

## Step 4 — Build deterministic mission playback

**Files:**

- Create: `visualizer/AscentUnreal/Source/AscentUnreal/Mission/AscentFlightTrace.h`
- Create: `visualizer/AscentUnreal/Source/AscentUnreal/Mission/AscentMissionSubsystem.h`
- Create: `visualizer/AscentUnreal/Source/AscentUnreal/Mission/AscentMissionSubsystem.cpp`
- Create: `visualizer/AscentUnreal/Source/AscentUnreal/Vehicle/AscentReferenceVehicle.h`
- Create: `visualizer/AscentUnreal/Source/AscentUnreal/Vehicle/AscentReferenceVehicle.cpp`
- Create: playback automation tests in `AscentUnrealTests`

**Interfaces:**

- Produces: immutable Unreal trace/channel/event structures and `UAscentMissionSubsystem`.
- Playback operations: `Play`, `Pause`, `SeekSeconds`, `StepFrames`, `SetRate`, and `SetRange`.
- Rates: `0.25`, `0.5`, `1.0`, `2.0`, and `5.0`.

- [ ] **Step 1: Write failing reconstruction and transform tests**

Cover missing chunks, duplicate chunks, out-of-order chunks, hash mismatch, identity orientation, cardinal axes, compound rotation, metre-to-centimetre scaling, exact event seeks, invalid-sample gaps, and end behavior.

- [ ] **Step 2: Implement immutable trace reconstruction**

Accept a trace only after all declared channels and events arrive, sequences are contiguous, validation succeeds, and its identity matches the bridge manifest. Failed traces never partially replace the current mission.

- [ ] **Step 3: Implement the coordinate basis**

Map ENU metres to Unreal centimetres with `X = North`, `Y = East`, and `Z = Up`. Convert attitude through the corresponding basis matrix and reconstruct the Unreal quaternion from that matrix.

- [ ] **Step 4: Implement deterministic sampling**

Linearly interpolate valid position samples and use normalized shortest-path slerp for attitude. Never interpolate across invalid samples or evidence gaps. Seek events to their exact Rust timestamps.

- [ ] **Step 5: Build the reference vehicle actor and camera rigs**

Create separable stage components and pad, chase, onboard, ground-tracking, and free-inspection cameras. Rust stage events control stage visibility and detachment. Unreal physics remains disabled for scientific transforms.

- [ ] **Step 6: Run playback automation**

```bash
rtk proxy "/Users/Shared/Epic Games/UE_5.8/Engine/Binaries/Mac/UnrealEditor-Cmd" visualizer/AscentUnreal/AscentUnreal.uproject -unattended -nop4 -NullRHI -ExecCmds="Automation RunTests Ascent.Mission;Quit"
```

Expected: PASS; seeking any time twice yields the same actor transforms and current event.

- [ ] **Step 7: Commit playback**

```bash
rtk git add visualizer/AscentUnreal/Source/AscentUnreal/Mission visualizer/AscentUnreal/Source/AscentUnreal/Vehicle visualizer/AscentUnreal/Source/AscentUnrealTests
rtk git commit -m "feat: add deterministic Unreal mission playback"
```

**Out of scope:** authoring, user-controlled physics, arbitrary mission imports, or multiplayer.

---

## Step 5 — Build the realistic engineering X-ray and cinematic tiers

**Files:**

- Create: `visualizer/AscentUnreal/Source/AscentUnreal/Rendering/`
- Create: `visualizer/AscentUnreal/Source/AscentUnreal/UI/`
- Create: `visualizer/AscentUnreal/Content/Ascent/`
- Create: `visualizer/AscentUnreal/Config/DefaultScalability.ini`
- Create: `visualizer/AscentUnreal/Config/DefaultEngine.ini`

**Interfaces:**

- Produces: layer controller, semantic timeline, state inspector, camera selector, interactive preset, cinematic preset, and export manifest.
- Every overlay consumes an immutable trace channel, typed event, or evidence record.

- [ ] **Step 1: Write failing layer and timeline tests**

Verify that each layer declares its source id, units, visibility, valid time interval, and evidence status. Verify that one clean-view action hides every engineering layer without changing the trace or playback clock.

- [ ] **Step 2: Build the evidence-backed vehicle geometry**

Generate the Black Brant IX body and fins reproducibly from the reference dimensions. Use separable stage meshes, PBR materials, panel seams, NASA-derived markings with recorded rights, and explicit notes for visual approximations.

- [ ] **Step 3: Build the bounded White Sands environment**

Import only the checked-in terrain subset. Configure Sky Atmosphere, directional sunlight, restrained height fog, shadows, and volumetrics. Do not add Cesium or runtime downloads.

- [ ] **Step 4: Add trace-driven effects and engineering layers**

Drive the Niagara plume and stage state from Rust data. Provide trajectory, velocity, body-axis, attitude, uncertainty when available, stage-state, and named-event layers. Spatial labels must remain unit-bearing, occlusion-aware, and readable in motion.

- [ ] **Step 5: Build the professional review shell**

Use Slate/UMG for mission status, semantic timeline, layer controls, state inspector, camera controls, evidence labels, loading, cancellation, and actionable errors. Prefer restrained typography, consistent spacing, and progressive disclosure over a game HUD.

- [ ] **Step 6: Implement quality tiers and export**

Interactive mode uses medium scalability and targets 1920x1080. Cinematic mode uses Movie Render Queue with higher lighting, shadow, atmosphere, and antialiasing settings. Export PNG or EXR universally and Apple ProRes when available. The adjacent JSON manifest records trace hash, protocol version, camera, range, preset, resolution, frame rate, and caveats.

- [ ] **Step 7: Run visual and performance checks**

Capture Unreal Insights traces for the complete mission. Interactive acceptance is at least 30 FPS in editor and 45 FPS packaged on the M3 Pro/18 GB MacBook without sustained macOS memory-pressure warnings.

- [ ] **Step 8: Commit the visual experience**

```bash
rtk git add visualizer/AscentUnreal
rtk git commit -m "feat: add Unreal engineering review experience"
```

**Out of scope:** global Cesium terrain, full mission authoring, AI proposal UI, or final production asset polish.

---

## Step 6 — Verify the vertical slice and record the platform decision

**Files:**

- Create: `docs/UNREAL_VISUALIZER_VERIFICATION.md`
- Create: `docs/UNREAL_PLATFORM_DECISION.md`
- Modify: `docs/PRODUCT_DIRECTION.md`
- Modify: `xtask/src/main.rs`

**Interfaces:**

- Produces: a repeatable Unreal verification target and an evidence-backed go/no-go decision.

- [ ] **Step 1: Add the Unreal gate to `cargo xtask`**

The target verifies the Rust bridge, protocol fixtures, Unreal automation suite, packaged smoke launch, reference mission completion, and cinematic manifest. If Unreal is unavailable, it reports a clear prerequisite failure rather than silently skipping.

- [ ] **Step 2: Run the complete vertical-slice scenario**

Launch the packaged application, handshake, select Black Brant IX, run Rust simulation, play ignition through landing, seek every named event, switch every camera, toggle layers, kill and recover the bridge, export a cinematic, and rerun to confirm trace-hash equality.

- [ ] **Step 3: Run all project gates**

```bash
rtk cargo fmt --all -- --check
rtk cargo test --workspace
rtk cargo xtask test
rtk proxy "/Users/Shared/Epic Games/UE_5.8/Engine/Binaries/Mac/UnrealEditor-Cmd" visualizer/AscentUnreal/AscentUnreal.uproject -unattended -nop4 -NullRHI -ExecCmds="Automation RunTests Ascent;Quit"
```

Expected: all gates pass from a clean snapshot.

- [ ] **Step 4: Record performance and usability evidence**

Record editor FPS, packaged FPS, frame-time distribution, peak memory, bridge startup, simulation latency, transfer duration, trace size, shader experience, screenshots, and cinematic output.

- [ ] **Step 5: Apply the go/no-go criteria**

Advance Unreal toward replacing the workbench only if it is materially stronger visually, meets local performance targets, preserves the Rust authority boundary, recovers cleanly from bridge failure, and supports a coherent professional review workflow. Otherwise retain it as an optional visualizer and cinematic exporter.

- [ ] **Step 6: Commit verification and decision records**

```bash
rtk git add docs/UNREAL_VISUALIZER_VERIFICATION.md docs/UNREAL_PLATFORM_DECISION.md docs/PRODUCT_DIRECTION.md xtask/src/main.rs
rtk git commit -m "docs: record Unreal visualizer platform decision"
```

**Out of scope:** deleting Tauri or MCP, signing releases, or starting the next Unreal release before the gate is reviewed.

## Final self-review checklist

- [ ] Every requirement in the approved design maps to a task above.
- [ ] Rust and Unreal protocol names match exactly in every task.
- [ ] No task introduces a second physics, clock, evidence, or mutation authority.
- [ ] All public or downloaded assets have recorded rights and hashes.
- [ ] Scoped tests pass before each commit.
- [ ] The complete workspace and Unreal gates pass before the platform decision.
