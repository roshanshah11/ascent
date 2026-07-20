import { describe, expect, it } from "vitest";
import { IDLE, reduceDrag, type DragState } from "./dragCommit";

const target = { id: 3, param: "span_m", startValue: 0.03 };

/** Run a whole event sequence, collecting every emitted command. */
function play(events: Parameters<typeof reduceDrag>[1][]) {
  let state: DragState = IDLE;
  const commands = [];
  const previews = [];
  for (const e of events) {
    const out = reduceDrag(state, e);
    state = out.state;
    if (out.command) commands.push(out.command);
    if (out.preview) previews.push(out.preview);
  }
  return { state, commands, previews };
}

describe("reduceDrag", () => {
  it("emits exactly one command, on release, with the final value", () => {
    const { commands } = play([
      { type: "start", target },
      { type: "move", value: 0.035 },
      { type: "move", value: 0.04 },
      { type: "move", value: 0.045 },
      { type: "release" },
    ]);
    expect(commands).toHaveLength(1);
    expect(commands[0]).toEqual({
      cmd: "set_part_param",
      id: 3,
      param: "span_m",
      value: 0.045,
    });
  });

  it("never emits a command during the drag (moves preview only)", () => {
    const { commands, previews } = play([
      { type: "start", target },
      { type: "move", value: 0.035 },
      { type: "move", value: 0.04 },
    ]);
    expect(commands).toHaveLength(0);
    // start + two moves each yield a preview.
    expect(previews.map((p) => p.value)).toEqual([0.03, 0.035, 0.04]);
  });

  it("emits no command when released at the start value (no real change)", () => {
    const { commands } = play([
      { type: "start", target },
      { type: "move", value: 0.05 },
      { type: "move", value: 0.03 }, // dragged back to start
      { type: "release" },
    ]);
    expect(commands).toHaveLength(0);
  });

  it("cancel emits no command and previews the start value for restore", () => {
    const out = play([
      { type: "start", target },
      { type: "move", value: 0.09 },
      { type: "cancel" },
    ]);
    expect(out.commands).toHaveLength(0);
    expect(out.state).toEqual(IDLE);
    expect(out.previews[out.previews.length - 1]).toEqual({
      id: 3,
      param: "span_m",
      value: 0.03,
    });
  });

  it("ignores moves and releases with no active drag", () => {
    expect(reduceDrag(IDLE, { type: "move", value: 1 })).toEqual({
      state: IDLE,
      preview: null,
      command: null,
    });
    expect(reduceDrag(IDLE, { type: "release" })).toEqual({
      state: IDLE,
      preview: null,
      command: null,
    });
  });

  it("is pure — same input yields same outcome", () => {
    const a = reduceDrag({ target, previewValue: 0.04 }, { type: "release" });
    const b = reduceDrag({ target, previewValue: 0.04 }, { type: "release" });
    expect(a).toEqual(b);
  });
});
