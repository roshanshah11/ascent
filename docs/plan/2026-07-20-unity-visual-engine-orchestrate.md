# Unity Visual Engine Plan-Orchestrate Result

> Warning: could not detect ECC install; defaulting to legacy form. If you use the plugin install, edit the prefixes manually.

**Plan:** `docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md`
**Lang:** `unknown` (Rust/C# polyglot; neither language exceeds 60% of tracked Rust, TypeScript, and C# sources at planning time)
**ECC mode:** `legacy`
**Steps:** 5
**Scope:** `all`

## Steps overview

| # | Title | Tags | Chain |
|---|---|---|---|
| 1 | Establish the evidence-graded Black Brant IX mission | design, impl, test, review | `planner,architect,tdd-guide,code-reviewer` |
| 2 | Implement the Rust protocol and supervised bridge | impl, test, security | `tdd-guide,code-reviewer,security-reviewer` |
| 3 | Create the Unity HDRP client and deterministic playback core | impl, test, build | `tdd-guide,e2e-runner,build-error-resolver,code-reviewer` |
| 4 | Build the narrow convincing engineering experience | impl, test, review | `tdd-guide,e2e-runner,code-reviewer` |
| 5 | Verify the slice and decide Unity's role | test, build, docs, review | `tdd-guide,e2e-runner,build-error-resolver,code-reviewer` |

---

## Step 1 — Establish the evidence-graded Black Brant IX mission

**Intent:** Establish one credible, deterministic mission before creating the Unity experience around it.

**Tags:** design, impl, test, review

**Chain rationale:** The planner and architect settle the evidence and staged-mission boundary; TDD builds the reference; the generic reviewer closes the implementation gate because the full project is polyglot.

```bash
/orchestrate custom "planner,architect,tdd-guide,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-1] Build the evidence-graded NASA Black Brant IX reference mission and deterministic two-stage trace; bind every input to authoritative, derived, or approximate evidence and label it as a reference scenario; Acceptance: source hashes and rights validate; required launch-to-landing events exist; repeat runs have byte-identical trace identity; Out of scope: orbital flight, proprietary data, or historical-flight equivalence claims."
```

## Step 2 — Implement the Rust protocol and supervised bridge

**Intent:** Create a narrow, versioned, read-only process boundary between Unity and the authoritative Rust core.

**Tags:** impl, test, security

**Chain rationale:** TDD drives framing and subprocess behavior; the generic reviewer checks public Rust contracts; the security reviewer closes the untrusted-input and child-process boundary.

```bash
/orchestrate custom "tdd-guide,code-reviewer,security-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-2] Implement protocol v1 and the read-only Rust stdio bridge for handshake, catalog, run, progress, trace chunks, events, cancellation, errors, and shutdown; Acceptance: all message fixtures pass; subprocess reconstruction matches the canonical trace hash; malformed, duplicate, out-of-order, oversized, and unauthorized input fails closed; Out of scope: sockets, MCP reuse, arbitrary files, mutation, or remote execution."
```

## Step 3 — Create the Unity HDRP client and deterministic playback core

**Intent:** Establish the pinned Unity 6 LTS HDRP project, reliable sidecar lifecycle, validated trace assembly, coordinate conversion, and playback clock.

**Tags:** impl, test, build

**Chain rationale:** TDD defines deterministic C# behavior; the end-to-end runner exercises the real Rust process; the build resolver handles Unity/Cargo integration; the generic reviewer closes the polyglot implementation gate.

```bash
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-3] Create the pinned Unity 6 LTS HDRP client, binary framed-I/O sidecar supervisor, immutable trace assembly, ENU basis conversion, and deterministic playback clock; Acceptance: Rust fixtures parse unchanged; forced bridge failure recovers once and leaves no orphan; coordinate, gap, event, and repeated-seek tests pass; Out of scope: visual polish, authoring, Unity physics authority, remote bridges, or installers."
```

## Step 4 — Build the narrow convincing engineering experience

**Intent:** Turn the accepted trace into one visually credible, professional launch-to-landing review with engineering layers and cinematic export.

**Tags:** impl, test, review

**Chain rationale:** TDD protects trace bindings and interaction contracts; the end-to-end runner proves the complete review path; the generic reviewer gates visual-system boundaries and completion.

```bash
/orchestrate custom "tdd-guide,e2e-runner,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-4] Build the realistic Black Brant IX vehicle and bounded White Sands scene, trace-driven plume and stages, semantic timeline, state/evidence inspector, engineering layers, five Cinemachine cameras, clean view, and cinematic manifest; Acceptance: launch-to-landing PlayMode flow passes; every readout resolves to evidence; interactive M3 targets pass; Out of scope: global terrain, full authoring, AI proposal UI, generalized imports, or orbital rendering."
```

## Step 5 — Verify the slice and decide Unity's role

**Intent:** Run the complete technical, performance, visual, and unguided-usability gate and make the predeclared expansion decision.

**Tags:** test, build, docs, review

**Chain rationale:** TDD and the end-to-end runner execute the acceptance workflow; the build resolver closes platform failures; the generic reviewer validates the evidence-backed decision. The documentation agent is omitted to keep the chain within four agents; the plan contains the exact required records.

```bash
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-5] Add and run the complete visualizer gate, record M3 performance and unguided review evidence, then apply the seven-criterion expand-or-contain rule; Acceptance: Rust, Unity, packaged smoke, lifecycle, rerun hash, and export gates have explicit results; platform decision follows recorded evidence; Out of scope: deleting Tauri or MCP, adding authoring, signing, cloud rendering, or starting the next phase."
```

## Batch execution

Run these in order after v0.6 is consolidated. Each step consumes contracts and artifacts from the previous step.

```bash
/orchestrate custom "planner,architect,tdd-guide,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-1] Build the evidence-graded NASA Black Brant IX reference mission and deterministic two-stage trace; bind every input to authoritative, derived, or approximate evidence and label it as a reference scenario; Acceptance: source hashes and rights validate; required launch-to-landing events exist; repeat runs have byte-identical trace identity; Out of scope: orbital flight, proprietary data, or historical-flight equivalence claims."
/orchestrate custom "tdd-guide,code-reviewer,security-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-2] Implement protocol v1 and the read-only Rust stdio bridge for handshake, catalog, run, progress, trace chunks, events, cancellation, errors, and shutdown; Acceptance: all message fixtures pass; subprocess reconstruction matches the canonical trace hash; malformed, duplicate, out-of-order, oversized, and unauthorized input fails closed; Out of scope: sockets, MCP reuse, arbitrary files, mutation, or remote execution."
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-3] Create the pinned Unity 6 LTS HDRP client, binary framed-I/O sidecar supervisor, immutable trace assembly, ENU basis conversion, and deterministic playback clock; Acceptance: Rust fixtures parse unchanged; forced bridge failure recovers once and leaves no orphan; coordinate, gap, event, and repeated-seek tests pass; Out of scope: visual polish, authoring, Unity physics authority, remote bridges, or installers."
/orchestrate custom "tdd-guide,e2e-runner,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-4] Build the realistic Black Brant IX vehicle and bounded White Sands scene, trace-driven plume and stages, semantic timeline, state/evidence inspector, engineering layers, five Cinemachine cameras, clean view, and cinematic manifest; Acceptance: launch-to-landing PlayMode flow passes; every readout resolves to evidence; interactive M3 targets pass; Out of scope: global terrain, full authoring, AI proposal UI, generalized imports, or orbital rendering."
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-20-unity-visual-engine-vertical-slice.md#step-5] Add and run the complete visualizer gate, record M3 performance and unguided review evidence, then apply the seven-criterion expand-or-contain rule; Acceptance: Rust, Unity, packaged smoke, lifecycle, rerun hash, and export gates have explicit results; platform decision follows recorded evidence; Out of scope: deleting Tauri or MCP, adding authoring, signing, cloud rendering, or starting the next phase."
```
