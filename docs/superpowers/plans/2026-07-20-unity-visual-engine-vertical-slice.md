# Unity Visual Engineering Vertical Slice Implementation Plan

> **Execution mode:** Work inline, one major step at a time. Implement first, then add and run the focused verification listed for that step. Do not use test-first cycles, subagent chains, or worktrees. Verification snippets may appear before implementation text for reference, but execute them only afterward.

**Goal:** Build one narrow but convincing Ascent experience: an evidence-graded NASA Black Brant IX mission simulated by Rust and reviewed launch-to-landing in a realistic, interactive, cinematic Unity client.

**Architecture:** Rust remains the sole authority for mission state, physics, events, evidence, and canonical trace identity. Unity 6 LTS launches a supervised read-only Rust sidecar over a versioned framed-stdio protocol, validates the complete trace, and owns only playback presentation, engineering layers, cameras, UI, and export.

**Tech Stack:** Rust 1.97, Serde JSON, SHA-256, Unity 6 LTS (6000.0 line), C#/.NET Standard 2.1, HDRP, Cinemachine, Timeline, VFX Graph, UI Toolkit, Splines, Unity Test Framework, and `com.unity.nuget.newtonsoft-json`.

## Global Constraints

- Begin implementation only after the active v0.6 work is consolidated and its required gates pass from a clean snapshot.
- Preserve Rust as the only engineering, physics, evidence, event, and mission-state authority.
- Preserve Tauri, React, and `ascent-mcp`; do not redesign or delete them during this slice.
- Unity is read-only and dispatches no Ascent document mutation commands.
- Use no runtime network access, sockets, streamed terrain, or remote bridge.
- Use Unity 6 LTS from the 6000.0 release line and commit the exact editor version in `ProjectVersion.txt`.
- Use HDRP, not URP, for the slice.
- Use `X = East`, `Y = Up`, `Z = North`, and one Unity unit per metre.
- Every displayed engineering value must resolve to a trace channel, typed event, or evidence record.
- Every external artifact must record source URL, retrieval date, SHA-256, rights, affected fields, and evidence grade.
- Interactive acceptance is 1920x1080 at 30 FPS or better in-editor and 45 FPS or better packaged on the M3 Pro with 18 GB memory.
- Cinematic rendering may run offline and has no real-time frame-rate requirement.
- Prefix repository shell commands with `rtk`; use one raw fallback immediately if RTK rejects a command form.

---

## File map

### Rust and evidence

- `data/reference/nasa-black-brant-ix/manifest.json`: source inventory, rights, hashes, grades, and field bindings.
- `data/reference/nasa-black-brant-ix/vehicle.json`: reviewable reference inputs separate from code.
- `data/reference/nasa-black-brant-ix/SOURCES.md`: formulas, assumptions, and non-historical-use caveat.
- `examples/nasa-black-brant-ix.ascent`: checked-in reference mission.
- `crates/ascent-domain/src/reference.rs`: typed reference loader and validation.
- `crates/ascent-domain/tests/black_brant_ix.rs`: evidence and vehicle invariants.
- `crates/ascent-sim/tests/black_brant_ix_trace.rs`: deterministic event and trace regression.
- `crates/ascent-visualizer-protocol/`: protocol schema, framing, fixtures, and codec tests.
- `crates/ascent-visualizer-bridge/`: read-only stdio process and subprocess tests.

### Unity

- `visualizer/AscentUnity/Packages/manifest.json`: exact first-party Unity package set.
- `visualizer/AscentUnity/ProjectSettings/`: pinned editor and HDRP project configuration.
- `visualizer/AscentUnity/Assets/Ascent/Runtime/Bridge/`: process lifecycle and framed transport.
- `visualizer/AscentUnity/Assets/Ascent/Runtime/Trace/`: immutable trace assembly, validation, coordinates, and sampling.
- `visualizer/AscentUnity/Assets/Ascent/Runtime/Playback/`: playback clock, events, stage state, and view model.
- `visualizer/AscentUnity/Assets/Ascent/Runtime/Presentation/`: vehicle, environment, VFX, layers, cameras, and export.
- `visualizer/AscentUnity/Assets/Ascent/Runtime/UI/`: UI Toolkit review shell.
- `visualizer/AscentUnity/Assets/Ascent/Tests/EditMode/`: deterministic unit and fixture tests.
- `visualizer/AscentUnity/Assets/Ascent/Tests/PlayMode/`: lifecycle and complete-workflow tests.
- `scripts/unity.sh`: resolves the pinned Unity Hub editor and runs it consistently.

### Verification

- `xtask/src/main.rs`: opt-in `visualizer-test` gate.
- `docs/UNITY_VISUAL_ENGINE_VERIFICATION.md`: reproducible evidence and hardware results.
- `docs/UNITY_PLATFORM_DECISION.md`: measured expansion or containment decision.
- `docs/PRODUCT_DIRECTION.md`: current Ascent framing and platform role.

---

## Step 1 — Establish the evidence-graded Black Brant IX mission

**Files:**

- Create: `data/reference/nasa-black-brant-ix/manifest.json`
- Create: `data/reference/nasa-black-brant-ix/vehicle.json`
- Create: `data/reference/nasa-black-brant-ix/SOURCES.md`
- Create: `examples/nasa-black-brant-ix.ascent`
- Create: `crates/ascent-domain/src/reference.rs`
- Create: `crates/ascent-domain/tests/black_brant_ix.rs`
- Create: `crates/ascent-sim/tests/black_brant_ix_trace.rs`
- Modify: `crates/ascent-domain/src/lib.rs`

**Interfaces:**

- Consumes: existing `Vehicle`, motor, evidence, staged-flight, `FlightTrace`, and canonical serialization types.
- Produces: `pub const BLACK_BRANT_IX_MISSION_ID: &str`, `pub fn black_brant_ix_reference() -> Result<ReferenceMission, ReferenceError>`, and a deterministic two-stage reference trace.
- `ReferenceMission` contains `mission_id: String`, `vehicle: Vehicle`, `evidence: Vec<EvidenceBinding>`, and the existing simulation inputs required by the staged 6-DOF runner.

- [ ] **Step 1.1: Capture primary sources and rights before entering values**

Use the NASA Sounding Rockets User Handbook as the primary vehicle source, official NASA imagery for visible proportions and markings, and one bounded offline USGS 3DEP White Sands terrain product for later presentation. Store redistributable source artifacts under `data/reference/nasa-black-brant-ix/sources/`; otherwise store a metadata record and retrieval instructions without copying the artifact.

Every manifest item uses this exact field set. The implementation obtains the digest with `shasum -a 256 <artifact>` and copies the command output into `sha256` before running tests:

```json
{
  "id": "nasa-sounding-rockets-user-handbook",
  "source_url": "https://sites.wff.nasa.gov/code810/files/SRHB.pdf",
  "retrieved_at": "2026-07-20",
  "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "rights": "United States Government work; verify the specific page and image credit",
  "fields": ["vehicle.stage_count", "vehicle.geometry", "vehicle.mass"],
  "grade": "authoritative",
  "stored_path": "sources/SRHB.pdf"
}
```

The digest shown above is the known SHA-256 of an empty file and demonstrates the serialized format only; validation also rejects an empty source artifact, so it cannot pass as source evidence.

- [ ] **Step 1.2: Add focused evidence and vehicle verification after implementation**

```rust
#[test]
fn black_brant_ix_is_fully_evidence_graded() {
    let reference = ascent_domain::reference::black_brant_ix_reference().unwrap();
    assert_eq!(reference.mission_id, "nasa.black-brant-ix.reference");
    assert_eq!(reference.vehicle.stages().len(), 2);
    assert!(reference.evidence.iter().all(|binding| {
        !binding.field_path.is_empty()
            && matches!(binding.grade.as_str(), "authoritative" | "derived" | "approximate")
    }));
    assert!(reference.sources.iter().all(|source| {
        source.sha256.len() == 64
            && source.sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    }));
}
```

Run:

```bash
rtk cargo test -p ascent-domain --test black_brant_ix
```

Verification target after implementation: PASS.

- [ ] **Step 1.3: Implement the typed reference loader**

Add `pub mod reference;` to `crates/ascent-domain/src/lib.rs`. Parse checked-in JSON with `serde_json`, reject a field without exactly one evidence binding, reject non-finite or non-positive engineering dimensions, and reject a source digest that is not 64 lowercase hexadecimal characters. Derived entries must contain a formula and named input field paths. Approximate entries must contain an impact statement.

- [ ] **Step 1.4: Add the deterministic mission regression after implementation**

```rust
#[test]
fn black_brant_ix_trace_is_repeatable_and_event_complete() {
    let first = run_black_brant_ix_reference().unwrap();
    let second = run_black_brant_ix_reference().unwrap();
    assert_eq!(first.canonical_bytes().unwrap(), second.canonical_bytes().unwrap());

    let names: Vec<_> = first.events.iter().map(|event| event.event_type.as_str()).collect();
    for required in [
        "ignition", "rail_exit", "stage_1_burnout", "stage_separation",
        "stage_2_ignition", "stage_2_burnout", "apogee", "recovery", "landing",
    ] {
        assert!(names.contains(&required), "missing event {required}");
    }
}
```

Run:

```bash
rtk cargo test -p ascent-sim --test black_brant_ix_trace
```

Verification target after implementation: PASS.

- [ ] **Step 1.5: Implement and verify the reference mission**

Map authoritative values directly. Put every derivation and input in `SOURCES.md`. Use approximate inputs only where a public authoritative value is unavailable, label their downstream effect, and display the sentence “Reference scenario, not a reconstruction of a historical NASA flight” in both the `.ascent` metadata and source notes.

Run:

```bash
rtk cargo test -p ascent-domain --test black_brant_ix
rtk cargo test -p ascent-sim --test black_brant_ix_trace
rtk cargo fmt --all -- --check
```

Expected: PASS, including byte-identical trace bytes across two runs.

- [ ] **Step 1.6: Commit the evidence reference**

```bash
rtk git add data/reference/nasa-black-brant-ix examples/nasa-black-brant-ix.ascent crates/ascent-domain/src/lib.rs crates/ascent-domain/src/reference.rs crates/ascent-domain/tests/black_brant_ix.rs crates/ascent-sim/tests/black_brant_ix_trace.rs
rtk git commit -m "feat: add evidence-graded Black Brant IX reference"
```

**Acceptance:** all engineering inputs are evidence-bound; the mission contains the required ordered events; identical runs produce identical canonical bytes and SHA-256.

**Out of scope:** orbital flight, TVC, powered landing, proprietary motor data, or historical-flight equivalence claims.

---

## Step 2 — Implement the Rust protocol and supervised bridge

**Files:**

- Create: `crates/ascent-visualizer-protocol/Cargo.toml`
- Create: `crates/ascent-visualizer-protocol/src/lib.rs`
- Create: `crates/ascent-visualizer-protocol/src/codec.rs`
- Create: `crates/ascent-visualizer-protocol/src/messages.rs`
- Create: `crates/ascent-visualizer-protocol/tests/codec.rs`
- Create: `crates/ascent-visualizer-bridge/Cargo.toml`
- Create: `crates/ascent-visualizer-bridge/src/main.rs`
- Create: `crates/ascent-visualizer-bridge/src/runtime.rs`
- Create: `crates/ascent-visualizer-bridge/tests/subprocess.rs`
- Create: `data/protocol/visualizer/v1/`
- Modify: `Cargo.toml`

**Interfaces:**

- Produces: `Envelope<T>`, `ClientRequest`, `ServerMessage`, `FrameDecoder`, `encode_frame`, fixture JSON, and `ascent-visualizer-bridge --stdio`.
- Frame: four-byte little-endian length plus UTF-8 JSON, maximum 16 MiB.
- Chunk: one channel, at most 1,024 samples, contiguous `sequence` beginning at zero.

- [ ] **Step 2.1: Add focused codec verification after implementation**

```rust
#[test]
fn decoder_retains_partial_frames_and_emits_concatenated_frames() {
    let first = encode_frame(&hello_fixture()).unwrap();
    let second = encode_frame(&list_fixture()).unwrap();
    let mut decoder = FrameDecoder::new(16 * 1024 * 1024);
    assert!(decoder.push(&first[..3]).unwrap().is_empty());
    let mut rest = first[3..].to_vec();
    rest.extend_from_slice(&second);
    let decoded = decoder.push(&rest).unwrap();
    assert_eq!(decoded, vec![hello_fixture(), list_fixture()]);
}
```

Also test zero length, 16 MiB plus one byte, malformed UTF-8, malformed JSON, unknown protocol version, and decoder poisoning after a framing violation.

Run:

```bash
rtk cargo test -p ascent-visualizer-protocol
```

Verification target after implementation: PASS.

- [ ] **Step 2.2: Define the exact v1 envelope and message kinds**

```rust
pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CHUNK_SAMPLES: usize = 1_024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope<T> {
    pub protocol_version: u16,
    pub message_id: String,
    pub request_id: Option<String>,
    pub kind: String,
    pub payload: T,
}
```

Use internally tagged payload enums while preserving the envelope fields exactly. Client kinds are `hello`, `list_missions`, `run_mission`, `cancel_run`, and `shutdown`. Server kinds are `hello_ack`, `mission_catalog`, `run_started`, `run_progress`, `trace_manifest`, `trace_channel_chunk`, `trace_events`, `run_completed`, `run_cancelled`, and `error`.

- [ ] **Step 2.3: Implement framing and cross-language fixtures**

`encode_frame` serializes once, checks the maximum, writes the little-endian `u32` length, and appends bytes. `FrameDecoder::push` retains incomplete input, emits every complete envelope, and remains poisoned after any framing error. Generate stable fixtures under `data/protocol/visualizer/v1/` for every message kind plus malformed, duplicate, out-of-order, and hash-invalid cases.

Run:

```bash
rtk cargo test -p ascent-visualizer-protocol
```

Expected: PASS.

- [ ] **Step 2.4: Add the subprocess contract verification after implementation**

The test spawns `ascent-visualizer-bridge --stdio`, completes `hello`, confirms the single mission id, starts a run, reconstructs all channel chunks and events, validates the trace hash, starts and cancels a second run, starts a third run, and shuts down cleanly. It asserts stdout contains only frames and stderr never contains protocol bytes.

Run:

```bash
rtk cargo test -p ascent-visualizer-bridge --test subprocess
```

Verification target after implementation: PASS.

- [ ] **Step 2.5: Implement the bridge state machine**

States are `AwaitingHello`, `Idle`, `Running { request_id }`, and `Stopping`. Only `hello` is valid before negotiation. Only one run is active. The bridge loads `nasa.black-brant-ix.reference` in memory, executes the existing staged 6-DOF runner, validates the trace, emits canonical channel order in 1,024-sample chunks, then emits events and `run_completed`. Stable error codes are `protocol_mismatch`, `invalid_request`, `mission_not_found`, `run_failed`, `cancelled`, and `internal`.

No request accepts an arbitrary path, URL, Ascent command, or serialized document mutation.

- [ ] **Step 2.6: Run scoped, security, and workspace gates**

```bash
rtk cargo test -p ascent-visualizer-protocol -p ascent-visualizer-bridge
rtk cargo test --workspace
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS; a bridge run leaves the checked-in mission and journal byte-identical.

- [ ] **Step 2.7: Commit the bridge boundary**

```bash
rtk git add Cargo.toml Cargo.lock crates/ascent-visualizer-protocol crates/ascent-visualizer-bridge data/protocol/visualizer/v1
rtk git commit -m "feat: add Unity visualizer bridge protocol"
```

**Acceptance:** Rust and fixture tests cover every message; malformed input fails closed; the subprocess streams a deterministic, hash-valid trace and supports cancellation plus clean shutdown.

**Out of scope:** TCP, WebSocket, MCP reuse, arbitrary file access, project mutation, compression, or remote execution.

---

## Step 3 — Create the Unity HDRP client and deterministic playback core

**Files:**

- Create: `scripts/unity.sh`
- Create: `visualizer/AscentUnity/Packages/manifest.json`
- Create: `visualizer/AscentUnity/ProjectSettings/ProjectVersion.txt`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Bridge/BridgeProcess.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Bridge/FrameDecoder.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Bridge/ProtocolMessages.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Trace/FlightTrace.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Trace/TraceAssembler.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Trace/CoordinateBasis.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Playback/PlaybackClock.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Tests/EditMode/`
- Create: `visualizer/AscentUnity/Assets/Ascent/Tests/PlayMode/BridgeLifecycleTests.cs`
- Modify: `.gitignore`

**Interfaces:**

- Consumes: Rust v1 fixtures and `ascent-visualizer-bridge --stdio`.
- Produces: `BridgeProcess`, immutable `FlightTrace`, `TraceAssembler`, `CoordinateBasis`, and `PlaybackClock`.
- `BridgeProcess` publishes immutable messages on the Unity main thread and never exposes its worker buffers.
- `PlaybackClock` exposes `Play()`, `Pause()`, `Seek(double seconds)`, `StepFrames(int frames)`, `SetRate(double rate)`, and `SetRange(double start, double end)`.

- [ ] **Step 3.1: Pin the project and first-party packages**

Create the project through Unity Hub using the current Unity 6 LTS editor from the 6000.0 line and commit the exact generated `m_EditorVersion` and `m_EditorVersionWithRevision`. Add `com.unity.render-pipelines.high-definition`, `com.unity.cinemachine`, `com.unity.timeline`, `com.unity.visualeffectgraph`, `com.unity.ui`, `com.unity.test-framework`, `com.unity.splines`, and `com.unity.nuget.newtonsoft-json`, selecting versions resolved for that editor line. Commit `Packages/packages-lock.json` so resolution is reproducible.

Create `scripts/unity.sh` with this behavior:

```bash
#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(awk '/m_EditorVersion:/{print $2}' "$project_root/visualizer/AscentUnity/ProjectSettings/ProjectVersion.txt")"
editor="/Applications/Unity/Hub/Editor/$version/Unity.app/Contents/MacOS/Unity"
if [[ ! -x "$editor" ]]; then
  echo "Unity editor $version is not installed through Unity Hub" >&2
  exit 2
fi
exec "$editor" -projectPath "$project_root/visualizer/AscentUnity" "$@"
```

- [ ] **Step 3.2: Add focused fixture and frame-decoder verification after implementation**

Use Rust fixture bytes unchanged. Test one-byte reads, split length prefix, split payload, multiple frames, zero length, oversize, malformed UTF-8, malformed JSON, duplicate `message_id`, and unknown protocol version.

```csharp
[Test]
public void DecoderRetainsPartialPrefixAndPayload()
{
    var bytes = Fixture.ReadFrame("hello_ack.frame");
    var decoder = new FrameDecoder(16 * 1024 * 1024);
    Assert.That(decoder.Push(bytes.AsSpan(0, 3)), Is.Empty);
    var decoded = decoder.Push(bytes.AsSpan(3));
    Assert.That(decoded.Single().Kind, Is.EqualTo("hello_ack"));
}
```

Run:

```bash
rtk proxy scripts/unity.sh -batchmode -nographics -runTests -testPlatform EditMode -testResults visualizer/AscentUnity/TestResults/editmode.xml -quit
```

Verification target after implementation: PASS.

- [ ] **Step 3.3: Implement process supervision and binary framed I/O**

Launch with `System.Diagnostics.Process`, `UseShellExecute = false`, and all three standard streams redirected. Read `StandardOutput.BaseStream` on a dedicated task, read stderr independently, and write request frames through `StandardInput.BaseStream`. Never use `ReadLine`, `OutputDataReceived`, or newline-delimited JSON.

Handshake expires after five seconds. Shutdown waits two seconds before killing and reaping the child. One unexpected exit triggers one restart with a new process generation. Messages tagged to an earlier generation are discarded. Application quit always invokes shutdown and reap.

- [ ] **Step 3.4: Add focused trace assembly and coordinate verification after implementation**

Cover missing, duplicate, and out-of-order chunks; manifest count mismatch; invalid hash; identity attitude; East, North, and Up axes; compound rotation; shortest-path slerp; invalid-sample gaps; and exact event seeking.

```csharp
[Test]
public void EnuBasisMapsAxesWithoutQuaternionFieldSwaps()
{
    Assert.That(CoordinateBasis.Position(new Vector3d(1, 0, 0)), Is.EqualTo(new Vector3(1, 0, 0)));
    Assert.That(CoordinateBasis.Position(new Vector3d(0, 1, 0)), Is.EqualTo(new Vector3(0, 0, 1)));
    Assert.That(CoordinateBasis.Position(new Vector3d(0, 0, 1)), Is.EqualTo(new Vector3(0, 1, 0)));
    Assert.That(CoordinateBasis.Attitude(Matrix3x3d.Identity), Is.EqualTo(Quaternion.identity));
}
```

- [ ] **Step 3.5: Implement immutable trace validation and playback**

Accept a trace only after every declared channel and event arrives with contiguous sequences, declared sample counts, valid masks, units, and matching canonical SHA-256. Convert attitudes with the complete ENU-to-Unity basis matrix. Use linear interpolation for adjacent valid positions and normalized shortest-path slerp for adjacent valid attitudes. Return a gap state rather than crossing an invalid interval.

Rates are exactly 0.25x, 0.5x, 1x, 2x, and 5x. Frame step uses the export frame rate. Reverse behavior is seek-based; Rust is never rerun to move backward.

- [ ] **Step 3.6: Add bridge lifecycle PlayMode tests**

Test successful negotiation, forced child exit and one restart, failed second restart, cancellation followed by a new run, graceful quit, and no remaining bridge PID. Tests use the real debug bridge binary built by Cargo, not a line-oriented mock.

Run:

```bash
rtk cargo build -p ascent-visualizer-bridge
rtk proxy scripts/unity.sh -batchmode -nographics -runTests -testPlatform EditMode -testResults visualizer/AscentUnity/TestResults/editmode.xml -quit
rtk proxy scripts/unity.sh -batchmode -nographics -runTests -testPlatform PlayMode -testResults visualizer/AscentUnity/TestResults/playmode.xml -quit
```

Expected: PASS with zero failed Unity tests and no orphan bridge process.

- [ ] **Step 3.7: Commit the Unity runtime foundation**

```bash
rtk git add .gitignore scripts/unity.sh visualizer/AscentUnity
rtk git commit -m "feat: add supervised Unity visualizer runtime"
```

**Acceptance:** Unity consumes Rust fixtures unchanged; a real bridge lifecycle is recoverable; coordinate and playback tests are deterministic; failed traces never partially replace the current mission.

**Out of scope:** rendering polish, mission authoring, Unity physics authority, arbitrary mission imports, remote bridges, installers, or signing.

---

## Step 4 — Build the narrow convincing engineering experience

**Files:**

- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Presentation/ReferenceVehicle.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Presentation/EnvironmentController.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Presentation/CameraDirector.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Presentation/EngineeringLayers.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/Presentation/CinematicExporter.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Runtime/UI/ReviewWorkbench.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/UI/ReviewWorkbench.uxml`
- Create: `visualizer/AscentUnity/Assets/Ascent/UI/ReviewWorkbench.uss`
- Create: `visualizer/AscentUnity/Assets/Ascent/Scenes/BlackBrantIX.unity`
- Create: `visualizer/AscentUnity/Assets/Ascent/Tests/EditMode/LayerBindingTests.cs`
- Create: `visualizer/AscentUnity/Assets/Ascent/Tests/PlayMode/VerticalSliceTests.cs`

**Interfaces:**

- Consumes: accepted immutable `FlightTrace`, typed events, evidence bindings, and `PlaybackClock`.
- Produces: five camera modes, semantic timeline, state inspector, trace-backed engineering layers, clean view, interactive preset, cinematic preset, and export manifest.
- Camera ids are `pad`, `chase`, `onboard`, `ground_tracking`, and `inspection`.
- Layer ids are `trajectory`, `velocity`, `body_axes`, `attitude`, `stage_state`, `events`, and `uncertainty` when present.

- [ ] **Step 4.1: Add focused layer-binding and clean-view verification after implementation**

```csharp
[Test]
public void EveryEngineeringLayerDeclaresTraceableInputs()
{
    foreach (var layer in EngineeringLayerCatalog.All)
    {
        Assert.That(layer.SourceIds, Is.Not.Empty, layer.Id);
        Assert.That(layer.Units, Is.Not.Null, layer.Id);
        Assert.That(layer.EvidencePolicy, Is.Not.EqualTo(EvidencePolicy.Unspecified), layer.Id);
    }
}

[UnityTest]
public IEnumerator CleanViewHidesLayersWithoutChangingReviewState()
{
    var before = harness.SnapshotReviewState();
    harness.SetCleanView(true);
    yield return null;
    Assert.That(harness.VisibleEngineeringLayerCount, Is.Zero);
    Assert.That(harness.SnapshotReviewState().WithoutLayerVisibility(), Is.EqualTo(before.WithoutLayerVisibility()));
}
```

Run these EditMode and PlayMode checks only after the catalog and shell are implemented. Expected: PASS.

- [ ] **Step 4.2: Build the evidence-backed vehicle and bounded range**

Generate or import the two separable stage meshes from the checked-in reference dimensions. Use PBR materials, restrained panel seams, and rights-cleared markings. Keep collision and Rigidbody simulation disabled for scientific motion. Rust stage events control attachment visibility and the rendered separation transform.

Import only the bounded, checked-in USGS terrain subset. Configure HDRP sunlight, sky, exposure, atmospheric haze, shadows, and fog for scale readability. Do not install Cesium or fetch assets at runtime. Record each visual approximation in the evidence drawer.

- [ ] **Step 4.3: Add trace-driven plume, stages, and engineering layers**

Drive VFX Graph plume intensity, color regime, and cutoff from explicit thrust and event channels. Use Splines for the trajectory. Velocity and body axes use unit-bearing glyphs; attitude uses a compact frame visualization; events use exact trace timestamps; uncertainty renders only when a channel exists and is labeled with its confidence meaning.

Every layer binds through `EngineeringLayerDescriptor.SourceIds`; no presentation component may query or derive canonical simulation state from Transform, Rigidbody, ParticleSystem, or VFX state.

- [ ] **Step 4.4: Build the five-camera cinematic grammar**

Use Cinemachine for pad, chase, onboard, ground-tracking, and free inspection cameras. Camera changes affect presentation only. Pad establishes scale, chase preserves motion cues, onboard exposes attitude and plume behavior, ground tracking preserves geographic context, and inspection pauses camera automation for close review. Use damped blends that never change playback time.

- [ ] **Step 4.5: Build the professional review workbench**

Use UI Toolkit with a compact top mission/status bar, left layer rail, right state/evidence inspector, and bottom semantic timeline. Timeline markers cover every typed event. Selecting a marker seeks the exact Rust time. Visible values include units, validity, and evidence status. Loading, cancellation, protocol failure, missing data, and bridge recovery each have explicit non-game copy and a clear next action.

Keyboard contracts are Space for play/pause, Left/Right for frame step, Shift+Left/Right for event jump, 1–5 for cameras, `L` for layers, and `C` for clean view. Text, controls, and focus order pass the Unity accessibility checks available to UI Toolkit and remain usable at 1920x1080.

- [ ] **Step 4.6: Implement cinematic export and manifest**

Export PNG image sequences for the slice; optional platform codecs are additive. The JSON manifest beside the output contains `trace_sha256`, `protocol_version`, `mission_id`, `camera_id`, `start_seconds`, `end_seconds`, `quality_preset`, `width`, `height`, `frame_rate`, `unity_version`, and `evidence_caveats`. Export advances a presentation-only fixed clock and cannot mutate the accepted trace.

- [ ] **Step 4.7: Prove the complete workflow in PlayMode**

The automated scenario launches the real bridge, runs Black Brant IX, waits for a valid trace, plays ignition to landing, seeks every named event, switches all five cameras, toggles each available layer, enables clean view, cancels and reruns, and writes a short export plus manifest. Assert a rerun has the same trace hash and clean view does not alter the review clock.

Run:

```bash
rtk cargo build -p ascent-visualizer-bridge
rtk proxy scripts/unity.sh -batchmode -runTests -testPlatform EditMode -testResults visualizer/AscentUnity/TestResults/editmode.xml -quit
rtk proxy scripts/unity.sh -batchmode -runTests -testPlatform PlayMode -testResults visualizer/AscentUnity/TestResults/playmode.xml -quit
```

Expected: PASS; the complete scenario finishes without Unity physics determining a scientific transform.

- [ ] **Step 4.8: Tune and commit the vertical slice**

Use the interactive HDRP preset at 1920x1080 on the M3 Pro/18 GB MacBook. Reduce bounded terrain detail, volumetric sample cost, shadow distance, and VFX particle count until the editor sustains at least 30 FPS and the packaged build sustains at least 45 FPS without sustained memory pressure. Preserve the offline cinematic preset separately.

```bash
rtk git add visualizer/AscentUnity
rtk git commit -m "feat: build Unity aerospace review vertical slice"
```

**Acceptance:** one launch-to-landing flow is visually credible and fully operable; five cameras and all present layers work; one action produces a clean view; every readout is traceable; the short cinematic export has a complete manifest.

**Out of scope:** global terrain, full authoring, AI proposal UI, generalized vehicle import, marketplace production assets, or orbital-scale rendering.

---

## Step 5 — Verify the slice and decide Unity's role

**Files:**

- Create: `docs/UNITY_VISUAL_ENGINE_VERIFICATION.md`
- Create: `docs/UNITY_PLATFORM_DECISION.md`
- Modify: `docs/PRODUCT_DIRECTION.md`
- Modify: `xtask/src/main.rs`

**Interfaces:**

- Produces: `rtk cargo xtask visualizer-test`, a repeatable acceptance record, and an evidence-backed `expand` or `contain` platform decision.

- [ ] **Step 5.1: Add the focused xtask command verification after implementation**

Add a command-parsing test proving `visualizer-test` is recognized and reports each gate distinctly: Rust protocol, bridge subprocess, Unity EditMode, Unity PlayMode, packaged smoke, performance record, and export manifest. If the pinned Unity editor is absent, return a prerequisite error naming the required editor version; never silently skip.

Run:

```bash
rtk cargo test -p xtask
```

Verification target after implementation: PASS.

- [ ] **Step 5.2: Implement the opt-in visualizer gate**

The gate builds the bridge, runs protocol and subprocess tests, invokes `scripts/unity.sh` for both Unity test platforms, launches a packaged development build with the reference scenario, validates process cleanup, and parses the export manifest. Keep the existing `cargo xtask test` behavior unchanged so machines without Unity retain the current workspace gate.

- [ ] **Step 5.3: Run the complete technical gate from a clean snapshot**

```bash
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace
rtk cargo xtask test
rtk cargo xtask visualizer-test
```

Expected: PASS with no orphan bridge process and identical trace hashes across the automated rerun.

- [ ] **Step 5.4: Record hardware and experience evidence**

In `docs/UNITY_VISUAL_ENGINE_VERIFICATION.md`, record exact Mac model, macOS version, Unity version, build commit, editor FPS median and 1% low, packaged FPS median and 1% low, peak memory, memory-pressure state, bridge startup, simulation duration, transfer duration, trace size, shader compilation experience, screenshots, export duration, and manifest path. Record failures as evidence rather than omitting them.

Conduct one unguided review with a technically literate person. Ask them to identify ignition, separation, apogee, active stage, vehicle orientation, and the source of one displayed value. Record whether they succeeded without spoken instruction.

- [ ] **Step 5.5: Apply the platform decision rule**

Set `decision` to `expand` only when all seven design gates pass: material visual improvement, local interactive targets, deterministic trace identity, recoverable process lifecycle, traceable overlays, intact Rust authority, and a coherent unguided review. Otherwise set it to `contain`, keeping Unity as an optional visualizer and cinematic exporter. Do not choose a partial result by enthusiasm alone.

Update `docs/PRODUCT_DIRECTION.md` to say:

> Ascent is a visual engineering runtime for constructing, simulating, interrogating, and reviewing complex aerospace systems. Rust is the authoritative engineering core. Unity's role is governed by the recorded visual-engine platform decision.

- [ ] **Step 5.6: Commit the verification record**

```bash
rtk git add xtask/src/main.rs docs/UNITY_VISUAL_ENGINE_VERIFICATION.md docs/UNITY_PLATFORM_DECISION.md docs/PRODUCT_DIRECTION.md
rtk git commit -m "docs: record Unity visual engine decision"
```

**Acceptance:** all technical gates have explicit results; M3 performance is measured; the complete review is repeatable; the platform decision follows the seven predeclared criteria.

**Out of scope:** deleting Tauri or MCP, adding Unity authoring, signing a release, cloud rendering, or beginning the next platform phase before review.

---

## Final self-review checklist

- [ ] Every approved design requirement maps to one step above.
- [ ] Rust and C# use identical protocol fields, message kinds, limits, timeouts, mission id, and coordinate basis.
- [ ] No task introduces a second physics, evidence, event, clock, or mutation authority.
- [ ] All displayed values and external assets have resolvable provenance.
- [ ] Focused verification is added and run after implementation; no test-first cycle or redundant broad test pass is required.
- [ ] Each step ends in a focused commit and independently reviewable deliverable.
- [ ] The interactive and cinematic quality tiers remain separate.
- [ ] Tauri, React, and `ascent-mcp` remain intact regardless of the Unity gate.
- [ ] The slice proves one complete experience instead of a generalized platform.
