/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DocumentState, JobDoneEvent, JobProgressEvent, Study } from "../core/types";

const ipc = vi.hoisted(() => ({
  enqueueStudyJob: vi.fn(),
  cancelJob: vi.fn(() => Promise.resolve()),
  dispatchCommand: vi.fn(),
  getDocument: vi.fn(),
}));

// Captures the job-event handlers so tests can fire events like Tauri would.
const events = vi.hoisted(() => ({
  handlers: {} as Record<string, (event: { payload: unknown }) => void>,
}));

vi.mock("../ipc", () => ipc);
vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, handler: (event: { payload: unknown }) => void) => {
    events.handlers[name] = handler;
    return Promise.resolve(() => {
      delete events.handlers[name];
    });
  },
}));

import JobsPanel from "./JobsPanel";

const study: Study = {
  id: 1,
  name: "Landing spread",
  kind: { kind: "dispersion", flights: 1000 },
  engine: "native",
  seed: 42,
};

describe("JobsPanel", () => {
  let container: HTMLDivElement;
  let root: Root;
  let latestDoc: DocumentState | null;

  const render = async (studies: Study[]) => {
    await act(async () => {
      root.render(
        <JobsPanel studies={studies} onDocChange={(d) => (latestDoc = d)} />,
      );
      await Promise.resolve();
    });
  };

  const button = (label: string) => {
    const found = Array.from(container.querySelectorAll("button")).find(
      (candidate) => candidate.textContent === label,
    );
    if (!found) throw new Error(`missing ${label} button`);
    return found;
  };

  const click = async (label: string) => {
    await act(async () => {
      button(label).dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
  };

  const fire = async (name: string, payload: unknown) => {
    await act(async () => {
      events.handlers[name]?.({ payload });
      await Promise.resolve();
    });
  };

  beforeEach(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    latestDoc = null;
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
    vi.clearAllMocks();
    events.handlers = {};
  });

  it("runs a study and renders live progress from job events", async () => {
    ipc.enqueueStudyJob.mockResolvedValue(3);
    await render([study]);
    expect(container.textContent).toContain("not run");

    await click("Run");
    expect(ipc.enqueueStudyJob).toHaveBeenCalledWith(1);

    const progress: JobProgressEvent = {
      event: "progress",
      job_id: 3,
      study_id: 1,
      completed: 500,
      total: 1000,
    };
    await fire("job-progress", progress);
    expect(container.textContent).toContain("50%");
    expect(container.querySelector("progress")).not.toBeNull();
  });

  it("refreshes the document when a job finishes", async () => {
    ipc.enqueueStudyJob.mockResolvedValue(3);
    const finished: DocumentState = {
      vehicle: { name: "x", parts: [] },
      design: {} as DocumentState["design"],
      studies: [{ ...study, results: { input_hash: "abcdef123456", data: {} } }],
      can_undo: true,
      can_redo: false,
    };
    ipc.getDocument.mockResolvedValue(finished);
    await render([study]);
    await click("Run");

    const done: JobDoneEvent = {
      event: "done",
      job_id: 3,
      study_id: 1,
      status: "done",
      error: null,
    };
    await fire("job-done", done);
    expect(ipc.getDocument).toHaveBeenCalled();
    expect(latestDoc).toEqual(finished);
    // Fresh snapshot rendered by the parent shows the landed hash.
    await render(finished.studies);
    expect(container.textContent).toContain("run abcdef123456");
  });

  it("cancels through IPC with the running job id", async () => {
    ipc.enqueueStudyJob.mockResolvedValue(7);
    await render([study]);
    await click("Run");
    await fire("job-progress", {
      event: "progress",
      job_id: 7,
      study_id: 1,
      completed: 50,
      total: 1000,
    });
    await click("Cancel");
    expect(ipc.cancelJob).toHaveBeenCalledWith(7);
  });

  it("creates a dispersion study through the command spine", async () => {
    const withStudy: DocumentState = {
      vehicle: { name: "x", parts: [] },
      design: {} as DocumentState["design"],
      studies: [study],
      can_undo: true,
      can_redo: false,
    };
    ipc.dispatchCommand.mockResolvedValue(withStudy);
    await render([]);
    expect(container.textContent).toContain("No studies yet");

    await click("New dispersion study");
    expect(ipc.dispatchCommand).toHaveBeenCalledWith({
      cmd: "create_study",
      name: "Dispersion 1",
      kind: { kind: "dispersion", flights: 1000 },
      engine: "native",
      seed: 42,
    });
    expect(latestDoc).toEqual(withStudy);
  });
});
