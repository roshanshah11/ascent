/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Design } from "../core/types";
import Inspector from "./Inspector";

const design: Design = {
  name: "Estes Alpha III",
  dry_mass_g: 34,
  diameter_mm: 25,
  cd: 0.6,
  chute: { enabled: true, diameter_cm: 30, cd: 0.75 },
  motor_designation: "C6",
  rail_length_m: 0.9,
};

describe("Inspector recovery controls", () => {
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

  it("enables dual deploy as one design edit with usable defaults", async () => {
    const onChange = vi.fn();
    await act(async () => root.render(<Inspector design={design} onChange={onChange} />));

    const label = Array.from(container.querySelectorAll("label")).find((candidate) =>
      candidate.textContent?.includes("Dual deploy"),
    );
    expect(label).toBeDefined();
    const checkbox = label?.querySelector<HTMLInputElement>('input[type="checkbox"]');

    await act(async () => checkbox?.click());

    expect(onChange).toHaveBeenCalledWith({
      ...design,
      chute: {
        ...design.chute,
        main_deploy_altitude_m: 60,
        drogue_diameter_cm: 8,
        drogue_cd: 0.8,
      },
    });
  });

  it("edits the main deployment altitude for an active dual-deploy design", async () => {
    const onChange = vi.fn();
    const dual: Design = {
      ...design,
      chute: {
        ...design.chute,
        main_deploy_altitude_m: 60,
        drogue_diameter_cm: 8,
        drogue_cd: 0.8,
      },
    };
    await act(async () => root.render(<Inspector design={dual} onChange={onChange} />));

    const label = Array.from(container.querySelectorAll("label")).find((candidate) =>
      candidate.textContent?.includes("Main deploy AGL (m)"),
    );
    expect(label).toBeDefined();
    const input = label?.querySelector<HTMLInputElement>('input[type="number"]');
    expect(input).toBeDefined();

    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
      setter?.call(input, "75");
      input?.dispatchEvent(new Event("input", { bubbles: true }));
    });

    expect(onChange).toHaveBeenCalledWith({
      ...dual,
      chute: { ...dual.chute, main_deploy_altitude_m: 75 },
    });
  });
});
