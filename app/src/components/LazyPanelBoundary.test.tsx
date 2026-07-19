/** @vitest-environment jsdom */

import { act, type ComponentType, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";

it("contains a rejected lazy panel and renders its scoped message", async () => {
  let Boundary: ComponentType<{ children: ReactNode; message: string }> | undefined;
  try {
    const module = await vi.importActual("./LazyPanelBoundary");
    Boundary = (module as { default: typeof Boundary }).default;
  } catch {
    // The red phase intentionally reaches the assertion below while the
    // boundary module does not exist yet.
  }
  expect(Boundary, "LazyPanelBoundary module is missing").toBeDefined();
  if (!Boundary) return;

  const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  const container = document.createElement("div");
  const root = createRoot(container);
  const Broken = () => {
    throw new Error("chunk unavailable");
  };

  await act(async () => {
    root.render(
      <Boundary message="Unable to load this viewport.">
        <Broken />
      </Boundary>,
    );
  });

  expect(container.querySelector('[role="alert"]')?.textContent).toBe(
    "Unable to load this viewport.",
  );
  await act(async () => root.unmount());
  consoleError.mockRestore();
});
