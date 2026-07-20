# Unreal Visualizer Plan-Orchestrate Commands

> Warning: could not detect ECC install; defaulting to legacy form. If you use the plugin install, edit the prefixes manually.

**Plan:** `docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md`
**Lang:** `unknown` (Rust/C++ polyglot)
**ECC mode:** `legacy`
**Steps:** 6
**Scope:** `all`

## Steps overview

| # | Title | Tags | Chain |
|---|---|---|---|
| 1 | Establish the evidence-graded Black Brant IX reference | design, impl, test, review | `planner,architect,tdd-guide,code-reviewer` |
| 2 | Implement the Rust visualizer protocol and bridge | impl, test, security | `tdd-guide,code-reviewer,security-reviewer` |
| 3 | Scaffold Unreal and supervise the sidecar | impl, test, build | `tdd-guide,e2e-runner,build-error-resolver,code-reviewer` |
| 4 | Build deterministic mission playback | impl, test | `tdd-guide,e2e-runner,code-reviewer` |
| 5 | Build the engineering X-ray and cinematic tiers | impl, test, review | `tdd-guide,e2e-runner,code-reviewer` |
| 6 | Verify and record the platform decision | test, build, docs, review | `tdd-guide,e2e-runner,build-error-resolver,code-reviewer` |

---

## Step 1 — Establish the evidence-graded Black Brant IX reference

**Intent:** Reconstruct one credible NASA reference vehicle and deterministic mission before creating a visual client around it.

**Chain rationale:** Planning and architecture establish evidence boundaries; TDD implements the fixture; the generic reviewer closes the polyglot-neutral gate.

```bash
/orchestrate custom "planner,architect,tdd-guide,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-1] Design and build an evidence-graded NASA Black Brant IX reference project with authoritative, derived, and approximate fields, a two-stage vehicle, White Sands terrain provenance, and deterministic launch-to-landing trace; Acceptance: all values have provenance; fixture hashes validate; repeated traces are byte-identical; Out of scope: orbital flight, proprietary data, or historical-flight equivalence claims."
```

## Step 2 — Implement the Rust visualizer protocol and bridge

**Intent:** Create a read-only, versioned process boundary between Unreal and the authoritative Rust core.

**Chain rationale:** TDD drives framing and subprocess behavior; the generic reviewer checks the Rust-facing API; the security reviewer closes the untrusted-input and process-boundary gate.

```bash
/orchestrate custom "tdd-guide,code-reviewer,security-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-2] Implement the v1 framed-stdio protocol and supervised Rust bridge for mission catalog, run, progress, deterministic trace chunks, cancellation, errors, and shutdown; Acceptance: subprocess integration passes; reassembled trace hash matches canonical Rust bytes; malformed or oversized frames fail safely; Out of scope: sockets, MCP reuse, project mutation, or compression."
```

## Step 3 — Scaffold Unreal and supervise the sidecar

**Intent:** Establish the Unreal 5.8 C++ project and reliable bridge lifecycle before building visual features.

**Chain rationale:** TDD and end-to-end validation cover process behavior; the build resolver handles Unreal toolchain issues; the reviewer closes the implementation gate.

```bash
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-3] Create the Unreal Engine 5.8 C++ project and bridge subsystem that launches, handshakes with, reads from, restarts once, and cleanly terminates the Rust sidecar; Acceptance: Rust fixtures parse in Unreal; forced bridge failure is recoverable; Unreal exit leaves no process; Out of scope: installers, signing, remote bridges, or MCP integration."
```

## Step 4 — Build deterministic mission playback

**Intent:** Turn the verified Rust trace into deterministic Unreal transforms, events, stage transitions, controls, and cameras.

**Chain rationale:** TDD defines coordinate and clock behavior; end-to-end validation exercises playback; the reviewer checks that Unreal never becomes a physics authority.

```bash
/orchestrate custom "tdd-guide,e2e-runner,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-4] Implement deterministic Black Brant IX mission playback with ENU-to-Unreal basis conversion, trace reconstruction, interpolation, exact event seeking, stage separation, playback controls, and five cameras; Acceptance: coordinate fixtures pass; repeated seeks are identical; gaps never interpolate; render code dispatches no commands; Out of scope: authoring, Unreal physics authority, arbitrary imports, or multiplayer."
```

## Step 5 — Build the engineering X-ray and cinematic tiers

**Intent:** Deliver the realistic professional visual experience, inspection layers, clean cinematic mode, and two hardware tiers.

**Chain rationale:** TDD protects evidence bindings and UI behavior; end-to-end validation covers the complete visual workflow; the reviewer gates quality and authority boundaries.

```bash
/orchestrate custom "tdd-guide,e2e-runner,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-5] Build the realistic engineering X-ray experience, bounded White Sands world, procedural evidence-backed vehicle, Niagara plume, semantic timeline, inspection layers, two quality tiers, and cinematic export manifest; Acceptance: overlays resolve to evidence; local interactive targets pass; clean cinematic mode preserves state; Out of scope: global Cesium, complete authoring, AI proposal UI, or final production polish."
```

## Step 6 — Verify and record the platform decision

**Intent:** Run the complete technical and visual gate and decide whether Unreal should expand beyond the visualizer role.

**Chain rationale:** TDD and end-to-end validation execute the acceptance workflow; the build resolver closes platform failures; the reviewer validates the final evidence-backed decision.

```bash
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-6] Run the complete Rust, Unreal, integration, render, performance, failure-recovery, and cinematic-export gates and document the Unreal go/no-go decision; Acceptance: launch-to-landing review completes; reruns preserve trace hash; M3 performance evidence is recorded; existing v0.6 surfaces remain green; Out of scope: deleting Tauri or MCP, signing releases, or starting the next release."
```

## Batch execution

Run only after the active v0.6 work is consolidated. The chain is sequential because each step consumes the contracts and artifacts of the previous step.

```bash
/orchestrate custom "planner,architect,tdd-guide,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-1] Design and build an evidence-graded NASA Black Brant IX reference project with authoritative, derived, and approximate fields, a two-stage vehicle, White Sands terrain provenance, and deterministic launch-to-landing trace; Acceptance: all values have provenance; fixture hashes validate; repeated traces are byte-identical; Out of scope: orbital flight, proprietary data, or historical-flight equivalence claims."
/orchestrate custom "tdd-guide,code-reviewer,security-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-2] Implement the v1 framed-stdio protocol and supervised Rust bridge for mission catalog, run, progress, deterministic trace chunks, cancellation, errors, and shutdown; Acceptance: subprocess integration passes; reassembled trace hash matches canonical Rust bytes; malformed or oversized frames fail safely; Out of scope: sockets, MCP reuse, project mutation, or compression."
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-3] Create the Unreal Engine 5.8 C++ project and bridge subsystem that launches, handshakes with, reads from, restarts once, and cleanly terminates the Rust sidecar; Acceptance: Rust fixtures parse in Unreal; forced bridge failure is recoverable; Unreal exit leaves no process; Out of scope: installers, signing, remote bridges, or MCP integration."
/orchestrate custom "tdd-guide,e2e-runner,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-4] Implement deterministic Black Brant IX mission playback with ENU-to-Unreal basis conversion, trace reconstruction, interpolation, exact event seeking, stage separation, playback controls, and five cameras; Acceptance: coordinate fixtures pass; repeated seeks are identical; gaps never interpolate; render code dispatches no commands; Out of scope: authoring, Unreal physics authority, arbitrary imports, or multiplayer."
/orchestrate custom "tdd-guide,e2e-runner,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-5] Build the realistic engineering X-ray experience, bounded White Sands world, procedural evidence-backed vehicle, Niagara plume, semantic timeline, inspection layers, two quality tiers, and cinematic export manifest; Acceptance: overlays resolve to evidence; local interactive targets pass; clean cinematic mode preserves state; Out of scope: global Cesium, complete authoring, AI proposal UI, or final production polish."
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-19-unreal-visualizer-vertical-slice.md#step-6] Run the complete Rust, Unreal, integration, render, performance, failure-recovery, and cinematic-export gates and document the Unreal go/no-go decision; Acceptance: launch-to-landing review completes; reruns preserve trace hash; M3 performance evidence is recorded; existing v0.6 surfaces remain green; Out of scope: deleting Tauri or MCP, signing releases, or starting the next release."
```
