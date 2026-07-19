/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Workbench, { type Workspace } from "./Workbench";

const WORKSPACES: Workspace[] = ["design", "simulate", "results", "review"];

describe("Workbench", () => {
  let container: HTMLDivElement;
  let root: Root;

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

  const tab = (label: string) =>
    Array.from(container.querySelectorAll('[role="tab"]')).find(
      (t) => t.textContent === label,
    ) as HTMLElement;

  const render = async (
    workspace: Workspace,
    onWorkspaceChange: (w: Workspace) => void,
    disabled: Workspace[] = [],
  ) => {
    await act(async () => {
      root.render(
        <Workbench
          workspace={workspace}
          onWorkspaceChange={onWorkspaceChange}
          disabledWorkspaces={disabled}
          title="Ascent"
          toolbar={<button>Run</button>}
        >
          <div>content-{workspace}</div>
        </Workbench>,
      );
      await Promise.resolve();
    });
  };

  it("renders a tab per workspace with the current one marked active", async () => {
    await render("design", vi.fn());
    for (const w of WORKSPACES) {
      const el = tab(w[0].toUpperCase() + w.slice(1));
      expect(el).toBeTruthy();
    }
    expect(tab("Design").getAttribute("aria-selected")).toBe("true");
    expect(tab("Simulate").getAttribute("aria-selected")).toBe("false");
  });

  it("calls onWorkspaceChange when a tab is clicked", async () => {
    const onWorkspaceChange = vi.fn();
    await render("design", onWorkspaceChange);
    await act(async () => {
      tab("Results").dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(onWorkspaceChange).toHaveBeenCalledWith("results");
  });

  it("disables tabs listed in disabledWorkspaces and does not dispatch on click", async () => {
    const onWorkspaceChange = vi.fn();
    await render("design", onWorkspaceChange, ["simulate"]);
    const simulate = tab("Simulate");
    expect(simulate.hasAttribute("disabled")).toBe(true);
    await act(async () => {
      simulate.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(onWorkspaceChange).not.toHaveBeenCalled();
  });

  it("renders the toolbar and the active workspace's children", async () => {
    await render("results", vi.fn());
    expect(container.textContent).toContain("content-results");
    expect(container.querySelector("button")?.textContent).toBe("Run");
  });

  it("renders the title", async () => {
    await render("design", vi.fn());
    expect(container.textContent).toContain("Ascent");
  });
});
