/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DocumentState } from "../core/types";

const ipc = vi.hoisted(() => ({
  consoleExec: vi.fn(),
  fetchSessionJournal: vi.fn(),
}));

vi.mock("../ipc", () => ipc);

import Console from "./Console";

const docState: DocumentState = {
  vehicle: { name: "x", parts: [] },
  design: {} as DocumentState["design"],
  studies: [],
  can_undo: true,
  can_redo: false,
};

describe("Console", () => {
  let container: HTMLDivElement;
  let root: Root;
  let received: DocumentState | null;

  const input = () =>
    container.querySelector("input[aria-label='console input']") as HTMLInputElement;

  const type = async (value: string) => {
    await act(async () => {
      const el = input();
      // React reads the value through its own tracker; set natively.
      const setter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )!.set!;
      setter.call(el, value);
      el.dispatchEvent(new Event("input", { bubbles: true }));
    });
  };

  const enter = async () => {
    await act(async () => {
      input().dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
      );
      await Promise.resolve();
    });
  };

  beforeEach(async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    received = null;
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
    await act(async () => {
      root.render(<Console onDocChange={(d) => (received = d)} />);
      await Promise.resolve();
    });
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
    vi.clearAllMocks();
  });

  it("sends the line to the dispatcher and reports ok", async () => {
    ipc.consoleExec.mockResolvedValue(docState);
    await type("set-sim-param cd 0.7");
    await enter();
    expect(ipc.consoleExec).toHaveBeenCalledWith("set-sim-param cd 0.7");
    expect(received).toEqual(docState);
    expect(container.textContent).toContain("> set-sim-param cd 0.7");
    expect(container.textContent).toContain("ok");
    expect(input().value).toBe("");
  });

  it("renders dispatcher errors without touching the document", async () => {
    ipc.consoleExec.mockRejectedValue("design has no parameter 'dc'");
    await type("set-sim-param dc 0.7");
    await enter();
    expect(container.textContent).toContain("design has no parameter 'dc'");
    expect(received).toBeNull();
  });

  it("dumps the session journal", async () => {
    ipc.fetchSessionJournal.mockResolvedValue('{"journal_version":1}\n{"op":"undo"}\n');
    await type("journal");
    await enter();
    expect(ipc.consoleExec).not.toHaveBeenCalled();
    expect(container.textContent).toContain('{"op":"undo"}');
  });

  it("recalls history with the arrow keys", async () => {
    ipc.consoleExec.mockResolvedValue(docState);
    await type("select-motor B6");
    await enter();
    await act(async () => {
      input().dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }),
      );
    });
    expect(input().value).toBe("select-motor B6");
  });
});
