/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GrammarCommand } from "../ipc";
import CommandPalette, { type PaletteAction } from "./CommandPalette";

const grammarCommands: GrammarCommand[] = [
  {
    verb: "select-motor",
    usage: "select-motor <designation>",
    description: "Select the motor",
  },
  {
    verb: "custom-rust-verb",
    usage: "custom-rust-verb <value>",
    description: "Provided by the Rust catalogue",
  },
];

describe("CommandPalette", () => {
  let container: HTMLDivElement;
  let root: Root;

  const input = () => container.querySelector<HTMLInputElement>('[aria-label="command palette input"]')!;
  const items = () => Array.from(container.querySelectorAll('[role="option"]'));
  const itemTitles = () =>
    items().map((i) => i.querySelector(".command-palette-item-title")?.textContent);

  const type = async (value: string) => {
    await act(async () => {
      const el = input();
      const setter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )!.set!;
      setter.call(el, value);
      el.dispatchEvent(new Event("input", { bubbles: true }));
    });
  };

  const keydown = async (key: string) => {
    await act(async () => {
      input().dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
      await Promise.resolve();
    });
  };

  beforeEach(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  const renderPalette = async (props: {
    open: boolean;
    onClose: () => void;
    actions: PaletteAction[];
    onExec: (line: string) => Promise<void>;
    grammarCommands?: GrammarCommand[];
  }) => {
    await act(async () => {
      root.render(
        <CommandPalette
          {...props}
          grammarCommands={props.grammarCommands ?? grammarCommands}
        />,
      );
      await Promise.resolve();
    });
  };

  it("renders nothing when closed", async () => {
    await renderPalette({ open: false, onClose: vi.fn(), actions: [], onExec: vi.fn() });
    expect(container.querySelector(".command-palette")).toBeNull();
  });

  it("lists named actions and grammar verbs when opened, actions first", async () => {
    const runDesign = vi.fn();
    await renderPalette({
      open: true,
      onClose: vi.fn(),
      actions: [{ id: "goto-design", title: "Switch to Design", run: runDesign }],
      onExec: vi.fn(),
    });
    const titles = itemTitles();
    expect(titles).toContain("Switch to Design");
    expect(titles).toContain("custom-rust-verb");
    expect(titles.indexOf("Switch to Design")).toBeLessThan(
      titles.indexOf("custom-rust-verb")!,
    );
  });

  it("filters the list as the query changes", async () => {
    await renderPalette({
      open: true,
      onClose: vi.fn(),
      actions: [{ id: "goto-design", title: "Switch to Design", run: vi.fn() }],
      onExec: vi.fn(),
    });
    await type("custom-rust");
    expect(itemTitles()).toEqual(["custom-rust-verb"]);
  });

  it("runs the selected named action on Enter and closes", async () => {
    const runDesign = vi.fn();
    const onClose = vi.fn();
    await renderPalette({
      open: true,
      onClose,
      actions: [{ id: "goto-design", title: "Switch to Design", run: runDesign }],
      onExec: vi.fn(),
    });
    await type("switch to design");
    await keydown("Enter");
    expect(runDesign).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("autocompletes a bare grammar verb into the input instead of dispatching", async () => {
    const onExec = vi.fn();
    const onClose = vi.fn();
    await renderPalette({ open: true, onClose, actions: [], onExec });
    await type("custom-rust-verb");
    await keydown("Enter");
    expect(onExec).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    expect(input().value).toBe("custom-rust-verb ");
  });

  it("dispatches a full grammar command line through onExec (console_exec path) and closes", async () => {
    const onExec = vi.fn(() => Promise.resolve());
    const onClose = vi.fn();
    await renderPalette({ open: true, onClose, actions: [], onExec });
    await type("select-motor C6");
    await keydown("Enter");
    expect(onExec).toHaveBeenCalledWith("select-motor C6");
    await act(async () => {
      await Promise.resolve();
    });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("shows the dispatch error and keeps the palette open when onExec rejects", async () => {
    const onExec = vi.fn(() => Promise.reject(new Error("bad param")));
    const onClose = vi.fn();
    await renderPalette({ open: true, onClose, actions: [], onExec });
    await type("select-motor nope");
    await keydown("Enter");
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(onClose).not.toHaveBeenCalled();
    expect(container.textContent).toContain("bad param");
  });

  it("navigates the selection with arrow keys", async () => {
    await renderPalette({
      open: true,
      onClose: vi.fn(),
      actions: [
        { id: "a", title: "Switch to Design", run: vi.fn() },
        { id: "b", title: "Switch to Results", run: vi.fn() },
      ],
      onExec: vi.fn(),
    });
    expect(items()[0].getAttribute("aria-selected")).toBe("true");
    await keydown("ArrowDown");
    expect(items()[1].getAttribute("aria-selected")).toBe("true");
    await keydown("ArrowUp");
    expect(items()[0].getAttribute("aria-selected")).toBe("true");
  });

  it("closes on Escape without running anything", async () => {
    const onClose = vi.fn();
    const run = vi.fn();
    await renderPalette({
      open: true,
      onClose,
      actions: [{ id: "a", title: "Switch to Design", run }],
      onExec: vi.fn(),
    });
    await keydown("Escape");
    expect(onClose).toHaveBeenCalledOnce();
    expect(run).not.toHaveBeenCalled();
  });

  it("resets the query and selection each time it reopens", async () => {
    const { rerender } = { rerender: async (open: boolean) => renderPalette({ open, onClose: vi.fn(), actions: [], onExec: vi.fn() }) };
    await rerender(true);
    await type("custom-rust-verb");
    await rerender(false);
    await rerender(true);
    expect(input().value).toBe("");
  });
});
