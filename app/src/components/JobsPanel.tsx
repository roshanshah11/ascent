// Thin studies/jobs panel (v0.3 Step 5). All state of record lives in
// Rust: studies come from the document snapshot, run/cancel go through
// IPC, and progress arrives as job events. This component only renders
// and forwards — it never computes or stores results.
import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DocumentState, JobDoneEvent, JobProgressEvent, Study } from "../core/types";
import { cancelJob, dispatchCommand, enqueueStudyJob, getDocument } from "../ipc";

interface RunState {
  jobId: number;
  completed: number;
  total: number;
}

function kindLabel(study: Study): string {
  const k = study.kind;
  switch (k.kind) {
    case "dispersion":
      return `dispersion · ${k.flights} flights`;
    case "single_flight":
      return "single flight";
    case "motor_trade":
      return "motor trade";
    case "stability_sweep":
      return "stability sweep";
  }
}

export default function JobsPanel({
  studies,
  onDocChange,
}: {
  studies: Study[];
  onDocChange: (doc: DocumentState) => void;
}) {
  // study_id → live run state while a job is in flight.
  const [running, setRunning] = useState<Record<number, RunState>>({});
  const [notice, setNotice] = useState<string | null>(null);
  const onDocChangeRef = useRef(onDocChange);
  onDocChangeRef.current = onDocChange;

  useEffect(() => {
    const subs: Promise<UnlistenFn>[] = [
      listen<JobProgressEvent>("job-progress", ({ payload }) => {
        setRunning((r) => ({
          ...r,
          [payload.study_id]: {
            jobId: payload.job_id,
            completed: payload.completed,
            total: payload.total,
          },
        }));
      }),
      listen<JobDoneEvent>("job-done", ({ payload }) => {
        setRunning((r) => {
          const next = { ...r };
          delete next[payload.study_id];
          return next;
        });
        if (payload.status === "failed" && payload.error) {
          setNotice(payload.error);
        }
        // Results landed in the Rust document — pull the fresh snapshot.
        getDocument().then((d) => onDocChangeRef.current(d)).catch(() => {});
      }),
    ];
    return () => {
      for (const sub of subs) sub.then((un) => un()).catch(() => {});
    };
  }, []);

  const createStudy = async () => {
    setNotice(null);
    try {
      const doc = await dispatchCommand({
        cmd: "create_study",
        name: `Dispersion ${studies.length + 1}`,
        kind: { kind: "dispersion", flights: 1000 },
        engine: "native",
        seed: 42,
      });
      onDocChange(doc);
    } catch (e) {
      setNotice(String(e));
    }
  };

  const run = async (study: Study) => {
    setNotice(null);
    try {
      const jobId = await enqueueStudyJob(study.id);
      setRunning((r) => ({ ...r, [study.id]: { jobId, completed: 0, total: 0 } }));
    } catch (e) {
      setNotice(String(e));
    }
  };

  const cancel = (state: RunState) => {
    cancelJob(state.jobId).catch((e) => setNotice(String(e)));
  };

  return (
    <section className="jobs-panel">
      <div className="jobs-toolbar">
        <div>
          <span className="panel-eyebrow">Compute queue</span>
          <h3>Studies & jobs</h3>
        </div>
        <button className="button-secondary" onClick={createStudy}>New dispersion study</button>
      </div>
      {notice && <div className="inline-error">{notice}</div>}
      {studies.length === 0 && (
        <div className="jobs-empty">
          No studies yet. A study is a saved question — one flight, a
          dispersion, a trade — with its results tracked against the design.
        </div>
      )}
      <ul className="jobs-list">
        {studies.map((study) => {
          const state = running[study.id];
          const pct =
            state && state.total > 0 ? Math.round((100 * state.completed) / state.total) : 0;
          return (
            <li
              key={study.id}
              className="job-row"
            >
              <span className={`job-indicator ${state ? "running" : study.results ? "done" : "queued"}`} />
              <div className="job-copy">
                <div className="job-name">{study.name}</div>
                <div className="job-meta">
                  {kindLabel(study)} · seed {study.seed} ·{" "}
                  {state
                    ? "running"
                    : study.results
                      ? `run ${study.results.input_hash.slice(0, 12)}`
                      : "not run"}
                </div>
              </div>
              {state ? (
                <>
                  <progress value={state.completed} max={Math.max(state.total, 1)} />
                  <span className="job-percent">{pct}%</span>
                  <button className="button-secondary" onClick={() => cancel(state)}>Cancel</button>
                </>
              ) : (
                <button className="button-secondary" onClick={() => run(study)}>Run</button>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
