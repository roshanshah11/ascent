# Ascent v0.2 — Professionalization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. TDD throughout: failing test → minimal implementation → green → commit.

**Goal:** Turn Ascent v0.1 (validated demo) into real software: document layer, registry layer, physics ladder start, trust framework — per `docs/PROFESSIONAL_SOFTWARE_RESEARCH.md`.

**Architecture:** Keep strict layering (Core → Document → Interaction → UI). Everything user-owned flows through a new document layer (plain-text project file + command-based undo). Everything pluggable (motors, engines, rule packs) moves behind factory registries. Physics grows 1-DOF → 3-DOF+wind → Monte Carlo, each gated by external validation. UI redesign is a separate post-Jul-23 track (brainstorming skill first) — this plan only adds backend seams the redesign will need.

**Tech Stack:** Rust workspace (ascent-domain/sim/aero/review/app), Tauri v2 coarse IPC, React 18 + TS strict + vitest, serde/serde_json, SHA-256 provenance. New: none until Step 10 (wgpu decision deferred).

## Global Constraints

- Determinism is sacred: identical inputs → identical `input_hash` and byte-identical summaries. Every step keeps the golden regression pin green (`crates/ascent-app/tests/regression.rs`).
- All 98 existing tests stay green at every commit; new code lands test-first.
- No network at runtime; all data bundled or user-imported. Provenance JSON travels verbatim with every data file.
- SI units internal everywhere; display conversion happens only in the UI layer.
- Coarse IPC only — no chatty invoke patterns.
- Every physics claim ships with a reference (fixture, published data, or named assumption in the evidence report).
- Commits end with: `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` (Claude) or Codex's standard trailer (Codex).

---

## Step 1: Project file format + open/save (Document layer, part 1)

**Files:**
- Create: `crates/ascent-app/src/project.rs` (serialize/deserialize `Project`)
- Create: `docs/PROJECT_FORMAT.md` (schema spec, versioning policy)
- Modify: `crates/ascent-app/src/lib.rs` (new IPC: `save_project`, `load_project`)
- Test: `crates/ascent-app/src/project.rs` unit tests + roundtrip test

**Interfaces:**
- Produces: `struct Project { schema_version: u32, name: String, designs: Vec<Design>, runs: Vec<RunRecord> }`; `fn to_toml(p: &Project) -> Result<String, String>`; `fn from_toml(s: &str) -> Result<Project, String>` with forward-compat unknown-field tolerance.
- Format: TOML, human-diffable (the anti-.ork), `schema_version = 1`, one file `*.ascent`. Runs embed their full summary + input_hash so a loaded project can prove staleness.

**Acceptance:** roundtrip test (Project → TOML → Project → TOML byte-identical); loading a file with unknown extra fields succeeds; loading `schema_version = 999` fails with a clear message; git-diff of two saved files with one changed fin dimension shows only that line.

## Step 2: Command-based undo/redo (Document layer, part 2)

**Files:**
- Create: `app/src/core/commands.ts` (pure command stack module)
- Modify: `app/src/App.tsx` (route edits through commands; Undo/Redo buttons + ⌘Z/⇧⌘Z)
- Test: `app/src/core/commands.test.ts`

**Interfaces:**
- Produces: `type Command = { label: string; apply(d: Design): Design; invert(d: Design): Design }`; `pushCommand`, `undo`, `redo`, `canUndo`, `canRedo` over past/future stacks. Design edits become `setField(path, oldValue, newValue)` commands.
- Consumes: existing `Design` type from `app/src/core/types.ts`; existing `EDIT` dispatch in run-state machine (undo/redo also dispatch `EDIT` — an undone design is a dirty design).

**Acceptance:** pure-module tests: apply→undo restores byte-equal design; redo stack clears on new edit; 50-deep history works; run-state goes dirty on undo. No IPC involved.

## Step 3: Motor registry + full .eng import (Registry layer, part 1)

**Files:**
- Modify: `crates/ascent-domain/src/lib.rs` or new `crates/ascent-domain/src/registry.rs` (MotorRegistry replacing `MOTOR_SOURCES` const walk)
- Create: `crates/ascent-domain/src/eng_import.rs` (RASP .eng parser per `docs/RASP_FORMAT.md`)
- Modify: `crates/ascent-app/src/design.rs` + `review_ipc.rs` (consume registry)
- Test: parser tests against all 5 files in `data/eng-samples/` + MANIFEST.json expectations

**Interfaces:**
- Produces: `struct MotorRegistry`; `fn bundled() -> MotorRegistry` (C6/B6/D12 as today); `fn register_eng(&mut self, source: &str, provenance: serde_json::Value) -> Result<Designation, String>`; `fn get(&self, designation: &str) -> Option<&Motor>`; `fn list(&self) -> Vec<MotorInfo>`.
- Consumes: existing `Motor` struct + provenance pattern; Codex A6 corpus (`data/eng-samples/`, `docs/RASP_FORMAT.md`) — this is exactly what it was staged for.
- New IPC: `import_motor_file(contents: String)` → adds to session registry.

**Acceptance:** all 5 sample .eng files parse to correct total impulse (vs MANIFEST.json values); malformed .eng gives line-numbered error; bundled motors unchanged (existing tests untouched); imported motor flyable end-to-end in a sim run with provenance carried into the evidence report.

## Step 4: SimEngine trait — the cross-validation seam (Registry layer, part 2)

**Files:**
- Create: `crates/ascent-sim/src/engine.rs` (trait + native impl wrapper)
- Modify: `crates/ascent-app/src/design.rs` (`run_design` goes through the trait)
- Test: trait-object test proving native engine through the trait produces the golden-pin summary

**Interfaces:**
- Produces: `trait SimEngine { fn id(&self) -> &'static str; fn version(&self) -> &'static str; fn run(&self, rocket: &Rocket, motor: &Motor, env: &Environment, config: &SimConfig) -> Result<SimSummary, String> }`; `struct NativeEngine` (wraps today's solver); `fn engines() -> Vec<Box<dyn SimEngine>>` factory.
- Purpose: engine id+version go into `EvidenceReport.engine`; later steps add RocketPy/OpenRocket bridge engines behind the same trait, making "run the spread" a loop.

**Acceptance:** golden regression pin still exact through the trait; `EvidenceReport.engine` sources from `NativeEngine::id()/version()`; a `MockEngine` test double proves a second engine can register and be selected.

## Step 5: Wind + 3-DOF planar flight (Physics ladder, part 1)

**Files:**
- Create: `crates/ascent-sim/src/planar.rs` (3-DOF: x, z, pitch; wind profile; weathercocking)
- Modify: `crates/ascent-sim/src/lib.rs` (SimConfig gains `wind: WindProfile`; default = calm = existing behavior)
- Test: analytic fixtures (zero wind ⇒ matches existing vertical solver to 1e-9; constant crosswind ⇒ drift matches closed-form ballistic approximation) + convergence report extended to planar

**Interfaces:**
- Produces: `struct WindProfile { layers: Vec<(altitude_m: f64, speed_ms: f64, direction_deg: f64) > }`; `fn simulate_planar(...) -> PlanarSummary` (superset of SimSummary: adds `landing_range_m`, `max_aoa_deg`, `weathercock_deg`).
- Consumes: `ascent-aero` CP/CNα (Barrowman gives the restoring moment); existing atmosphere + events.
- Rule: calm-wind planar must reproduce the vertical golden pin — the old solver becomes the validated degenerate case.

**Acceptance:** calm-wind equivalence test to 1e-9; drift direction and magnitude sign-correct vs hand calc for 5 m/s crosswind; stability margin from ascent-aero actually drives the pitch dynamics (fin-off design goes unstable in test); convergence report green at dt/2.

## Step 6: Monte Carlo dispersion (Physics ladder, part 2 — the OpenRocket-beater)

**Files:**
- Create: `crates/ascent-sim/src/dispersion.rs` (seeded RNG, parameter distributions, N-run driver)
- Create: `crates/ascent-app/src/dispersion_ipc.rs` + IPC command `run_dispersion`
- Create: `app/src/components/DispersionView.tsx` (landing scatter + apogee histogram; thin UI, redesign later)
- Test: statistical fixtures with fixed seed

**Interfaces:**
- Produces: `struct Dispersion { seed: u64, samples: u32, vary: Vec<Variation> }` where `Variation = { param: VaryParam, sigma: f64 }` (thrust ±%, Cd ±%, wind speed ±m/s, launch angle ±deg, mass ±g); `struct DispersionSummary { apogee_p5/p50/p95, landing_ellipse: (a_m, b_m, bearing_deg), runs: Vec<CompactRun> }` — deterministic given seed (ChaCha8 or similar seeded RNG; no OS entropy).
- Consumes: Step 5 planar engine; Step 4 trait (dispersion driver is engine-agnostic).

**Acceptance:** same seed ⇒ byte-identical DispersionSummary (determinism holds); zero-sigma dispersion ⇒ all runs identical to single run; 1000-sample run < 5 s release build; percentiles verified against a hand-checked 10-sample fixture; evidence report extended with seed + distributions.

## Step 7: Credibility scorecard — NASA-7009-inspired trust framework

**Files:**
- Create: `crates/ascent-app/src/credibility.rs` (scorecard model + per-quantity regime flags)
- Create: `docs/CREDIBILITY.md` (the factor definitions, mapped to NASA-STD-7009B categories, cited)
- Modify: `crates/ascent-app/src/evidence.rs` (EvidenceReport gains `credibility: Scorecard`)
- Modify: `app/src/components/EvidenceDrawer.tsx` (render scorecard + regime badges)
- Test: regime-boundary tests

**Interfaces:**
- Produces: `struct Scorecard { factors: Vec<Factor> }` where `Factor = { name, score: u8 /* 0-4 */, basis: String }` — factors: Verification (convergence), Validation (fixture agreement), Input Pedigree (motor provenance), Uncertainty (dispersion run or not), Operating Regime; `enum Regime { Validated, Extrapolated(String) }` per output quantity (e.g. Mach > 0.8 ⇒ Extrapolated("transonic — Barrowman incompressible model outside validated regime")).
- Consumes: existing evidence machinery; `docs/EVIDENCE.md` tolerances become the Validation factor's basis strings.

**Acceptance:** reference design scores computed deterministically; a design pushed supersonic flips apogee to `Extrapolated` with the named reason; drawer shows the 0–4 spider values; every score's `basis` cites a file or fixture (no unexplained numbers — house rule).

## Step 8: Autosave + crash recovery (Document layer, part 3)

**Files:**
- Modify: `crates/ascent-app/src/project.rs` (autosave path management, atomic temp-file write)
- Modify: `app/src/App.tsx` (30 s dirty-timer autosave via IPC; recovery prompt on launch)
- Test: Rust-side atomic-write + recovery-detection tests

**Interfaces:**
- Produces: `fn autosave(p: &Project) -> Result<PathBuf, String>` (write temp + rename, never overwrite user file); `fn find_recovery() -> Option<(PathBuf, Project)>`; IPC `check_recovery`, `discard_recovery`.
- Pattern: MS-Office style — separate autosave location, delete stale autosave on clean save/exit, offer restore on next launch.

**Acceptance:** kill -9 during test harness leaves recoverable autosave; clean save deletes it; recovery roundtrips the full project; autosave of a 100-run project < 100 ms.

## Step 9: RocketPy bridge engine (Trust framework — cross-validation goes live)

**Files:**
- Create: `crates/ascent-sim/src/rocketpy_bridge.rs` (config export + result import; subprocess, feature-gated `bridge-rocketpy`)
- Create: `bridges/rocketpy/run_ascent.py` (thin script: JSON in → RocketPy Flight → JSON out)
- Create: `docs/CROSS_VALIDATION.md` (mapping table Ascent↔RocketPy params, known model differences, tolerance policy)
- Test: golden fixture from one manual RocketPy run (same pattern as Day 3's OpenRocket fixture)

**Interfaces:**
- Produces: `struct RocketPyEngine` implementing Step 4's `SimEngine`; `struct EngineSpread { engines: Vec<(id, SimSummary)>, apogee_spread_m: f64 }`; IPC `run_spread(design)`.
- Consumes: Step 4 trait; RocketPy ≥ 1.4 (MIT, peer-reviewed, ~1% validated — the ally engine per IDEA_SPEC).
- Honesty rule: bridge is optional (feature flag + graceful "RocketPy not installed"); disagreement is displayed, never averaged.

**Acceptance:** frozen fixture regression (RocketPy output for reference design pinned, tolerance documented in CROSS_VALIDATION.md); spread view shows native vs RocketPy apogee side by side with delta; absent Python environment degrades gracefully; determinism of native path untouched.

## Step 10: 3D viewport foundation (Presentation seam — pre-work for the redesign)

**Files:**
- Create: `app/src/core/mesh.ts` (procedural mesh generation from Design: nose revolve, body cylinder, fin plates — pure, tested)
- Create: `app/src/components/Viewport3D.tsx` (Three.js or `<canvas>`+WebGPU render of the mesh; camera orbit; thin)
- Test: `app/src/core/mesh.test.ts` (vertex counts, symmetry, dimensions match Design fields)

**Interfaces:**
- Produces: `fn designToMesh(d: Design): { positions: Float32Array, normals: Float32Array, indices: Uint32Array, parts: PartRange[] }` — pure function, no renderer dependency, so the post-Jul-23 redesign can swap the renderer freely (wgpu-native later if webview WebGPU underperforms; decision deferred per research doc §5).
- Consumes: Design geometry fields; per research: **no CAD kernel** — bodies of revolution + plates cover the market.

**Acceptance:** mesh tests green (ogive profile matches the 0.466·L tangent-ogive equation used in ascent-aero; fin count/positions match design); 3D view renders reference rocket and updates live on edit; 2D SVG viewport retained as fallback.

---

## Sequencing and ownership

- **Steps 1→2→3 are the "it's software now" milestone** — do these first, in order (2 depends on nothing but types; 1 and 3 are parallelizable).
- **Steps 4→5→6 are the physics track** — 4 is small and unblocks both 5/6 and 9. 5 gates 6.
- **Step 7 anytime after 4; Step 8 anytime after 1; Step 9 after 4; Step 10 anytime** (pure frontend, good Codex track).
- Codex-friendly (spec-heavy, isolated, testable): **3 (eng parser), 9 (bridge script + mapping doc), 10 (mesh module), 7's CREDIBILITY.md**. Claude-track: 1, 2, 4, 5, 6, 8 (touch shared core).
- Cadence per research doc: ship each step as its own commit chain; nothing waits for a big-bang.
