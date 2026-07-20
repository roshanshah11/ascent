# Physics-in-the-loop Fin Drag Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a tree-backed fin-span slider with debounced read-only physics preview and one journaled command on release.

**Architecture:** Rust owns the preview calculation by cloning the document and running the existing tree-derived marker and planar-dispersion paths. React owns gesture timing and renders transient preview results; only the existing dispatcher mutates canonical state.

**Tech Stack:** Rust/Tauri v2, React 18, TypeScript, Vitest.

## Global Constraints

- Preview must never mutate the document, journal, undo stack, or studies.
- Release is the only mutation point and emits at most one `set_part_param`.
- No new dependency and no warning suppression.
- Preserve unrelated shared-worktree edits.

---

### Task 1: Read-only Rust preview seam

**Files:**
- Modify: `crates/ascent-app/src/lib.rs`
- Test: `crates/ascent-app/src/lib.rs`

**Interfaces:**
- Consumes: `Document: Clone`, `Command::SetPartParam`, `markers::vehicle_markers`, `dispersion_ipc::run`.
- Produces: `preview_part_param(state, id, param, value) -> Result<PartPreview, String>` where `PartPreview` contains `markers` and `apogee_m`.

- [ ] Write a test that snapshots document state/journal, previews fin span `0.05`, asserts changed stability/apogee, and asserts the original snapshot/journal are unchanged.
- [ ] Run `rtk cargo test -p ascent-app preview_part_param` and verify it fails because the preview helper is absent.
- [ ] Implement a pure helper that clones `Document`, dispatches only into the clone, derives markers, runs one deterministic zero-variation planar flight, and expose it as a registered Tauri command.
- [ ] Run `rtk cargo test -p ascent-app preview_part_param` and verify it passes.

### Task 2: DragSlider component

**Files:**
- Create: `app/src/components/DragSlider.tsx`
- Create: `app/src/components/DragSlider.test.tsx`
- Modify: `app/src/ipc.ts`
- Modify: `app/src/core/types.ts`

**Interfaces:**
- Consumes: `reduceDrag`, `previewPartParam(id, param, value)`.
- Produces: `DragSlider({target, min, max, step, onPreview, onCommit})`.

- [ ] Write Vitest cases proving moves debounce to one preview, release commits once, no-op release commits zero times, and Escape cancels.
- [ ] Run `rtk npm --prefix app test -- DragSlider.test.tsx` and verify failure because the component is absent.
- [ ] Implement the minimal controlled range component, IPC DTO/binding, timer cleanup, and reducer event wiring.
- [ ] Run the focused test and verify all cases pass.

### Task 3: Tree part inspector and App integration

**Files:**
- Create: `app/src/components/PartInspector.tsx`
- Create: `app/src/components/PartInspector.test.tsx`
- Modify: `app/src/App.tsx`
- Modify: `app/src/styles/theme.css`

**Interfaces:**
- Consumes: `DocumentState.vehicle`, `DragSlider`, `dispatchCommand`, `PartPreview`.
- Produces: a Fin Set panel showing committed span plus transient stability and apogee preview.

- [ ] Write a component test that finds nested fin sets, renders their span, forwards previews, and emits the exact release command.
- [ ] Run the focused Vitest test and verify it fails because `PartInspector` is absent.
- [ ] Implement recursive fin discovery, preview labels, error state, App dispatch/marker refresh, and scoped styling.
- [ ] Run focused tests, `rtk cargo xtask test`, `rtk npm --prefix app run build`, and `rtk git diff --check`.

