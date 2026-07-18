# V1 Tech Context — read this before building

*Written 2026-07-18 for a fresh session with zero conversation context. Companion to `2026-07-18-v1-roadmap.md` (the what/why) — this file is the how: stack decisions, packages, doc sources, and skills. Decisions here were made deliberately; do not relitigate them without new evidence.*

## Read-first order for a fresh session

1. `docs/plan/2026-07-18-v1-roadmap.md` — the V1 plan, five pillars, ambition level (visual bar: "the clip a Basilisk user stops scrolling for")
2. This file — stack + tooling
3. `docs/CAE_PARADIGM_RESEARCH.md` — why the architecture is command-spine-first
4. `docs/COPILOT_INTERFACE.md` + `docs/JOURNAL_FORMAT.md` — the two public contracts
5. `docs/plan/2026-07-18-v03-cae-skeleton.md` — the completed v0.3 plan (steps 1–9 landed, 6-DOF merged at `864cb08`; step 10 = shell is the next build)
6. `AGENTS.md` / `CHECKLIST.md` — repo conventions

Current state: **188 Rust + 62 vitest green.** Suite must stay green every commit; golden pin in `crates/ascent-app/tests/regression.rs` is sacred.

## Stack decisions (settled)

### Viewport & cinematics: React Three Fiber, in the frontend

**Decision: the ensemble viewport, timeline playback, journal cinema, and ghost proposals are built with `@react-three/fiber` + `@react-three/drei` in `app/`. wgpu stays out of the render path until v1.0 GPU compute.**

Why:
- Instancing is the whole ballgame for 1,000+ trajectory ribbons: one `InstancedMesh` = one draw call for hundreds of thousands of objects (R3F scaling-performance docs). Draw-call budget: a few hundred max; every repeated thing (ribbons, dispersion landing points, part meshes across ensemble ghosts) gets instanced.
- Cinematic cameras are solved there: `useFrame` render-loop hook + drei's `MotionPathControls`/`CameraControls` for chase/pad/onboard presets and scrubber-driven camera paths.
- The existing hand-rolled canvas viewport (v0.2 step 10) gets replaced, not extended. The pure mesh module (`app/src/core/` procedural mesh, zero renderer imports) survives — it feeds R3F geometry instead.
- Rust-side wgpu render viewport was considered and rejected for V1: it would put a second UI technology in the app, and the React shell already renders snapshots-only. R3F respects the same invariant: **the render layer dispatches zero commands** — drags preview locally, drop dispatches exactly one journaled command.

Packages (add to `app/`, justify-on-add rule satisfied here):
- `three` + `@react-three/fiber` — renderer
- `@react-three/drei` — CameraControls, MotionPathControls, Instances, Line2 helpers
- `@types/three` dev
- NOT `@react-three/rapier` (we have our own physics — never let a game physics engine near the sim), NOT postprocessing until the look demands it.

Performance rules from the docs: instanced everything; mutate matrices in `useFrame`, never via React state; `frameloop="demand"` when idle (determinism-friendly — render only on snapshot/scrub change); one `<Canvas>` for the whole workbench.

### GPU compute (v1.0): wgpu, Rust-side, headless

**Decision: 10k-flight GPU dispersion is a headless wgpu compute pipeline inside `ascent-sim` (feature-gated crate or `ascent-gpu`), never touching the window.**

- Pattern is wgpu's `hello_compute` standalone example: device/queue without surface → WGSL module → `create_compute_pipeline` → dispatch → buffer readback. No winit, no surface — works in CI.
- Determinism claim (byte-identical to CPU) is the hard part: WGSL f32 arithmetic must match the CPU path or the claim narrows to "statistically identical + documented tolerance." Resolve at build time with a cross-check test in CI (CPU 1000-flight vs GPU 1000-flight, same SplitMix64 streams). If byte-identity proves impossible across drivers, the honest fallback is per-platform golden pins — decide then, document either way.
- wgpu is already in the dep tree transitively via nothing today — it is a NEW dep, justified only when the GPU step actually starts. Do not add it early.

### Shell & IPC: Tauri v2 (existing), channels for streaming

- Keep coarse IPC (whole DocumentState snapshots). For 60fps playback/scrub data and job progress at higher rates, use Tauri v2 **channels** (ordered streaming, made for this) instead of event spam — the `tauri-v2` skill (installed, see below) covers invoke/emit/channel patterns and capability config.
- The workbench shell (roadmap step 1) is plain React + CSS — dockable panels do NOT need a library yet; start with a CSS-grid workspace layout and the command palette. Add a dock library only if panels genuinely need drag-rearrange (justify-on-add).

### UI/UX design language

- Dark mission-control aesthetic, typography-first. No component framework (no MUI/Chakra — they fight custom aesthetics); hand-rolled components + CSS custom properties for theming, as the codebase already does.
- Motion: CSS transitions for chrome; R3F `useFrame` for scene; the timeline scrubber is the one global time source — every view derives from it (single source of truth, same philosophy as the model tree).
- Command palette: fuzzy match over `Command::to_text()` grammar + named UI actions. Grammar already exists; palette is mostly free.

## Doc sources (context7 IDs — query these while building, don't guess APIs)

| Topic | Context7 ID | Use for |
|---|---|---|
| React Three Fiber | `/pmndrs/react-three-fiber` | instancing, useFrame, canvas setup, performance scaling |
| drei helpers | resolve `@react-three/drei` when needed | CameraControls, MotionPathControls, Instances |
| wgpu | `/gfx-rs/wgpu` (versioned: v26/v25) | headless compute pipeline, buffer readback |
| Learn Wgpu tutorial | `/websites/sotrh_github_io_learn-wgpu` | first compute pass walkthrough |
| Tauri v2 | via installed `tauri-v2` skill first; context7 `Tauri` if deeper | channels, capabilities, bundling/signing |
| Three.js core | resolve `Three.js` when needed | Line2/fat lines, BufferGeometry for ribbons |

## Skills installed for this build

- **`tauri-v2`** (global, 6K installs) — invoke/emit/channels/capabilities/build issues. Trigger it for any `tauri.conf.json`, IPC, or bundling work.
- Searched, not found/not worth it: no wgpu skill exists (use context7 + Learn Wgpu); three.js skills on skills.sh are low-install game-scene generators — skip, R3F official docs via context7 are better.
- Already-available local skills that apply: `superpowers:test-driven-development` (every step lands test-first), `superpowers:writing-plans` (per-release plan docs), `superpowers:verification-before-completion`, ECC `tdd-guide`/`code-reviewer` chains per the orchestrate commands already emitted (see roadmap conversation; commands start `/ecc:orchestrate custom ...` with `ecc:` prefix — rust-reviewer is NOT installed, use `ecc:code-reviewer`).

## MCP server (roadmap step 4)

- Spec finalizes **2026-07-28** — build `ascent-mcp` against the final spec, not the RC. stdio transport; official Rust SDK (`rmcp`) — resolve on context7 when starting; adopt the **Tasks extension** for long-running dispersion jobs.
- The server is a thin adapter over `propose_batch`/`apply_batch`/`cli::run_study` — zero new mutation logic. If the SDK fights the no-network invariant, hand-roll JSON-RPC over stdio; the protocol is small.

## Invariants (restated so a fresh session cannot miss them)

1. Every mutation is a `Command` through the one dispatcher — GUI, console, CLI, MCP, extensions, **and the future viewport drag-drop**.
2. Journal replays byte-identically; golden pin green every commit; full suite green every commit.
3. Render layer dispatches zero commands; consumes snapshots/journals only.
4. No runtime network. External data (wind profiles, DEM tiles, altimeter logs) enters as files, hashed into study input hashes.
5. New dependency = written justification in the commit message.
6. Nothing lands half-done. No placeholders, no "fix later."
7. Commits end `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

## Build order (next session starts here)

1. **Step 1 (shell)** — workspace layout + command palette + mission-control styling. No new deps.
2. **R3F viewport foundation** — replace hand-rolled canvas: `three` + `@react-three/fiber` + `@react-three/drei` land here with justification; port procedural mesh module to R3F geometry; single-vehicle view + CP/CG overlays first, ensemble second.
3. **Steps 3–5 of the orchestrate batch** (multi-stage, ascent-mcp after 07-28, workspace pass) in roadmap order.
4. Run the app: `cargo install tauri-cli --locked` (once), `npm install` in `app/` (once), then `cargo tauri dev` from `crates/ascent-app/`.
