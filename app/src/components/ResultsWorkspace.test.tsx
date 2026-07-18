/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Study } from "../core/types";
import ResultsWorkspace from "./ResultsWorkspace";

const study = (id: number, name: string, apogees: number[]): Study => ({
  id,
  name,
  kind: { kind: "dispersion", flights: apogees.length },
  engine: "native",
  seed: 42,
  results: {
    input_hash: "hash",
    data: {
      seed: 42,
      samples: apogees.length,
      vary: [],
      apogee_p5_m: Math.min(...apogees),
      apogee_p50_m: apogees[0],
      apogee_p95_m: Math.max(...apogees),
      landing_mean_m: 55,
      landing_ellipse: { a_m: 20, b_m: 0, bearing_deg: 0 },
      runs: apogees.map((a, i) => ({
        apogee_m: a,
        landing_range_m: 40 + i * 10,
        max_aoa_deg: 2,
      })),
    },
  },
});

describe("ResultsWorkspace", () => {
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

  const render = async (studies: Study[]) => {
    await act(async () => {
      root.render(<ResultsWorkspace studies={studies} record={null} />);
      await Promise.resolve();
    });
  };

  it("overlays two completed studies with a legend and scatter points", async () => {
    await render([study(1, "baseline", [300, 310]), study(2, "heavy", [280])]);
    expect(container.textContent).toContain("baseline");
    expect(container.textContent).toContain("heavy");
    // 3 scatter points across the two series.
    expect(container.querySelectorAll("circle").length).toBe(3);
    // Percentile cards for both studies.
    expect(container.textContent).toContain("apogee p5 / p50 / p95");
    expect(container.textContent).toContain("2 flights");
    expect(container.textContent).toContain("1 flights");
  });

  it("removes a deselected study from the overlay", async () => {
    await render([study(1, "baseline", [300, 310]), study(2, "heavy", [280])]);
    const checkbox = Array.from(container.querySelectorAll("input[type=checkbox]"))[1];
    await act(async () => {
      (checkbox as HTMLInputElement).click();
    });
    expect(container.querySelectorAll("circle").length).toBe(2);
  });

  it("offers CSV export only when something is plotted", async () => {
    await render([]);
    const labels = Array.from(container.querySelectorAll("button")).map((b) => b.textContent);
    expect(labels).not.toContain("Export CSV");
    await render([study(1, "baseline", [300])]);
    const after = Array.from(container.querySelectorAll("button")).map((b) => b.textContent);
    expect(after).toContain("Export CSV");
  });

  it("skips unrun studies in the selector", async () => {
    const unrun: Study = {
      id: 9,
      name: "not yet",
      kind: { kind: "dispersion", flights: 100 },
      engine: "native",
      seed: 1,
    };
    await render([unrun]);
    expect(container.querySelectorAll("input[type=checkbox]").length).toBe(0);
    expect(container.textContent).toContain("No completed dispersion studies");
  });
});
