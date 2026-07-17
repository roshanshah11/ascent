// Design/run state machine (Day 5 acceptance: transitions correct).
//
// The four user-visible states:
//   current — a result exists and matches the design on screen
//   dirty   — the design changed since the last result (or no result yet)
//   running — a simulation is in flight for the design as of run start
//   stale   — a result exists, but the design changed after the run started
//
// Pure reducer, no UI or IPC — presentation can be redesigned around it.

export type RunState = "current" | "dirty" | "running" | "stale";

export interface RunStatus {
  state: RunState;
  /// Design changed while a run was in flight; its result will land stale.
  editedWhileRunning: boolean;
}

export type RunAction =
  | { type: "EDIT" }
  | { type: "RUN_START" }
  | { type: "RUN_SUCCESS" }
  | { type: "RUN_FAIL" }
  | { type: "RESET" };

export const initialRunStatus: RunStatus = {
  state: "dirty", // no result yet: the design is unproven
  editedWhileRunning: false,
};

export function reduceRunStatus(s: RunStatus, a: RunAction): RunStatus {
  switch (a.type) {
    case "EDIT":
      if (s.state === "running") return { ...s, editedWhileRunning: true };
      if (s.state === "current" || s.state === "stale")
        return { state: "dirty", editedWhileRunning: false };
      return s; // dirty stays dirty
    case "RUN_START":
      // Running from any non-running state is allowed (re-runs included).
      if (s.state === "running") return s;
      return { state: "running", editedWhileRunning: false };
    case "RUN_SUCCESS":
      if (s.state !== "running") return s; // ignore ghost completions
      return s.editedWhileRunning
        ? { state: "stale", editedWhileRunning: false }
        : { state: "current", editedWhileRunning: false };
    case "RUN_FAIL":
      if (s.state !== "running") return s;
      return { state: "dirty", editedWhileRunning: false };
    case "RESET":
      // Demo reset: back to the pristine no-result state. A run still in
      // flight will complete as a ghost and be ignored (state !== running).
      return { ...initialRunStatus };
  }
}
