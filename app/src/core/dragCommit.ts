// Physics-in-the-loop drag lifecycle (v0.5 Step 7). A pure reducer over
// the drag of a single part parameter. The whole point is one invariant:
//
//   While dragging, the value only *previews* — the caller re-sims
//   read-only (run_simulation, which never journals) and shows the result.
//   Exactly ONE journaled command is emitted, and only on release.
//
// Modeling this as a pure reducer keeps that invariant unit-testable
// without React or IPC: feed it a start → move* → release sequence and
// assert a command appears once, on release, carrying the final value.
// The render layer stays command-free during the drag (it only reads),
// and the single release command goes through the normal dispatcher — no
// second mutation path, no per-frame journal spam.

import type { Command } from "./types";

export interface DragTarget {
  /** Part id whose parameter is being dragged. */
  id: number;
  /** serde field name on the PartKind (e.g. "span_m"). */
  param: string;
  /** Value at drag start, restored if the drag is cancelled. */
  startValue: number;
}

export interface DragState {
  target: DragTarget | null;
  /** The live preview value (what the read-only re-sim should use). */
  previewValue: number;
}

export type DragEvent =
  | { type: "start"; target: DragTarget }
  | { type: "move"; value: number }
  | { type: "release" }
  | { type: "cancel" };

export interface DragOutcome {
  state: DragState;
  /** The design value the caller should preview by read-only re-sim, or
   *  null when there is nothing to preview (idle / just released). */
  preview: { id: number; param: string; value: number } | null;
  /** Emitted on release ONLY, and only if the value actually moved. This
   *  is the single journaled mutation for the whole gesture. */
  command: Command | null;
}

export const IDLE: DragState = { target: null, previewValue: 0 };

/** Advance the drag state machine. Pure: same (state, event) → same
 *  outcome. `command` is non-null only for a `release` that changed the
 *  value. */
export function reduceDrag(state: DragState, event: DragEvent): DragOutcome {
  switch (event.type) {
    case "start":
      return {
        state: { target: event.target, previewValue: event.target.startValue },
        preview: {
          id: event.target.id,
          param: event.target.param,
          value: event.target.startValue,
        },
        command: null,
      };

    case "move": {
      if (!state.target) {
        // A move with no active drag is a no-op, not a preview.
        return { state, preview: null, command: null };
      }
      const next = { target: state.target, previewValue: event.value };
      return {
        state: next,
        preview: {
          id: state.target.id,
          param: state.target.param,
          value: event.value,
        },
        command: null,
      };
    }

    case "release": {
      if (!state.target) {
        return { state: IDLE, preview: null, command: null };
      }
      const { id, param, startValue } = state.target;
      const moved = state.previewValue !== startValue;
      const command: Command | null = moved
        ? { cmd: "set_part_param", id, param, value: state.previewValue }
        : null;
      // Back to idle; the committed value now lives in the document (once
      // the caller dispatches `command`), so there is nothing left to
      // preview.
      return { state: IDLE, preview: null, command };
    }

    case "cancel":
      // Abort: no command, snap the preview back to the start value so the
      // caller can restore the pre-drag view.
      return {
        state: IDLE,
        preview: state.target
          ? { id: state.target.id, param: state.target.param, value: state.target.startValue }
          : null,
        command: null,
      };
  }
}
