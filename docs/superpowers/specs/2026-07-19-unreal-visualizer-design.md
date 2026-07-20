# Unreal Mission Visualizer Design

**Status:** Approved 2026-07-19

## Purpose

Build a polished Unreal Engine 5.8 visual client for Ascent. The first vertical slice runs an evidence-graded NASA Black Brant IX reference mission through the existing Rust simulation core, then presents realistic launch-to-landing playback, engineering inspection, and cinematic export.

The prototype answers one strategic question: can Unreal become Ascent's professional visual application without weakening Ascent's deterministic, evidence-backed Rust architecture?

## Product decisions

- Unreal begins as a live, read-only visualizer, not a complete workbench replacement.
- Cinematic export is a feature of the visualizer.
- A later move toward a complete Unreal application is allowed only after a measured go/no-go review.
- The first experience is a polished vertical slice, not a rendering sandbox or miniature full workbench.
- The visual center is a realistic engineering X-ray: believable geometry, materials, atmosphere, scale, motion, and cameras underneath; traceable engineering layers above.
- The first scenario is a complete launch-to-landing mission using a faithfully reconstructed NASA Black Brant IX.
- The vehicle is evidence-graded. Public values are authoritative where available; derived and approximate values are labeled explicitly.
- Interactive and cinematic quality are separate tiers. The interactive tier must run locally on the M3 Pro with 18 GB memory. The cinematic tier may render offline or use stronger hardware.
- `ascent-mcp` remains available for external AI agents but is not reused as the visualization transport.
- Tauri and React remain intact during the prototype.

## Architecture

Rust remains the sole authority for mission documents, simulation, evidence, events, and trace generation. Unreal owns presentation, playback controls, cameras, effects, inspection layers, and export. Unreal never advances an independent physics simulation or changes canonical engineering state.

Unreal launches and supervises a dedicated `ascent-visualizer-bridge` child process. The bridge uses the existing Ascent crates and communicates through framed stdio. A separate `ascent-visualizer-protocol` crate owns the versioned message schema and codec. This avoids network listeners, Rust/C++ ABI coupling, and misuse of the MCP interface.

The bridge runs the checked-in Black Brant IX reference mission on an in-memory document. It streams progress followed by deterministic `FlightTrace` metadata, channel chunks, events, and a canonical content hash. Unreal reconstructs the trace, validates ordering and identity, then owns read-only playback.

## Protocol

Each frame is a four-byte little-endian payload length followed by UTF-8 JSON. Frames larger than 16 MiB are rejected. Every message contains `protocol_version`, `message_id`, optional `request_id`, `kind`, and `payload`.

Client requests:

- `hello`
- `list_missions`
- `run_mission`
- `cancel_run`
- `shutdown`

Server messages:

- `hello_ack`
- `mission_catalog`
- `run_started`
- `run_progress`
- `trace_manifest`
- `trace_channel_chunk`
- `trace_events`
- `run_completed`
- `run_cancelled`
- `error`

Trace channels are emitted in canonical channel order and split into chunks of at most 1,024 samples. The bridge writes protocol bytes only to stdout and sends diagnostics to stderr. The initial catalog contains only `nasa.black-brant-ix.reference`.

## Coordinate and playback model

Rust positions remain launch-local East-North-Up in metres. Unreal maps them to centimetres using `X = North`, `Y = East`, and `Z = Up`. Attitudes are converted through the complete basis matrix, never by manually swapping quaternion components.

Position uses linear interpolation. Attitude uses normalized shortest-path slerp. Invalid or absent samples produce visible gaps and are never interpolated through. Events seek to exact recorded times. Stage transitions follow Rust events; Chaos physics never determines scientific state.

Playback provides play, pause, seek, frame step, reverse seek, range selection, and speeds of 0.25x, 0.5x, 1x, 2x, and 5x. Camera presets are pad, chase, onboard, ground tracking, and free inspection. Camera state is presentation-only.

## Reference mission and visual treatment

The Black Brant IX configuration is reconstructed from the NASA Sounding Rockets User Handbook and official NASA imagery. A bounded White Sands environment uses an offline USGS 3DEP terrain subset. Every source is checked in or mirrored as a hashed artifact with retrieval date, rights statement, and affected fields.

The vehicle uses reproducible procedural geometry with separable stages, physically based materials, panel seams, markings, and explicit notes for visual approximations. The scene uses restrained atmosphere, fog, sunlight, shadows, and a Niagara plume driven by Rust trace or event data.

Engineering layers include trajectory, velocity, body axes, attitude, uncertainty when present, stage state, and named events. Every displayed value resolves to a trace channel, event, or evidence record. One action removes all overlays for cinematic output.

## Quality tiers

The interactive tier targets at least 30 FPS in the editor and 45 FPS in a packaged 1920x1080 build on the M3 Pro with 18 GB memory. It uses medium scalability, bounded terrain, and restrained volumetrics.

The cinematic tier enables higher lighting, shadow, atmosphere, and antialiasing settings through Movie Render Queue. It exports PNG or EXR sequences on all supported systems and Apple ProRes when the platform plugin is available. Its manifest records the trace hash, protocol version, camera, time range, preset, resolution, frame rate, and evidence caveats.

## Failure behavior

- Handshake timeout is five seconds.
- Graceful shutdown timeout is two seconds.
- Unreal makes one automatic restart attempt after an unexpected bridge exit.
- Messages from an earlier process generation are ignored.
- Malformed, oversized, out-of-order, duplicated, or hash-invalid data stops the load and produces an actionable error.
- Cancelling a run leaves Unreal ready to start a new run.
- Unreal exit leaves no child process.

## Go/no-go gate

Unreal advances toward becoming the full Ascent application only when the realistic engineering scene is materially stronger than the existing viewport, the local interactive tier meets its performance targets, the Rust boundary remains clean, bridge failures are recoverable, and the review workflow feels like professional software.

If the gate fails, Unreal remains an optional visualizer and cinematic exporter. The prototype does not delete Tauri, React, or MCP in either outcome.

## Explicit exclusions

- Complete mission or vehicle authoring
- AI proposal and approval UI in Unreal
- Global Cesium terrain or photorealistic Earth
- Orbital flight, TVC, powered landing, or SpaceX-class behavior
- Historical-flight equivalence claims
- Network transport, remote bridge operation, installers, or signing
- Production marketplace assets without verified redistribution rights
