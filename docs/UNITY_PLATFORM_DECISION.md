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
| Packaged smoke | **PASS** | `PlayerBuilder.BuildFromBatch` builds a Development `StandaloneOSX` player (317 MB, `BuildResult.Succeeded`, 0 errors). Launched with `-ascent-benchmark` it creates a real Metal swapchain at 1920×1080, renders the reference scene, logs `ASCENT_PACKAGED_SMOKE_OK`, and quits cleanly (exit 0). |
| Performance record | **PASS (in-editor + packaged)** | **In-editor** — windowed PlayMode render on **Apple M3 Pro / Metal**: median **189–220 FPS** (5.3 ms/frame), ≥30 target met ~6×. **Packaged** — standalone player: median **60.07 FPS** (16.65 ms/frame, p95 16.96 ms, VSync-locked to the 60 Hz display), ≥45 target met. Both recorded (`TestResults/performance.json`, `PackagedBenchmark` output; git-ignored). Batchmode measurements are explicitly rejected as non-representative (no swapchain → ~5000 FPS artifact, `representative_render:false`). |
| Export manifest | **PASS** | `VerticalSliceTests` runs the real mission through the bridge and writes `visualizer/AscentUnity/Exports/manifest.json` (git-ignored) with genuine provenance: `trace_sha256` (real accepted-trace hash), `protocol_version`, `mission_id`, `camera_id`, window, quality, and evidence caveats — all required fields present. |

## Seven-criterion evaluation (Step 5.4)

| # | Criterion | Verdict | Basis |
|---|-----------|---------|-------|
| 1 | Material visual improvement | **MET** | HDRP imported and the scene renders through it (commit `2544f68`): HDRP/Lit metallic airframe + gypsum ground, global volume. Trace-driven exhaust plume (`2307abd`), five Cinemachine review cameras (`e11b5f7`), a live UIDocument review HUD (`7a491f8`), and a White Sands gypsum-dune terrain (`cadc9b3`). The look layer is real and rendered, a clear step up from the untextured structural scaffold. (Subjective criterion; basis is the delivered, rendering visual layer.) |
| 2 | Local interactive targets | **MET** | Windowed in-editor M3 Pro measurement: median **189–220 FPS** (≥30 target, ~6×). Standalone packaged player: median **60.07 FPS** (≥45 target). Both halves pass on the Apple M3 Pro. |
| 3 | Deterministic trace identity | **MET** | Exact cross-language trace hash; rerun-identical verified. |
| 4 | Recoverable process lifecycle | **MET** | Forced bridge recovery leaves no orphan; supervised restart race fixed and tested. |
| 5 | Traceable overlays | **MET** | Every engineering readout is bound to the accepted `FlightTrace`; `EngineeringLayerCatalog` + review path enforce provenance. |
| 6 | Intact Rust authority | **MET** | `ascent-domain`/`ascent-sim` untouched; Tauri + React + `ascent-mcp` intact; Unity holds no canonical flight state. |
| 7 | Coherent unguided review | **NOT DONE (human)** | Requires one unguided review with a technically literate person identifying ignition, separation, apogee, active stage, orientation, and one value's source without spoken instruction. Not performed — cannot be produced in code. |

**Six of seven met; one not done.** All nine machine-verifiable gates above are
green, and criterion 2 now passes on both the in-editor and packaged halves. The
single remaining criterion (#7, coherent unguided review) is inherently a human
measurement and has not been run. The `expand` rule requires all seven, so the
decision remains `contain` — held there by **exactly one unperformed human gate**.

## What flips this to `expand`

**Conduct and record one successful unguided review** (criterion 7) — now the
sole blocker. A technically literate person, with no spoken guidance, opens the
build and correctly identifies ignition, stage separation, apogee, the active
stage, vehicle orientation, and the source of one displayed value. Record the
outcome here, then revise this decision to `expand`.

Every other gate and criterion is satisfied with recorded evidence: the Rust
authority chain, the deterministic trace identity, the recoverable bridge
lifecycle, the traceable overlays, the built HDRP look layer, the in-editor and
packaged M3 performance, the packaged smoke launch, and the provenance-complete
export manifest.

## Scope preserved regardless

Tauri, React, and `ascent-mcp` remain the primary application surface. Unity is
additive: an optional trace-driven visualizer and offline cinematic exporter.
