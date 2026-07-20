/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import AtmospherePanel from "./AtmospherePanel";

it("clears an imported profile through the supplied mutation seam", async () => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  const onClear = vi.fn(() => Promise.resolve());

  await act(async () => {
    root.render(
      <AtmospherePanel
        profile={{
          name: "koun-12z.csv",
          layers: [
            { altitude_m: 0, wind_speed_ms: 3, wind_direction_deg: 0 },
            { altitude_m: 1000, wind_speed_ms: 6, wind_direction_deg: 20 },
          ],
        }}
        onImport={() => Promise.resolve()}
        onClear={onClear}
      />,
    );
  });

  const button = Array.from(container.querySelectorAll("button")).find(
    (candidate) => candidate.textContent === "Use analytic atmosphere",
  );
  expect(button).toBeDefined();
  await act(async () => button?.click());
  expect(onClear).toHaveBeenCalledOnce();

  await act(async () => root.unmount());
  container.remove();
});
