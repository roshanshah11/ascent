/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CommandProposal, CounterfactualReview, DocumentState } from "../core/types";

const ipc = vi.hoisted(() => ({
  fetchCounterfactualReview: vi.fn(),
  applyProposal: vi.fn(),
}));

vi.mock("../ipc", () => ipc);

import CopilotPanel from "./CopilotPanel";

const proposal: CommandProposal = {
  valid: true,
  commands: [{ index: 0, text: "set-sim-param cd 0.58", error: null }],
  diff: {
    vehicle_changed: false,
    design_changed: true,
    atmosphere_changed: false,
    parts_added: 0,
    parts_removed: 0,
    studies_added: 0,
    studies_removed: 0,
    studies_made_stale: [4],
  },
};

const documentState = {
  vehicle: { name: "Alpha III", parts: [] },
  design: {
    name: "Alpha III",
    dry_mass_g: 34,
    diameter_mm: 25,
    cd: 0.58,
    chute: { enabled: true, diameter_cm: 30, cd: 0.75 },
    motor_designation: "C6",
    rail_length_m: 0.9,
  },
  studies: [],
  can_undo: true,
  can_redo: false,
} satisfies DocumentState;

const universe = {
  proposal,
  baseline_run: { summary: { apogee_m: 300 } },
  proposed_run: { summary: { apogee_m: 315 } },
  baseline_markers: { cg_ignition_from_nose_m: 0.2, cp_from_nose_m: 0.3, stability_ignition_cal: 4 },
  proposed_markers: { cg_ignition_from_nose_m: 0.21, cp_from_nose_m: 0.32, stability_ignition_cal: 4.4 },
  baseline_evidence: { input_hash: "a".repeat(64) },
  proposed_evidence: { input_hash: "b".repeat(64) },
  baseline_reconciliation: null,
  proposed_reconciliation: null,
  qualifications: ["measured reconciliation unavailable"],
  report_preview_html: "<!doctype html><p>proposal report</p>",
} as CounterfactualReview;

describe("CopilotPanel", () => {
  let container: HTMLDivElement;
  let root: Root;
  const onDocChange = vi.fn();
  const onApplied = vi.fn();

  beforeEach(async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
    ipc.fetchCounterfactualReview.mockResolvedValue(universe);
    ipc.applyProposal.mockResolvedValue(documentState);
    await act(async () => root.render(<CopilotPanel onDocChange={onDocChange} onApplied={onApplied} />));
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
    vi.clearAllMocks();
  });

  const click = async (label: string) => {
    const button = Array.from(container.querySelectorAll("button")).find(
      (candidate) => candidate.textContent?.trim() === label,
    );
    if (!button) throw new Error(`missing ${label}`);
    await act(async () => {
      button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
    });
  };

  it("dry-runs a command batch and surfaces evidence staleness before approval", async () => {
    await click("Preview changes");

    expect(ipc.fetchCounterfactualReview).toHaveBeenCalledWith(["set-sim-param cd 0.58"]);
    expect(container.textContent).toContain("Validated");
    expect(container.textContent).toContain("1 studies made stale");
    expect(container.textContent).toContain("300.00 → 315.00 m");
  });

  it("applies only a validated proposal through the atomic backend seam", async () => {
    await click("Preview changes");
    await click("Approve & apply");

    expect(ipc.applyProposal).toHaveBeenCalledWith(["set-sim-param cd 0.58"]);
    expect(onDocChange).toHaveBeenCalledWith(documentState);
    expect(onApplied).toHaveBeenCalledOnce();
  });

  it("Escape rejects the entire preview without dispatching", async () => {
    await click("Preview changes");
    expect(container.textContent).toContain("Validated");

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });

    expect(container.textContent).not.toContain("Validated");
    expect(ipc.applyProposal).not.toHaveBeenCalled();
    expect(onDocChange).not.toHaveBeenCalled();
  });
});
