import { describe, expect, it } from "vitest";
import {
  canRedo,
  canUndo,
  emptyHistory,
  pushCommand,
  redo,
  replaceDesign,
  setField,
  undo,
  type CommandHistory,
} from "./commands";
import type { Design } from "./types";

const base: Design = {
  name: "Estes Alpha III",
  dry_mass_g: 34,
  diameter_mm: 25,
  cd: 0.6,
  chute: { enabled: true, diameter_cm: 30, cd: 0.75 },
  motor_designation: "C6",
  rail_length_m: 0.9,
};

describe("command stack", () => {
  it("apply then undo restores a byte-equal design", () => {
    const cmd = setField("cd", base.cd, 0.72);
    const applied = pushCommand(emptyHistory, cmd, base);
    expect(applied.design.cd).toBe(0.72);

    const undone = undo(applied.history, applied.design);
    expect(JSON.stringify(undone.design)).toBe(JSON.stringify(base));
    expect(canUndo(undone.history)).toBe(false);
    expect(canRedo(undone.history)).toBe(true);
  });

  it("redo reapplies and a new edit clears the redo branch", () => {
    const s1 = pushCommand(emptyHistory, setField("cd", 0.6, 0.7), base);
    const u = undo(s1.history, s1.design);
    const r = redo(u.history, u.design);
    expect(r.design.cd).toBe(0.7);

    // New edit after undo: future must be discarded.
    const u2 = undo(r.history, r.design);
    const branched = pushCommand(
      u2.history,
      setField("dry_mass_g", 34, 40),
      u2.design,
    );
    expect(canRedo(branched.history)).toBe(false);
    expect(branched.design.dry_mass_g).toBe(40);
    expect(branched.design.cd).toBe(0.6);
  });

  it("replaceDesign round-trips nested chute edits", () => {
    const edited: Design = {
      ...base,
      chute: { ...base.chute, diameter_cm: 45 },
    };
    const s = pushCommand(emptyHistory, replaceDesign(base, edited), base);
    expect(s.design.chute.diameter_cm).toBe(45);
    const u = undo(s.history, s.design);
    expect(JSON.stringify(u.design)).toBe(JSON.stringify(base));
  });

  it("survives a 50-deep history walked all the way back and forward", () => {
    let history: CommandHistory = emptyHistory;
    let design = base;
    for (let i = 1; i <= 50; i++) {
      const next = pushCommand(
        history,
        setField("dry_mass_g", design.dry_mass_g, 34 + i),
        design,
      );
      history = next.history;
      design = next.design;
    }
    expect(design.dry_mass_g).toBe(84);

    for (let i = 0; i < 50; i++) {
      const u = undo(history, design);
      history = u.history;
      design = u.design;
    }
    expect(design.dry_mass_g).toBe(34);
    expect(canUndo(history)).toBe(false);

    for (let i = 0; i < 50; i++) {
      const r = redo(history, design);
      history = r.history;
      design = r.design;
    }
    expect(design.dry_mass_g).toBe(84);
    expect(canRedo(history)).toBe(false);
  });

  it("undo/redo on empty stacks is a no-op, not a crash", () => {
    const u = undo(emptyHistory, base);
    expect(u.design).toBe(base);
    const r = redo(emptyHistory, base);
    expect(r.design).toBe(base);
  });
});
