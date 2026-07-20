# Ascent — Design

*Architecture overview. The what/why lives in `docs/PRODUCT_DIRECTION.md`; the current build plan lives in `docs/plan/2026-07-18-v1-roadmap.md` and `docs/plan/2026-07-18-v1-tech-context.md`. This document is the map between them: how the system is decomposed and which invariants every part must honor.*

## 1. What this describes

Ascent is a local-first aerospace engineering workbench for modeling, simulation, uncertainty analysis, verification, and evidence. Its north star is *Basilisk-grade rigor inside a real workbench that an AI agent can operate*. The measuring user is a GNC or flight-dynamics engineer at a new-space or defense-tech startup.

The design exists to serve five engineering priorities: validated physics with named baselines, bit-for-bit repeatability with provenance, modular dynamics/environment/vehicle interfaces, professional workflows (model tree, studies, verification, reports), and **one audited command surface shared by GUI, CLI, console, and MCP agents**.

## 2. Core invariants

These are load-bearing. Every module is designed around them; a change that breaks one is a design change, not a bug fix.

1. **Single command spine.** Every mutation of user-owned state is a named, serde-serializable `Command` applied through one `Document` dispatcher. GUI, console, CLI, MCP, and the future viewport drag-drop are all *clients* of that dispatcher — none mutate state directly.
2. **Deterministic replay.** The journal of commands replays byte-identically. Every engine is `same inputs → same bytes`. The golden pin in `crates/ascent-app/tests/regression.rs` is sacred and green on every commit.
3. **Render layer dispatches zero commands.** Views consume snapshots and journals only. A drag previews locally; a drop dispatches exactly one journaled command.
4. **No runtime network.** All external data (motor curves, wind/density soundings, DEM tiles, altimeter logs) enters as files, hashed into study input hashes. Bundled data is embedded via `include_str!`.
5. **Explicit provenance and fidelity.** Every physics claim is traceable to a named baseline; results carry input hashes, model versions, assumptions, and convergence status.
6. **New dependency = written justification** in the commit message. Nothing lands half-done.

## 3. Crate topology

A Rust workspace (`Cargo.toml`) plus a Tauri v2 + React/Vite frontend in `app/`. Dependencies flow one direction — domain has zero UI/engine imports, engines take plain numbers, the app crate wires everything and owns IPC.

```
ascent-domain   motors + vehicle model tree           (no renderer, no UI, no engine)
      │
      ├── ascent-aero    Barrowman CP, CG, stability, rule-pack constraints
      │
      ├── ascent-sim     flight engines: 1-DOF / 3-DOF planar / 6-DOF / dispersion / staged
      │
      ├── ascent-review  requirements verification + repair solver + structural margins
      │
      └── ascent-app     command spine, Document, studies, jobs, evidence, DTOs, Tauri IPC, CLI
             │
             ├── ascent-mcp   thin MCP stdio adapter over propose/apply/run_study
             │
             └── app/         React + R3F frontend (snapshots-only)
```

### `ascent-domain` — foundations
Zero renderer, zero UI, zero engine imports. Owns the two things everything else builds on:
- **Motors** (`motor.rs`, `eng_import.rs`, `registry.rs`): thrust/mass curves with RASP provenance, `.eng` parsing, a registry of bundled + imported motors.
- **Vehicle model tree** (`vehicle.rs`): `Vehicle` / `Part` / `PartKind` hierarchy, mass rollups, `stages()`, and reference fixtures. The tree — not a flat parameter list — is the source of truth; the flat sim parameters are *derived* from it upstream.

### `ascent-aero` — aerodynamics & constraints
Barrowman center-of-pressure (`barrowman.rs`, worksheet in `docs/BARROWMAN_WORKSHEET.md`), time-varying CG, stability margin in calibers (`stability.rs`), and a rule-pack constraint engine (`constraints.rs`) whose checks cite their rule source. IREC 2026 / DTEG ships here as *one optional versioned rule pack*, never as the product identity.

### `ascent-sim` — flight engines
All engines are deterministic and take plain numbers; tree→parameter derivation lives upstream. The curated surface:
- `sim`: RK4 1-DOF vertical + convergence report
- `planar`: 3-DOF planar, staged variant, wind profiles
- `sixdof`: 6-DOF engine, staged, 3D wind, launch config
- `dispersion`: Monte-Carlo ensembles over seeded parameter variations, landing ellipses
- `atmosphere` / `rocket` / `profile`: 1976 standard atmosphere, drag/recovery models, imported soundings
- `engine`: the `SimEngine` trait + registry so engines are swappable

Determinism note: dispersion uses reproducible SplitMix64 streams so an ensemble is byte-stable across runs. The eventual GPU dispersion path (v1.0, headless wgpu) must cross-check byte-for-byte against the CPU path or narrow its claim to a documented tolerance — see tech-context.

### `ascent-review` — verification & repair
Constraint panel bound to the cited rule pack, a **deterministic** motor-enumeration + ballast-bisection solver that repairs an infeasible configuration toward an apogee target inside a stability window, before/after diff, and structural margin checks (`structural.rs`).

### `ascent-app` — the spine and services
Owns the command layer and everything downstream of it. Curated (non-IPC) exports are what `ascent-cli` and `ascent-mcp` build on — see §4. IPC is **coarse**: the UI sends a whole `Design` and gets a whole `RunRecord` back; no fine-grained property calls cross the bridge. All physics stays in domain/sim; this crate maps DTOs and downsamples trajectories for playback.

### `ascent-mcp` — agent adapter
A thin stdio MCP adapter (official `rmcp` SDK) over `propose_batch` / `apply_batch` / `cli::run_study`. Zero new mutation logic — agents go through the same audited spine as humans, adopting the Tasks extension for long-running dispersion jobs.

## 4. The command spine

The heart of the architecture (`crates/ascent-app/src/command.rs`, grammar contract in `docs/JOURNAL_FORMAT.md`).

- `Command` is a `#[serde(tag = "cmd")]` enum — `AddPart`, `RemovePart`, `RestorePart`, `SetPartParam`, `SetSimParam`, study commands, etc. Each variant is serializable, has a text-grammar form (`to_text()`), and is journaled.
- Every mutation flows through `Document::apply`, which returns a **concrete forward form plus its inverse**. This is the undo-fidelity rule: `AddPart` allocates an id at apply time, so its concrete form is a `RestorePart` carrying the resolved part — redo reproduces the *identical* document rather than allocating a fresh id.
- `SetPartParam` validation is by construction: the patched part must deserialize back into a valid `PartKind`. Parameter names are the serde field names documented in `docs/VEHICLE_TREE.md`.

Because every client (GUI, CLI, console, MCP, future viewport) emits `Command`s and only `Command`s, the journal is a complete, replayable, auditable record of user intent — the foundation for both undo/redo and reproducibility.

### Copilot seam
AI does not mutate directly. It produces typed `Proposal`s via `propose_batch`; a human (or an explicit approval path) applies them via `apply_batch`, with `CommandCheck` / `DiffSummary` surfacing exactly what would change. Contract: `docs/COPILOT_INTERFACE.md`.

## 5. Data & provenance model

- **Projects** serialize to/from TOML (`project.rs`, format in `docs/PROJECT_FORMAT.md`).
- **Studies** (`study.rs`) bundle a design + engine config; `study_input_hash` folds every input — including imported file hashes — into a single content hash. `JobRunner` runs studies (long ones async, streaming `JobEvent`s).
- **Evidence** (`evidence.rs`): `evidence_for` assembles input hash, model versions, assumptions, motor provenance, and a live convergence check into an immutable `EvidenceReport`. Cross-validation against external baselines is recorded in `docs/EVIDENCE.md` / `docs/CROSS_VALIDATION.md` with explicit agree/differ/why tolerances.
- **Credibility** (`credibility.rs`): a `Scorecard` of `Regime` / `QuantityFlag` factors so a result advertises its own fidelity honestly.

## 6. Frontend (`app/`)

Tauri v2 shell, React + Vite, hand-rolled components + CSS custom properties (no component framework — they fight the dark mission-control aesthetic). Key design rules:

- **Snapshots-only.** The frontend never simulates; it renders `RunRecord`s and journals. Playback is driven entirely by the stored run record — zero sim calls during playback.
- **Presentation-independent core.** `app/src/core/` (run-state machine, playback engine, command diff, procedural mesh) has zero renderer imports and is unit-tested independently. The visible skin is deliberately swappable.
- **Viewport → R3F.** The 3D viewport, ensemble ribbons, timeline cinema, and ghost proposals are `@react-three/fiber` + `@react-three/drei`. Everything repeated (trajectory ribbons, dispersion landing points, ensemble ghosts) is instanced; matrices mutate in `useFrame`, never via React state; `frameloop="demand"` keeps it determinism-friendly.
- **One time source.** The timeline scrubber is the single global time source; every view derives from it.
- **Streaming.** Coarse IPC for state snapshots; Tauri v2 **channels** for 60fps playback/scrub data and job progress.

## 7. Testing & verification

- Full suite green on every commit (Rust + vitest). The golden regression pin freezes the headless summary to 1e-9 — a clean restart must reproduce the identical summary with zero network.
- Physics is validated against named baselines with explicit tolerances (OpenRocket golden fixture at `data/reference/`, Barrowman worksheet cross-check, analytic no-drag closed form).
- New behavior lands test-first (`superpowers:test-driven-development`); completion is gated on `superpowers:verification-before-completion`.

## 8. Reading order for a new contributor

1. `docs/PRODUCT_DIRECTION.md` — what Ascent is and is not
2. This file — the architecture map
3. `docs/plan/2026-07-18-v1-roadmap.md` + `docs/plan/2026-07-18-v1-tech-context.md` — plan and stack
4. `docs/CAE_PARADIGM_RESEARCH.md` — why the design is command-spine-first
5. `docs/COPILOT_INTERFACE.md` + `docs/JOURNAL_FORMAT.md` — the two public contracts
6. `AGENTS.md` / `CHECKLIST.md` — repo conventions and status
