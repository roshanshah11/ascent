# Unity Visual Engine Orchestration

**Plan:** `docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md`
**Mode:** legacy ECC, sequential steps, one specialist per step
**Execution policy:** implement first, then run focused verification. No test-first cycle.

Run each command only after the preceding step passes its acceptance criteria.

## Step 1 — Evidence-graded Black Brant IX mission

```bash
/orchestrate custom "architect" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-1] Implement the evidence-graded NASA Black Brant IX reference mission and deterministic two-stage trace, then run focused evidence and repeatability checks; Acceptance: source rights and hashes validate; required launch-to-landing events exist; repeated traces have identical identity; Out of scope: orbital flight, proprietary data, or historical-flight equivalence claims."
```

## Step 2 — Rust protocol and supervised bridge

```bash
/orchestrate custom "rust-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-2] Implement protocol v1 and the read-only Rust stdio bridge, then verify fixtures, reconstruction, cancellation, errors, and shutdown; Acceptance: fixtures pass; reconstructed trace hash matches; malformed, duplicate, out-of-order, oversized, and unauthorized input fails closed; Out of scope: sockets, MCP reuse, mutation, arbitrary files, or remote execution."
```

## Step 3 — Unity client and deterministic playback

```bash
/orchestrate custom "build-error-resolver" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-3] Build the pinned Unity 6 LTS HDRP client, bridge supervisor, immutable trace assembly, ENU conversion, and deterministic playback, then run focused integration checks; Acceptance: Rust fixtures parse unchanged; one forced bridge recovery leaves no orphan; coordinate, gap, event, and repeated-seek checks pass; Out of scope: polish, authoring, Unity physics authority, remote bridges, or installers."
```

## Step 4 — Convincing engineering experience

```bash
/orchestrate custom "e2e-runner" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-4] Build the Black Brant IX vehicle, White Sands scene, trace-driven effects, timeline, evidence inspector, engineering layers, five cameras, clean view, and cinematic manifest, then verify the full review path; Acceptance: launch-to-landing flow passes; every readout is traceable; interactive M3 targets pass; Out of scope: global terrain, full authoring, AI proposal UI, generalized imports, or orbital rendering."
```

## Step 5 — Platform verification and decision

```bash
/orchestrate custom "e2e-runner" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-5] Run the complete visualizer gate, record M3 performance and unguided-review evidence, and apply the seven-criterion expand-or-contain rule; Acceptance: Rust, Unity, packaged smoke, lifecycle, rerun hash, and export gates have explicit results; the decision follows recorded evidence; Out of scope: deleting Tauri or MCP, authoring, signing, cloud rendering, or beginning the next phase."
```
