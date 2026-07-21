# Unity Platform Decision — Black Brant IX Vertical Slice

**Decision: `contain`** (updated 2026-07-21, branch `feat/unity-visual-engine`)

Unity is retained as an *optional* visualizer and cinematic exporter. It does
not become the primary platform. This decision follows the seven predeclared
criteria in `docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md`
(Step 5.4): `expand` requires **all seven** to pass; otherwise `contain`.

As of 2026-07-21 the gap has narrowed to effectively **one criterion**: the
coherent unguided review (#7), which is inherently a human measurement and has
not been performed. The visual look layer is now built (criterion 1) and the
in-editor M3 performance target is measured and passing (criterion 2, editor
half). The rule requires all seven, so a single unperformed human gate keeps
the decision at `contain`. This is a recorded-evidence decision, not an
enthusiasm call — and the honest recording is that no unguided review has run.

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
| Unity EditMode | **PASS** (last: commit `cadc9b3`) | 53 tests: decoder, coordinate basis, trace assembly, playback, layer binding, scene structure (incl. HDRP terrain + live UIDocument HUD), plume, camera rig, review-HUD binding |
| Unity PlayMode | **PASS** | 5 core tests: full review path vs real bridge (play / seek-every-event / 5 cameras / clean view / rerun-identical hash / export) + cancel + bridge lifecycle; plus 1 performance test |
| Rerun hash identity | **PASS** | Trace hash exact cross-language (IEEE-754 bit patterns hashed over LE bytes); rerun-identical asserted in PlayMode |
| Packaged smoke | **PENDING** | Needs an interactive packaged development build launched with the reference scenario |
| Performance record | **PASS (in-editor); packaged PENDING** | Windowed PlayMode render of `BlackBrantIX` on **Apple M3 Pro / Metal**: median **189.2 FPS** (5.29 ms/frame, p95 9.06 ms) over 300 frames, intended 1920×1080 — **≥30 in-editor target met with ~6× margin**. Recorded to `visualizer/AscentUnity/TestResults/performance.json` (git-ignored artifact) by `PerformanceTests`. The ≥45 **packaged** target still needs a standalone player build; a batchmode measurement is explicitly non-representative (no swapchain → ~5000 FPS artifact, flagged `representative_render:false`). |
| Export manifest | **PARTIAL** | `CinematicExporter.BuildManifest` emits all required provenance fields and is unit-covered; a produced-on-disk manifest awaits an interactive export |

## Seven-criterion evaluation (Step 5.4)

| # | Criterion | Verdict | Basis |
|---|-----------|---------|-------|
| 1 | Material visual improvement | **MET** | HDRP imported and the scene renders through it (commit `2544f68`): HDRP/Lit metallic airframe + gypsum ground, global volume. Trace-driven exhaust plume (`2307abd`), five Cinemachine review cameras (`e11b5f7`), a live UIDocument review HUD (`7a491f8`), and a White Sands gypsum-dune terrain (`cadc9b3`). The look layer is real and rendered, a clear step up from the untextured structural scaffold. (Subjective criterion; basis is the delivered, rendering visual layer.) |
| 2 | Local interactive targets | **MET in-editor; packaged half pending** | Windowed M3 Pro measurement: median **189.2 FPS** at intended 1920×1080, ~6× the ≥30 in-editor target. The ≥45 **packaged** target awaits a standalone player build. |
| 3 | Deterministic trace identity | **MET** | Exact cross-language trace hash; rerun-identical verified. |
| 4 | Recoverable process lifecycle | **MET** | Forced bridge recovery leaves no orphan; supervised restart race fixed and tested. |
| 5 | Traceable overlays | **MET** | Every engineering readout is bound to the accepted `FlightTrace`; `EngineeringLayerCatalog` + review path enforce provenance. |
| 6 | Intact Rust authority | **MET** | `ascent-domain`/`ascent-sim` untouched; Tauri + React + `ascent-mcp` intact; Unity holds no canonical flight state. |
| 7 | Coherent unguided review | **NOT DONE (human)** | Requires one unguided review with a technically literate person identifying ignition, separation, apogee, active stage, orientation, and one value's source without spoken instruction. Not performed — cannot be produced in code. |

**Six of seven met; one not done.** The single remaining criterion (#7,
unguided review) is inherently a human measurement and has not been run. The
`expand` rule requires all seven, so the decision remains `contain` — held there
by exactly one unperformed human gate, plus the ≥45-FPS *packaged* sub-target of
criterion 2 (editor target already passes).

## What flips this to `expand`

1. **Conduct and record one successful unguided review** (criterion 7) — the
   sole blocking human gate.
2. Build a standalone player and record its 1920×1080 FPS to close the ≥45
   *packaged* sub-target of criterion 2 (the in-editor target already passes at
   189 FPS).
3. Optionally produce the packaged-smoke build and an on-disk export manifest so
   the two remaining `cargo xtask visualizer-test` rows also go green.

When criterion 7 passes (and the packaged FPS is recorded), revise this decision
to `expand`.

## Scope preserved regardless

Tauri, React, and `ascent-mcp` remain the primary application surface. Unity is
additive: an optional trace-driven visualizer and offline cinematic exporter.
