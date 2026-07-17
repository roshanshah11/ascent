// Command-stack undo/redo (v0.2 Step 2). Pure module — no IPC, no React.
// Every design edit is a Command with apply/invert; past/future stacks give
// undo/redo for free. The run-state machine stays the authority on staleness:
// callers dispatch EDIT on undo/redo too (an undone design is a dirty design).
import type { Design } from "./types";

export interface Command {
  label: string;
  apply(d: Design): Design;
  invert(d: Design): Design;
}

export interface CommandHistory {
  past: Command[];
  future: Command[];
}

export const emptyHistory: CommandHistory = { past: [], future: [] };

/** Cap history depth so a marathon session can't grow memory unboundedly. */
const MAX_HISTORY = 200;

/** A field edit as a command. Stores both values so invert needs no diffing. */
export function setField<K extends keyof Design>(
  key: K,
  oldValue: Design[K],
  newValue: Design[K],
): Command {
  return {
    label: `edit ${String(key)}`,
    apply: (d) => ({ ...d, [key]: newValue }),
    invert: (d) => ({ ...d, [key]: oldValue }),
  };
}

/** Wholesale design replacement (motor swap, chute panel edits). */
export function replaceDesign(oldDesign: Design, newDesign: Design): Command {
  return {
    label: "edit design",
    apply: () => newDesign,
    invert: () => oldDesign,
  };
}

export function pushCommand(
  h: CommandHistory,
  cmd: Command,
  d: Design,
): { history: CommandHistory; design: Design } {
  const past = [...h.past, cmd].slice(-MAX_HISTORY);
  // A new edit invalidates the redo branch — standard editor semantics.
  return { history: { past, future: [] }, design: cmd.apply(d) };
}

export function canUndo(h: CommandHistory): boolean {
  return h.past.length > 0;
}

export function canRedo(h: CommandHistory): boolean {
  return h.future.length > 0;
}

export function undo(
  h: CommandHistory,
  d: Design,
): { history: CommandHistory; design: Design } {
  if (!canUndo(h)) return { history: h, design: d };
  const cmd = h.past[h.past.length - 1];
  return {
    history: { past: h.past.slice(0, -1), future: [cmd, ...h.future] },
    design: cmd.invert(d),
  };
}

export function redo(
  h: CommandHistory,
  d: Design,
): { history: CommandHistory; design: Design } {
  if (!canRedo(h)) return { history: h, design: d };
  const cmd = h.future[0];
  return {
    history: { past: [...h.past, cmd], future: h.future.slice(1) },
    design: cmd.apply(d),
  };
}
