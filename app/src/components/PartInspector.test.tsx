/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Vehicle } from "../core/types";
import PartInspector from "./PartInspector";

const vehicle: Vehicle = {
  name: "Test rocket",
  parts: [
    {
      id: 1,
      kind: { type: "body_tube", length_m: 0.3 },
      children: [
        {
          id: 3,
          kind: { type: "fin_set", span_m: 0.0375 },
          children: [],
        },
      ],
    },
  ],
};

describe("PartInspector", () => {
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

  it("finds nested fin sets and forwards preview and release through the command spine", async () => {
    const onPreview = vi.fn();
    const onCommit = vi.fn();
    await act(async () =>
      root.render(
        <PartInspector
          vehicle={vehicle}
          preview={null}
          previewing={false}
          error={null}
          onPreview={onPreview}
          onCommit={onCommit}
        />,
      ),
    );

    expect(container.textContent).toContain("Fin set #3");
    const input = container.querySelector<HTMLInputElement>('input[type="range"]')!;
    await act(async () => {
      input.dispatchEvent(new Event("pointerdown", { bubbles: true }));
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
      setter?.call(input, "0.05");
      input.dispatchEvent(new Event("input", { bubbles: true }));
      vi.advanceTimersByTime(120);
    });
    expect(onPreview).toHaveBeenCalledWith({ id: 3, param: "span_m", value: 0.05 });

    await act(async () => input.dispatchEvent(new Event("pointerup", { bubbles: true })));
    expect(onCommit).toHaveBeenCalledWith({
      cmd: "set_part_param",
      id: 3,
      param: "span_m",
      value: 0.05,
    });
  });
});
