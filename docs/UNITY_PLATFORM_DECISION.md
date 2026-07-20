# Unity Platform Decision — Black Brant IX Vertical Slice

**Decision: `contain`** (as of 2026-07-20, branch `feat/unity-visual-engine`)

Unity is retained as an *optional* visualizer and cinematic exporter. It does
not become the primary platform. This decision follows the seven predeclared
criteria in `docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md`
(Step 5.4): `expand` requires **all seven** to pass; otherwise `contain`. Three
criteria are not yet satisfiable from evidence, so the rule yields `contain`.
This is a recorded-evidence decision, not an enthusiasm call.

## Gate results (Step 5.3)

Run the machine-verifiable gates with `cargo xtask visualizer-test` (opt-in;
never part of `cargo xtask test`/`ci`). It reports each gate distinctly and
fails closed with a version-named prerequisite error if the pinned editor
(`6000.0.79f1`) is absent — it never silently skips.

| Gate | Result | Evidence |
|------|--------|----------|
| Rust protocol | **PASS** | `cargo test -p ascent-visualizer-protocol` green; fixtures + fail-closed decoder |
| Bridge subprocess | **PASS** | `cargo test -p ascent-visualizer-bridge --test subprocess`; hello/list/run/cancel/shutdown over real stdio |
| Rust workspace | **PASS** | `cargo test --workspace` = 314 passed, 4 ignored |
| Unity EditMode | **PASS** (last: commit `e663a55`) | 36 tests: decoder, coordinate basis, trace assembly, playback, layer binding, scene structure |
| Unity PlayMode | **PASS** (last: commit `e663a55`) | 5 tests: full review path vs real bridge (play / seek-every-event / 5 cameras / clean view / rerun-identical hash / export) + cancel + bridge lifecycle |
| Rerun hash identity | **PASS** | Trace hash exact cross-language (IEEE-754 bit patterns hashed over LE bytes); rerun-identical asserted in PlayMode |
| Packaged smoke | **PENDING** | Needs an interactive packaged development build launched with the reference scenario |
| Performance record | **PENDING (human/hardware)** | M3 Pro FPS at 1920×1080 not measured (target ≥30 in-editor, ≥45 packaged) |
| Export manifest | **PARTIAL** | `CinematicExporter.BuildManifest` emits all required provenance fields and is unit-covered; a produced-on-disk manifest awaits an interactive export |

## Seven-criterion evaluation (Step 5.4)

| # | Criterion | Verdict | Basis |
|---|-----------|---------|-------|
| 1 | Material visual improvement | **NOT MET (pending)** | HDRP look layer (materials/volume/terrain/VFX plume/Cinemachine) not built — disk-gated (~8 GB free; HDRP shader compilation unsafe to import until freed). Structural scene is built and smoke-tested, but appearance is unproven. |
| 2 | Local interactive targets | **NOT MET (pending, hardware)** | M3 FPS not measured. |
| 3 | Deterministic trace identity | **MET** | Exact cross-language trace hash; rerun-identical verified. |
| 4 | Recoverable process lifecycle | **MET** | Forced bridge recovery leaves no orphan; supervised restart race fixed and tested. |
| 5 | Traceable overlays | **MET** | Every engineering readout is bound to the accepted `FlightTrace`; `EngineeringLayerCatalog` + review path enforce provenance. |
| 6 | Intact Rust authority | **MET** | `ascent-domain`/`ascent-sim` untouched; Tauri + React + `ascent-mcp` intact; Unity holds no canonical flight state. |
| 7 | Coherent unguided review | **NOT DONE (human)** | Requires one unguided review with a technically literate person identifying ignition, separation, apogee, active stage, orientation, and one value's source without spoken instruction. |

**Four of seven met; three pending.** Two of the pending three (interactive
targets, unguided review) are inherently human/hardware measurements; the third
(material visual improvement) is blocked on freeing disk for a safe HDRP import.

## What flips this to `expand`

All three pending criteria pass, i.e.:

1. Free disk, add HDRP 17.0.4 / Cinemachine / Timeline 1.8.12 / VFX 17.0.4 /
   Splines 2.8.4 to `visualizer/AscentUnity/Packages/manifest.json`, build the
   look layer, and confirm a material visual improvement.
2. Record M3 Pro performance at 1920×1080 (≥30 in-editor, ≥45 packaged) into
   `visualizer/AscentUnity/TestResults/performance.json`.
3. Conduct and record one successful unguided review.

Re-run `cargo xtask visualizer-test` (now with the packaged build, performance
record, and produced export manifest present) so all nine gate rows are green,
then revise this decision to `expand`.

## Scope preserved regardless

Tauri, React, and `ascent-mcp` remain the primary application surface. Unity is
additive: an optional trace-driven visualizer and offline cinematic exporter.
