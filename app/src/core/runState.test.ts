import { describe, expect, it } from "vitest";
import {
  initialRunStatus,
  reduceRunStatus,
  type RunAction,
  type RunStatus,
} from "./runState";

function replay(actions: RunAction[], from: RunStatus = initialRunStatus) {
  return actions.reduce(reduceRunStatus, from);
}

describe("run-state machine", () => {
  it("starts dirty (no result exists yet)", () => {
    expect(initialRunStatus.state).toBe("dirty");
  });

  it("dirty -> running -> current on a clean run", () => {
    const s = replay([{ type: "RUN_START" }, { type: "RUN_SUCCESS" }]);
    expect(s.state).toBe("current");
  });

  it("editing a current design makes it dirty", () => {
    const s = replay([
      { type: "RUN_START" },
      { type: "RUN_SUCCESS" },
      { type: "EDIT" },
    ]);
    expect(s.state).toBe("dirty");
  });

  it("editing during a run lands the result as stale, not current", () => {
    const s = replay([
      { type: "RUN_START" },
      { type: "EDIT" },
      { type: "RUN_SUCCESS" },
    ]);
    expect(s.state).toBe("stale");
  });

  it("stale design can re-run to current", () => {
    const s = replay([
      { type: "RUN_START" },
      { type: "EDIT" },
      { type: "RUN_SUCCESS" }, // stale
      { type: "RUN_START" },
      { type: "RUN_SUCCESS" },
    ]);
    expect(s.state).toBe("current");
  });

  it("failed run returns to dirty", () => {
    const s = replay([{ type: "RUN_START" }, { type: "RUN_FAIL" }]);
    expect(s.state).toBe("dirty");
  });

  it("edit during run followed by failure is dirty (no stale ghost)", () => {
    const s = replay([
      { type: "RUN_START" },
      { type: "EDIT" },
      { type: "RUN_FAIL" },
    ]);
    expect(s.state).toBe("dirty");
    expect(s.editedWhileRunning).toBe(false);
  });

  it("completions outside running are ignored", () => {
    const s = replay([{ type: "RUN_SUCCESS" }]);
    expect(s.state).toBe("dirty");
  });

  it("multiple edits while dirty stay dirty", () => {
    const s = replay([{ type: "EDIT" }, { type: "EDIT" }]);
    expect(s.state).toBe("dirty");
  });
});

describe("demo reset", () => {
  it("reset from any state returns to the initial dirty state", () => {
    for (const prefix of [
      [],
      [{ type: "RUN_START" }],
      [{ type: "RUN_START" }, { type: "RUN_SUCCESS" }],
      [{ type: "RUN_START" }, { type: "EDIT" }, { type: "RUN_SUCCESS" }],
    ] as RunAction[][]) {
      const s = replay([...prefix, { type: "RESET" }]);
      expect(s).toEqual(initialRunStatus);
    }
  });

  it("a run completing after reset is ignored as a ghost", () => {
    const s = replay([
      { type: "RUN_START" },
      { type: "RESET" },
      { type: "RUN_SUCCESS" },
    ]);
    expect(s.state).toBe("dirty");
  });
});
