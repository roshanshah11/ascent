# Physics-in-the-loop fin drag

## Goal

Expose the vehicle tree's fin span as a direct slider that previews physical consequences without mutating the document, then records exactly one command when the gesture ends.

## Design

- `preview_part_param` clones the current Rust document, applies one `set_part_param` command to the clone, and returns tree-derived CP/CG/stability markers plus a deterministic one-flight planar apogee. The live document, journal, undo stack, and studies remain untouched.
- `DragSlider` owns pointer/keyboard gesture state through the existing `reduceDrag` reducer. Moves are debounced before previewing. Release emits the reducer's single command; Escape restores the starting preview and emits no command.
- `PartInspector` traverses the current vehicle tree and exposes fin-set span controls. The committed tree remains the rendered source of truth; preview markers and apogee are labeled temporary and disappear after release/cancel.
- `App` dispatches the release command through the existing command spine, marks results stale, refreshes committed markers, and never journals preview traffic.

## Acceptance

- Repeated moves create zero journaled commands; a changed release creates exactly one `set_part_param`.
- Cancel and no-op release create zero commands.
- Preview changes stability/apogee for a changed fin span and leaves the live document byte-identical.
- The UI debounces preview calls and exposes preview status/errors without blocking release.
- Existing flat design controls and all current tests remain intact.

