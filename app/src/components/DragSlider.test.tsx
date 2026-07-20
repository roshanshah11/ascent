/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import DragSlider from "./DragSlider";

const target = { id: 3, param: "span_m", startValue: 0.0375 };

describe("DragSlider", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  async function render(onPreview = vi.fn(), onCommit = vi.fn()) {
    await act(async () =>
      root.render(
        <DragSlider
          label="Fin span"
          target={target}
          min={0.02}
          max={0.08}
          step={0.0025}
          onPreview={onPreview}
          onCommit={onCommit}
        />,
      ),
    );
    return {
      input: container.querySelector<HTMLInputElement>('input[type="range"]')!,
      onPreview,
      onCommit,
    };
  }

  function setRange(input: HTMLInputElement, value: string) {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
    setter?.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }

  it("debounces moves and commits exactly the final value on release", async () => {
    const { input, onPreview, onCommit } = await render();
    await act(async () => {
      input.dispatchEvent(new Event("pointerdown", { bubbles: true }));
      setRange(input, "0.045");
      setRange(input, "0.05");
      vi.advanceTimersByTime(120);
    });
    expect(onPreview).toHaveBeenCalledTimes(1);
    expect(onPreview).toHaveBeenLastCalledWith({ id: 3, param: "span_m", value: 0.05 });

    await act(async () => input.dispatchEvent(new Event("pointerup", { bubbles: true })));
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith({
      cmd: "set_part_param",
      id: 3,
      param: "span_m",
      value: 0.05,
    });
  });

  it("does not commit a no-op gesture", async () => {
    const { input, onCommit } = await render();
    await act(async () => {
      input.dispatchEvent(new Event("pointerdown", { bubbles: true }));
      input.dispatchEvent(new Event("pointerup", { bubbles: true }));
    });
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("Escape restores the starting preview and emits no command", async () => {
    const { input, onPreview, onCommit } = await render();
    await act(async () => {
      input.dispatchEvent(new Event("pointerdown", { bubbles: true }));
      setRange(input, "0.06");
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
    expect(input.value).toBe("0.0375");
    expect(onPreview).toHaveBeenLastCalledWith({ id: 3, param: "span_m", value: 0.0375 });
    expect(onCommit).not.toHaveBeenCalled();
  });
});
