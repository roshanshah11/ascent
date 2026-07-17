# Plan-Orchestrate Result

**Plan**: `docs/plan/2026-07-17-v02-professionalization.md`
**Lang**: `rust` (workspace) / `typescript` (app) — no rust/ts reviewer installed in the trimmed ECC set, so `ecc:code-reviewer` closes every chain
**ECC mode**: `plugin` (local `ecc:` namespace — matches the working commands in CHECKLIST.md)
**Steps**: 10
**Scope**: all

## Steps overview

| # | Title | Tags | Chain |
|---|---|---|---|
| 1 | Project file format + open/save | impl | `ecc:tdd-guide,ecc:code-reviewer` |
| 2 | Command-based undo/redo | impl | `ecc:tdd-guide,ecc:code-reviewer` |
| 3 | Motor registry + .eng import | impl | `ecc:tdd-guide,ecc:code-reviewer` |
| 4 | SimEngine trait | design, impl | `ecc:architect,ecc:tdd-guide,ecc:code-reviewer` |
| 5 | Wind + 3-DOF planar flight | impl | `ecc:tdd-guide,ecc:code-reviewer` |
| 6 | Monte Carlo dispersion | impl | `ecc:tdd-guide,ecc:code-reviewer` |
| 7 | Credibility scorecard | impl, docs | `ecc:tdd-guide,ecc:doc-updater,ecc:code-reviewer` |
| 8 | Autosave + crash recovery | impl | `ecc:tdd-guide,ecc:code-reviewer` |
| 9 | RocketPy bridge engine | impl | `ecc:tdd-guide,ecc:python-reviewer,ecc:code-reviewer` |
| 10 | 3D viewport foundation | impl | `ecc:tdd-guide,ecc:code-reviewer` |

## Batch execution (paste per step when ready)

```bash
/ecc:orchestrate custom "ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-1] Build the Project document layer in crates/ascent-app/src/project.rs: TOML project file (*.ascent, schema_version=1) holding designs + run records, save_project/load_project IPC, docs/PROJECT_FORMAT.md schema spec; Acceptance: TOML roundtrip byte-identical; unknown fields tolerated, future schema_version rejected with clear error; one-field git diff is one line; all 98 existing tests + golden pin stay green"

/ecc:orchestrate custom "ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-2] Add pure command-stack undo/redo in app/src/core/commands.ts (Command with apply/invert, past/future stacks, pushCommand/undo/redo/canUndo/canRedo), wire Design edits in App.tsx through it with Undo/Redo buttons and cmd-Z/shift-cmd-Z; Acceptance: apply-then-undo restores byte-equal design; redo clears on new edit; undo dispatches EDIT so run-state goes dirty; module has zero IPC imports"

/ecc:orchestrate custom "ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-3] Replace MOTOR_SOURCES const walk with MotorRegistry in ascent-domain plus a RASP .eng parser (eng_import.rs per docs/RASP_FORMAT.md) and import_motor_file IPC; validate against all 5 files in data/eng-samples/ + MANIFEST.json; Acceptance: all 5 samples parse to manifest impulse values; malformed .eng gives line-numbered error; bundled C6/B6/D12 behavior unchanged; imported motor flies end-to-end with provenance in the evidence report"

/ecc:orchestrate custom "ecc:architect,ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-4] Introduce SimEngine trait in crates/ascent-sim/src/engine.rs (id, version, run(rocket,motor,env,config)->SimSummary), wrap the existing solver as NativeEngine, route run_design through it, source EvidenceReport.engine from the trait; Acceptance: golden regression pin exact through the trait; MockEngine test double registers and is selectable; no behavior change anywhere else"

/ecc:orchestrate custom "ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-5] Add 3-DOF planar flight (x,z,pitch) with layered WindProfile and Barrowman-driven weathercocking in crates/ascent-sim/src/planar.rs, calm wind as default; Acceptance: calm-wind planar reproduces the vertical golden pin to 1e-9; 5 m/s crosswind drift sign/magnitude matches hand calc; fin-off design goes unstable in test; convergence report green at dt/2"

/ecc:orchestrate custom "ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-6] Build seeded Monte Carlo dispersion (crates/ascent-sim/src/dispersion.rs + run_dispersion IPC + thin DispersionView): distributions over thrust/Cd/wind/launch-angle/mass, ChaCha-seeded RNG, DispersionSummary with apogee p5/p50/p95 + landing ellipse; Acceptance: same seed gives byte-identical summary; zero sigma collapses to single-run values; 1000 samples under 5s release; percentiles match a hand-checked 10-sample fixture"

/ecc:orchestrate custom "ecc:tdd-guide,ecc:doc-updater,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-7] Add NASA-STD-7009-inspired credibility scorecard (crates/ascent-app/src/credibility.rs + docs/CREDIBILITY.md): 0-4 factors (verification, validation, input pedigree, uncertainty, regime) with cited basis strings, per-quantity Validated/Extrapolated regime flags, rendered in EvidenceDrawer; Acceptance: reference design scores deterministic; supersonic design flips apogee to Extrapolated with named reason; every score basis cites a file or fixture"

/ecc:orchestrate custom "ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-8] Add MS-Office-style autosave + crash recovery to the project layer: atomic temp-file autosave every 30s while dirty, never overwriting the user file, check_recovery/discard_recovery IPC, restore prompt on launch; Acceptance: hard kill leaves a recoverable autosave that roundtrips the full project; clean save deletes it; 100-run project autosaves under 100ms"

/ecc:orchestrate custom "ecc:tdd-guide,ecc:python-reviewer,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-9] Build the feature-gated RocketPy bridge engine (rocketpy_bridge.rs implementing SimEngine, bridges/rocketpy/run_ascent.py JSON-in/JSON-out, docs/CROSS_VALIDATION.md mapping + tolerance policy) and run_spread IPC showing native-vs-RocketPy apogee delta; Acceptance: frozen RocketPy fixture regression with documented tolerance; missing Python degrades gracefully; disagreement displayed never averaged; native determinism untouched"

/ecc:orchestrate custom "ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-17-v02-professionalization.md#step-10] Create pure procedural mesh module app/src/core/mesh.ts (designToMesh: tangent-ogive revolve, body cylinder, fin plates -> positions/normals/indices/part ranges, zero renderer imports) plus a thin Viewport3D with orbit camera; keep 2D SVG fallback; Acceptance: ogive profile matches the 0.466L equation from ascent-aero; fin count/positions match design; 3D view updates live on edit"
```

**Codex track** (spec-heavy, isolated): steps 3, 9, 10, and 7's CREDIBILITY.md. **Claude track** (shared core): 1, 2, 4, 5, 6, 8. Do 1→2→3 first ("it's software" milestone), then 4 unblocks 5/6/9.
