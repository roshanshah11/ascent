/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Design, SpreadResult } from "./core/types";

const ipc = vi.hoisted(() => ({
  fetchReferenceDesign: vi.fn(),
  fetchMotors: vi.fn(),
  runSimulation: vi.fn(),
  runSpread: vi.fn(),
  autosaveProject: vi.fn(() => Promise.resolve()),
  checkRecovery: vi.fn(() => Promise.resolve(null)),
  discardRecovery: vi.fn(() => Promise.resolve()),
  getDocument: vi.fn(),
  dispatchCommand: vi.fn(),
  undoDocument: vi.fn(),
  redoDocument: vi.fn(),
}));

vi.mock("./ipc", () => ipc);
vi.mock("./components/EvidenceDrawer", () => ({ default: () => null }));
vi.mock("./components/JobsPanel", () => ({ default: () => null }));
vi.mock("./components/FlightMode", () => ({ default: () => null }));
vi.mock("./components/MotorSelector", () => ({ default: () => null }));
vi.mock("./components/ReviewPanel", () => ({ default: () => null }));
vi.mock("./components/Viewport", () => ({ default: () => null }));
vi.mock("./components/Inspector", () => ({
  default: ({ design, onChange }: { design: Design; onChange: (next: Design) => void }) => (
    <button onClick={() => onChange({ ...design, cd: design.cd + 0.01 })}>
      Edit design
    </button>
  ),
}));

import App from "./App";

const reference: Design = {
  name: "Estes Alpha III",
  dry_mass_g: 34,
  diameter_mm: 25,
  cd: 0.6,
  chute: { enabled: true, diameter_cm: 30, cd: 0.75 },
  motor_designation: "C6",
  rail_length_m: 0.9,
};

const spread: SpreadResult = {
  native: {
    apogee_m: 361,
    apogee_time_s: 7.8,
    burnout_time_s: 1.86,
    burnout_velocity_ms: 112,
    max_velocity_ms: 113,
    rail_exit_velocity_ms: 17,
    landing_time_s: 103,
    landing_velocity_ms: -3.7,
    input_hash: "native",
  },
  rocketpy: {
    available: true,
    engine_id: "rocketpy-bridge",
    engine_version: "rocketpy-1.12.1",
    summary: {
      apogee_m: 360,
      apogee_time_s: 7.8,
      burnout_time_s: 1.86,
      burnout_velocity_ms: 112,
      max_velocity_ms: 113,
      rail_exit_velocity_ms: 17,
      landing_time_s: 103,
      landing_velocity_ms: -3.7,
      input_hash: "rocketpy",
    },
    reason: null,
  },
  apogee_spread_m: 1,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("engine comparison UI", () => {
  let container: HTMLDivElement;
  let root: Root;

  const button = (label: string) => {
    const found = Array.from(container.querySelectorAll("button")).find(
      (candidate) => candidate.textContent === label,
    );
    if (!found) throw new Error(`missing ${label} button`);
    return found;
  };

  const click = async (label: string) => {
    await act(async () => {
      button(label).dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
  };

  const expectComparison = (visible: boolean) => {
    expect(container.textContent?.includes("Cross-validation")).toBe(visible);
  };

  beforeEach(async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    ipc.fetchReferenceDesign.mockResolvedValue(reference);
    ipc.fetchMotors.mockResolvedValue([]);
    ipc.runSimulation.mockReset();
    ipc.runSpread.mockReset();

    // Stateful fake of the Rust document store: enough command semantics
    // for the UI flows under test (scalar edits, motor, coarse replace).
    let designNow: Design = reference;
    let past: Design[] = [];
    let future: Design[] = [];
    const state = () => ({
      vehicle: { name: designNow.name, parts: [] },
      design: designNow,
      studies: [],
      can_undo: past.length > 0,
      can_redo: future.length > 0,
    });
    ipc.getDocument.mockImplementation(async () => state());
    ipc.dispatchCommand.mockImplementation(
      async (cmd: { cmd: string } & Record<string, unknown>) => {
        past.push(designNow);
        future = [];
        if (cmd.cmd === "set_sim_param") {
          designNow = { ...designNow, [cmd.param as string]: cmd.value } as Design;
        } else if (cmd.cmd === "select_motor") {
          designNow = { ...designNow, motor_designation: cmd.designation as string };
        } else if (cmd.cmd === "set_design") {
          designNow = cmd.design as Design;
        }
        return state();
      },
    );
    ipc.undoDocument.mockImplementation(async () => {
      const prev = past.pop();
      if (prev) {
        future.push(designNow);
        designNow = prev;
      }
      return state();
    });
    ipc.redoDocument.mockImplementation(async () => {
      const next = future.pop();
      if (next) {
        past.push(designNow);
        designNow = next;
      }
      return state();
    });
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
    await act(async () => {
      root.render(<App />);
      await Promise.resolve();
    });
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
    vi.clearAllMocks();
  });

  it("discards a comparison that completes after an edit", async () => {
    const pending = deferred<SpreadResult>();
    ipc.runSpread.mockReturnValueOnce(pending.promise);

    await click("Compare engines");
    await click("Edit design");
    await act(async () => {
      pending.resolve(spread);
      await Promise.resolve();
    });

    expectComparison(false);
  });

  it("clears a completed comparison for undo, redo, and reset", async () => {
    ipc.runSpread.mockResolvedValue(spread);

    await click("Compare engines");
    expectComparison(true);
    await click("Edit design");
    expectComparison(false);

    await click("Compare engines");
    expectComparison(true);
    await click("Undo");
    expectComparison(false);

    await click("Compare engines");
    expectComparison(true);
    await click("Redo");
    expectComparison(false);

    await click("Compare engines");
    expectComparison(true);
    await click("Reset demo");
    expectComparison(false);
  });

  it("renders only the most recent overlapping comparison", async () => {
    const first = deferred<SpreadResult>();
    const second = deferred<SpreadResult>();
    ipc.runSpread.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    await click("Compare engines");
    await click("Compare engines");
    await act(async () => {
      first.resolve(spread);
      await Promise.resolve();
    });
    expectComparison(false);

    await act(async () => {
      second.resolve({ ...spread, apogee_spread_m: 2 });
      await Promise.resolve();
    });
    expectComparison(true);
    expect(container.textContent).toContain("Apogee delta: 2.0 m");
  });
});
