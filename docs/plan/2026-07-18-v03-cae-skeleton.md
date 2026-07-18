# Ascent v0.3 — CAE Paradigm Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. TDD throughout: failing test → minimal implementation → green → commit.

**Mission:** Ascent = **the AI-native aerospace simulation workbench**. Fluent/NX-class structure; two differentiators no incumbent has: an AI that operates the software as a first-class client of the command layer, and determinism + provenance + credibility as core architecture. Rocketry is the first domain pack, not the identity. (Intent confirmed 2026-07-18; research in `docs/CAE_PARADIGM_RESEARCH.md`.)

**Goal:** Build the full CAE paradigm skeleton — model tree, command spine + journal, study system, managed jobs, 6-DOF solver tier, post-processing environment, scripting console + headless runner, copilot seam, workbench shell. ~2-month horizon; this plan is the whole skeleton, each station real but not deep.

**Architecture:** The keystone inversion (design rule #1–#3 in the research doc): the model moves *into Rust*. A `Document` store in Tauri state owns the vehicle tree + studies; every mutation is a named serializable command through one dispatcher; the journal is append-only and replayable byte-identically; React becomes a thin client rendering state snapshots. GUI, console, CLI, and (later) the AI copilot are all clients of the same command stream.

**Tech Stack:** unchanged — Rust workspace, Tauri v2 coarse IPC, React 18 + TS strict + vitest (+ jsdom, test-only). New deps: none without explicit justification (wgpu decision stays deferred to the design pass).

## Global Constraints

- Determinism is sacred: identical inputs → identical `input_hash` and byte-identical summaries. Golden regression pin (`crates/ascent-app/tests/regression.rs`) green at every commit; journal replay must reproduce document state byte-identically.
- All existing tests (134 Rust + 38 vitest at plan time) stay green at every commit; new code lands test-first.
- No network at runtime; all data bundled or user-imported. Provenance JSON travels verbatim.
- SI internal everywhere; display conversion only in the UI layer. Coarse IPC only.
- Every physics claim ships with a reference (fixture, published data, or named assumption in the evidence report). The reference Alpha III must reproduce its current validated numbers (±1% apogee) when rebuilt as a tree, with any deltas documented in `docs/EVIDENCE.md`.
- The full visual design pass is still its own later track — Step 10 lands workbench *layout*, not final visual design.
- Commits end with: `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` (Claude) or Codex's standard trailer (Codex).

---

## Step 1: Vehicle model tree (data-blocks)

**Files:**
- Create: `crates/ascent-domain/src/vehicle.rs` (part tree + mass rollup)
- Create: `docs/VEHICLE_TREE.md` (schema spec: part types, parameters, invariants)
- Modify: `crates/ascent-domain/src/lib.rs` (export)
- Test: unit tests in `vehicle.rs`

**Interfaces:**
- Produces: `struct Vehicle { name: String, parts: Vec<Part> }` where `Part { id: PartId, kind: PartKind, children: Vec<Part> }` and `PartKind` covers `NoseCone { shape: NoseShape, length_m, base_radius_m, mass_g }`, `BodyTube { length_m, outer_radius_m, wall_mm, mass_g }`, `Transition`, `FinSet { count, root_chord_m, tip_chord_m, span_m, sweep_m, thickness_mm, mass_g }`, `MotorMount { motor_designation }`, `Parachute { diameter_cm, cd }`, `MassComponent { mass_g, position_m }`. `PartId` = stable u32 counter per vehicle. Axial positions derive from stacking order; every part has explicit mass.
- Produces: `fn mass_properties(v: &Vehicle) -> MassProperties { total_mass_g, cg_from_nose_m, longitudinal_moi_kg_m2 }` — summed per part with parallel-axis theorem (thin-shell/point approximations documented per part type in `VEHICLE_TREE.md`).
- Produces: `fn reference_vehicle() -> Vehicle` — the Alpha III as a tree, dimensioned to match today's validated configuration (25 mm diameter, matching dry mass and stack length).

**Acceptance:** mass rollup of `reference_vehicle()` matches today's `dry_mass_g` exactly; CG and MOI have hand-computed fixture tests (worksheet-style, like the Barrowman fixture); tree serializes to TOML and roundtrips byte-identically; a two-stage-style nested tree (parts with children) rolls up correctly.

## Step 2: Command spine + journal (the keystone)

**Files:**
- Create: `crates/ascent-app/src/document.rs` (document store: vehicle tree + design params + undo/redo stacks)
- Create: `crates/ascent-app/src/command.rs` (command enum, dispatcher, journal)
- Create: `docs/JOURNAL_FORMAT.md` (command grammar, replay semantics, versioning)
- Modify: `crates/ascent-app/src/lib.rs` (new IPC: `dispatch_command(cmd) -> DocumentState`, `get_document() -> DocumentState`, `undo()`, `redo()`)
- Modify: `app/src/App.tsx`, `app/src/core/commands.ts` (frontend history becomes thin: edits send commands, render returned state)
- Test: `command.rs` unit tests + replay determinism test

**Interfaces:**
- Produces: `enum Command { AddPart { parent: Option<PartId>, kind: PartKind }, RemovePart { id: PartId }, SetPartParam { id: PartId, param: String, value: Value }, SetSimParam { param: String, value: Value }, SelectMotor { designation: String }, … }` — serde-serializable, versioned (`schema_version` in the journal header).
- Produces: `struct Document { vehicle: Vehicle, …, past: Vec<Command>, future: Vec<Command> }` with `fn apply(&mut self, cmd: Command) -> Result<(), String>` and inverse computation for undo (each command stores or derives its inverse).
- Produces: journal = JSONL, one command per line, header line with schema version; `fn replay(journal: &str) -> Result<Document, String>`.
- Consumes: `Vehicle` from Step 1. Tauri state holds `Mutex<Document>`.

**Acceptance:** dispatch → undo → redo restores byte-equal document; journal replay of a 50-command session reproduces the live document byte-identically (serialize both, compare); an unknown command in a journal fails with line number; frontend Undo/Redo buttons and ⌘Z work through the new IPC (App.test.tsx updated); golden pin untouched.

## Step 3: One source of truth — derive aero, mass, and mesh from the tree

**Files:**
- Modify: `crates/ascent-aero/src/lib.rs` (Barrowman per-component from real tree geometry — replaces `planar_vehicle_for` provisional proportions)
- Modify: `crates/ascent-sim` callers (mass properties from Step 1 rollup instead of flat `dry_mass_g`)
- Modify: `app/src/core/mesh.ts` (`designToMesh` consumes the tree: real nose length/shape, real body length, real fin planform — kills the 12-caliber provisional)
- Modify: `docs/EVIDENCE.md` (document any numeric deltas from provisional → tree-derived geometry)
- Test: aero fixture tests against the Barrowman worksheet; mesh tests updated; cross-check test

**Interfaces:**
- Consumes: `Vehicle`, `mass_properties` from Step 1.
- Produces: `fn aero_model_for(v: &Vehicle) -> AeroModel` (per-component CNα and CP stations from actual dimensions); mesh part ranges named by `PartId` so selection can map tree ↔ viewport.
- Constraint: the reference vehicle's CP must stay within 0.5% of the current validated Barrowman fixture; apogee within ±1% of the golden pin — if the pin must move because geometry is now *more* correct, that is a deliberate, documented pin update in its own commit.

**Acceptance:** one struct drives sim, aero, and mesh — grep proves `planar_vehicle_for`'s provisional constants and mesh's `NOSE_CALIBERS`/`BODY_CALIBERS` are gone; fin dimension edit in the tree changes CP, mass, and rendered fins together; cross-validation vs OpenRocket fixture still within documented tolerance.

## Step 4: Study system

**Files:**
- Create: `crates/ascent-app/src/study.rs`
- Modify: `crates/ascent-app/src/project.rs` (project schema v2: studies persist with the design; v1 files still load)
- Modify: `crates/ascent-app/src/command.rs` (study CRUD commands: `CreateStudy`, `SetStudyParam`, `DeleteStudy`)
- Test: `study.rs` unit tests + project v1→v2 migration test

**Interfaces:**
- Produces: `struct Study { id: StudyId, name: String, kind: StudyKind, engine: String, seed: u64, results: Option<StudyResults> }` where `StudyKind` = `SingleFlight { config }`, `Dispersion { n_flights, scatter }`, `MotorTrade { candidates }`, `StabilitySweep { param, range }`. Results carry `input_hash` so staleness is provable, exactly like runs today.
- Consumes: `Document` from Step 2 (studies live in the document, mutate via commands, journal like everything else).

**Acceptance:** create/configure/delete studies via commands with undo working; project v2 roundtrips; loading a v1 project yields zero studies and no data loss; a study's `input_hash` goes stale when the vehicle tree changes.

## Step 5: Managed job runner

**Files:**
- Create: `crates/ascent-app/src/jobs.rs`
- Modify: `crates/ascent-app/src/lib.rs` (IPC: `submit_study(study_id) -> JobId`, `job_status(JobId)`, `cancel_job(JobId)`; Tauri events `job-progress`, `job-done`)
- Create: `app/src/components/JobsPanel.tsx` (thin: list jobs, progress bars, cancel buttons)
- Test: `jobs.rs` unit tests (progress, cancel, queue) + determinism test

**Interfaces:**
- Produces: `struct JobRunner` on a worker thread; jobs report `JobStatus { state: Queued|Running { pct }|Done|Cancelled|Failed { msg } }`; Monte Carlo dispersion reports per-fleet progress; results land back in the study via a completion command (journaled, so replay includes results provenance).
- Consumes: `Study` from Step 4, `SimEngine` implementations from ascent-sim.
- Constraint: cancellation is cooperative (checked between flights); a cancelled job leaves the study results untouched; determinism unaffected — same seed, same results, regardless of cancel/retry history.

**Acceptance:** 1000-flight dispersion runs as a background job with visible progress and responsive UI; cancel mid-run works; two queued jobs run in order; job completion updates the study and the byte-identical-on-same-seed test still passes.

## Step 6: 6-DOF solver tier (Codex track)

**Files:**
- Create: `docs/SIXDOF_DERIVATION.md` (contract first: state vector, quaternion kinematics, aero moments from the Barrowman model, assumptions and validity limits)
- Create: `crates/ascent-sim/src/sixdof.rs` (new `SimEngine` implementation)
- Test: fixture tests + cross-tier consistency tests

**Interfaces:**
- Produces: `SixDofEngine` implementing the existing `SimEngine` trait; state = position, velocity, quaternion attitude, angular rate; RK4 on the full state; aero forces/moments from the Step 3 tree-derived model; MOI from Step 1.
- Constraint: in calm wind with zero initial tilt, 6-DOF must reproduce the vertical golden pin apogee within 1e-6 relative; in the planar-wind cases it must match the 3-DOF weathercocking results within documented tolerance (this *is* the cross-validation seam working — document residuals like `docs/EVIDENCE.md` does). 3-DOF becomes the labeled "fast preview" tier.
- Regime honesty: roll dynamics and fin cant are out of scope — the credibility scorecard's Regime flag must mark quantities Extrapolated where 6-DOF leaves its validated envelope.

**Acceptance:** calm-wind 6-DOF reproduces the golden pin; planar cases match 3-DOF within tolerance with residuals documented; a tilted-rail case shows physically sensible (monotonic, bounded) attitude history; deterministic across runs.

## Step 7: Post-processing environment

**Files:**
- Create: `app/src/components/ResultsWorkspace.tsx` (own workspace, not a footer: plot area + study selector + comparison)
- Create: `app/src/core/plots.ts` (pure plot-data module: series extraction, axes, comparison overlays — SVG rendering stays thin)
- Modify: `app/src/App.tsx` (Results becomes a top-level mode alongside Design/Flight/Review)
- Test: `plots.test.ts` (pure), workspace smoke test (jsdom)

**Interfaces:**
- Produces: plot registry — trajectory (altitude/velocity vs t), stability margin vs t, dispersion scatter + p5/p50/p95 ellipse summary, multi-study overlay comparison; CSV export of any series (no new deps — hand-rolled CSV).
- Consumes: `StudyResults` from Steps 4–5; existing timeline/summary types.

**Acceptance:** two studies overlay on one plot with a legend; dispersion study renders scatter + percentile summary; exported CSV matches the plotted series exactly (test on fixture data); all plot-data functions pure with unit tests.

## Step 8: Scripting console + headless runner

**Files:**
- Create: `app/src/components/Console.tsx` (in-app console: journal-grammar commands in, dispatcher results out, history)
- Create: `crates/ascent-app/src/bin/ascent-cli.rs` (headless: `ascent-cli replay session.jsonl`, `ascent-cli run-study <project.ascent> <study>` → results JSON to stdout)
- Modify: `crates/ascent-app/src/command.rs` (text form of the command grammar per `docs/JOURNAL_FORMAT.md`: parse + pretty-print)
- Test: grammar roundtrip tests (parse → Command → print → parse), CLI integration test, console smoke test

**Interfaces:**
- Produces: every command has a canonical text form (e.g. `set-part-param 3 root_chord_m 0.05`); console and CLI both parse to the same `Command` enum and go through the same dispatcher — no second mutation path.
- Consumes: Steps 2, 4, 5. The CLI is also the CI story: golden-pin-style regression can run a journal headless.

**Acceptance:** a session driven entirely from the console (build vehicle, create study, run, read result) produces the same document as the same commands via GUI; `ascent-cli replay` of that journal reproduces it byte-identically; `ascent-cli run-study` output is deterministic and hash-stamped; text grammar roundtrips for every command variant.

## Step 9: Copilot seam (the AI-native contract)

**Files:**
- Create: `docs/COPILOT_INTERFACE.md` (the machine interface: how an agent reads document state, proposes a command batch, receives validation + evidence)
- Create: `crates/ascent-app/src/propose.rs` (`propose_batch(cmds) -> Proposal { valid, errors, diff_summary, evidence_refs }` — dry-run validation against a document clone, never mutating without approval)
- Modify: `crates/ascent-app/src/lib.rs` (IPC: `propose_commands`, `apply_proposal(id)`)
- Test: `propose.rs` unit tests

**Interfaces:**
- Produces: the propose/approve loop — an agent (or any client) submits a command batch; Ascent dry-runs it on a clone, returns a structured diff (what changes, which studies go stale) plus evidence/credibility references; applying requires a second explicit call. This is the approval-gated, physics-in-the-loop pattern the research identified as the open gap.
- Constraint: **no network, no LLM runtime in this step** — the seam is the deliverable. An external agent (Claude/Codex via CLI or MCP later) connects to it; Ascent itself stays offline and deterministic.
- Consumes: Steps 2, 4, 8 (proposals are journal-grammar batches).

**Acceptance:** a valid batch returns a correct diff summary without mutating the document; an invalid batch returns per-command errors with indices; apply-after-propose lands atomically and journals normally; a proposal touching sim inputs lists exactly the studies it would stale.

## Step 10: Workbench shell (layout skeleton, not the design pass)

**Files:**
- Create: `app/src/components/Workbench.tsx` (layout: model tree panel left, viewport center, properties right, console + jobs bottom, studies/results as workspace tabs)
- Create: `app/src/components/ModelTree.tsx` (render the vehicle tree, select part → properties panel + viewport highlight via Step 3's PartId ranges)
- Modify: `app/src/App.tsx` (compose existing panels into the workbench regions; dark professional theme tokens in one CSS module)
- Test: tree interaction tests (jsdom), layout smoke test

**Interfaces:**
- Consumes: everything — this step is composition. Tree selection drives the properties panel (Inspector generalizes from flat Design fields to per-part params via `SetPartParam` commands) and highlights the part's mesh range in Viewport3D.
- Constraint: still disposable presentation — plain CSS grid, no dock/layout library, no new deps. The dedicated visual design pass replaces the skin; this step proves the paradigm shape.

**Acceptance:** open project → tree on the left shows real parts → click a fin set → properties edit → viewport highlights and updates → create dispersion study → submit job → watch progress → open results workspace → compare two studies → type a command in the console and see it journal. One continuous session, all tests green.

---

## Track split

- **Codex track** (spec-heavy, isolated worktree): Step 6 (6-DOF: derivation doc first, then engine), plus contract docs for Steps 1–2 (`VEHICLE_TREE.md` schema review, `JOURNAL_FORMAT.md` grammar review) and Step 7's plot-math fixtures if idle.
- **Claude track** (shared-core, sequential): 1 → 2 → 3 → 4 → 5 → 8 → 9 → 10, with 7 slotted after 5. Steps 1–3 are the critical path; nothing else starts until the tree + command spine are in.
- Merge protocol unchanged: Codex works on `codex/remaining-track` in its worktree; merges resolved as unions; full suite green before and after every merge.
