import { useEffect, useReducer, useRef, useState } from "react";
import DispersionView from "./components/DispersionView";
import EvidenceDrawer from "./components/EvidenceDrawer";
import FlightMode from "./components/FlightMode";
import Inspector from "./components/Inspector";
import MotorSelector from "./components/MotorSelector";
import ReviewPanel from "./components/ReviewPanel";
import SpreadPanel from "./components/SpreadPanel";
import Viewport from "./components/Viewport";
import Viewport3D from "./components/Viewport3D";
import { diffDesign } from "./core/commandDiff";
import { initialRunStatus, reduceRunStatus } from "./core/runState";
import type {
  Design,
  DocumentState,
  MotorInfo,
  Project,
  RunRecord,
  SpreadResult,
} from "./core/types";
import {
  autosaveProject,
  checkRecovery,
  discardRecovery,
  dispatchCommand,
  fetchMotors,
  fetchReferenceDesign,
  getDocument,
  redoDocument,
  runSimulation,
  runSpread,
  undoDocument,
} from "./ipc";

const STATE_BADGE: Record<string, { label: string; color: string }> = {
  current: { label: "Current", color: "#3fb950" },
  dirty: { label: "Not run", color: "#e8a33d" },
  running: { label: "Running…", color: "#58a6ff" },
  stale: { label: "Stale result", color: "#c74b3c" },
};

export default function App() {
  // The Rust document store owns all user state (v0.3); we render its
  // snapshots and send commands. No design mutation happens client-side.
  const [doc, setDoc] = useState<DocumentState | null>(null);
  const [view3d, setView3d] = useState(false);
  const [motors, setMotors] = useState<MotorInfo[]>([]);
  const [record, setRecord] = useState<RunRecord | null>(null);
  const [spread, setSpread] = useState<SpreadResult | null>(null);
  const [status, dispatch] = useReducer(reduceRunStatus, initialRunStatus);
  const [mode, setMode] = useState<"design" | "flight" | "review">("design");
  const [error, setError] = useState<string | null>(null);
  // Every design mutation and comparison request advances this generation.
  // A completion may render only while it still describes the current design.
  const comparisonGeneration = useRef(0);
  const [recovery, setRecovery] = useState<Project | null>(null);
  // Latest state for the autosave interval, without re-arming the timer on
  // every render.
  const autosaveRef = useRef<{ design: Design | null; record: RunRecord | null; dirty: boolean }>({
    design: null,
    record: null,
    dirty: false,
  });
  autosaveRef.current = {
    design: doc?.design ?? null,
    record,
    dirty: status.state === "dirty",
  };

  useEffect(() => {
    getDocument().then(setDoc).catch((e) => setError(String(e)));
    fetchMotors().then(setMotors).catch((e) => setError(String(e)));
    // A surviving autosave means the last session crashed — offer a restore.
    checkRecovery().then(setRecovery).catch(() => {});
  }, []);

  // MS-Office style autosave: every 30 s, if there are unsaved edits, write
  // the crash-recovery file. Never touches the user's own project file.
  useEffect(() => {
    const timer = setInterval(() => {
      const { design: d, record: r, dirty } = autosaveRef.current;
      if (!d || !dirty) return;
      const project: Project = {
        schema_version: 1,
        name: d.name,
        designs: [d],
        runs: r ? [r] : [],
      };
      autosaveProject(project).catch(() => {});
    }, 30_000);
    return () => clearInterval(timer);
  }, []);

  if (!doc) {
    return <div style={{ padding: 24 }}>{error ?? "Loading document…"}</div>;
  }
  const design = doc.design;

  const invalidateSpread = () => {
    comparisonGeneration.current += 1;
    setSpread(null);
  };

  const markEdited = (next: DocumentState) => {
    setDoc(next);
    invalidateSpread();
    dispatch({ type: "EDIT" });
  };

  const edit = async (next: Design) => {
    const cmds = diffDesign(design, next);
    if (cmds.length === 0) return;
    try {
      let state = doc;
      for (const cmd of cmds) {
        state = await dispatchCommand(cmd);
      }
      markEdited(state);
    } catch (e) {
      setError(String(e));
    }
  };

  // An undone design is a dirty design — the run-state machine stays authoritative.
  const doUndo = async () => {
    if (!doc.can_undo) return;
    markEdited(await undoDocument());
  };

  const doRedo = async () => {
    if (!doc.can_redo) return;
    markEdited(await redoDocument());
  };

  const run = async () => {
    dispatch({ type: "RUN_START" });
    setError(null);
    try {
      const result = await runSimulation(design);
      setRecord(result);
      dispatch({ type: "RUN_SUCCESS" });
      setMode("flight");
    } catch (e) {
      setError(String(e));
      dispatch({ type: "RUN_FAIL" });
    }
  };

  const compare = async () => {
    const generation = ++comparisonGeneration.current;
    setSpread(null);
    setError(null);
    try {
      const result = await runSpread(design);
      if (generation === comparisonGeneration.current) setSpread(result);
    } catch (e) {
      if (generation === comparisonGeneration.current) setError(String(e));
    }
  };

  // Demo reset: journaled coarse design replacement, back to design mode.
  const reset = async () => {
    setError(null);
    setRecord(null);
    invalidateSpread();
    setMode("design");
    dispatch({ type: "RESET" });
    try {
      const reference = await fetchReferenceDesign();
      setDoc(await dispatchCommand({ cmd: "set_design", design: reference }));
    } catch (e) {
      setError(String(e));
    }
  };

  const restoreRecovery = async () => {
    if (!recovery) return;
    try {
      if (recovery.designs.length > 0) {
        markEdited(
          await dispatchCommand({ cmd: "set_design", design: recovery.designs[0] }),
        ); // Restored work is unsaved work.
      }
      setRecord(recovery.runs.length > 0 ? recovery.runs[recovery.runs.length - 1] : null);
    } catch (e) {
      setError(String(e));
    }
    setRecovery(null);
    discardRecovery().catch(() => {});
  };

  const dismissRecovery = () => {
    setRecovery(null);
    discardRecovery().catch(() => {});
  };

  const badge = STATE_BADGE[status.state];

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "z") return;
    e.preventDefault();
    if (e.shiftKey) void doRedo();
    else void doUndo();
  };

  return (
    <div style={{ padding: 20 }} tabIndex={-1} onKeyDown={onKeyDown}>
      <header style={{ display: "flex", alignItems: "center", gap: 16, marginBottom: 16 }}>
        <h2 style={{ margin: 0 }}>Ascent</h2>
        <span
          style={{
            border: `1px solid ${badge.color}`,
            color: badge.color,
            borderRadius: 12,
            padding: "2px 10px",
            fontSize: 12,
          }}
        >
          {badge.label}
        </span>
        <button onClick={run} disabled={status.state === "running"}>
          Run simulation
        </button>
        <button onClick={compare}>Compare engines</button>
        <button onClick={reset} title="Restore the reference design and clear results">
          Reset demo
        </button>
        <button onClick={doUndo} disabled={!doc.can_undo} title="Undo (⌘Z)">
          Undo
        </button>
        <button onClick={doRedo} disabled={!doc.can_redo} title="Redo (⇧⌘Z)">
          Redo
        </button>
        <nav style={{ marginLeft: "auto" }}>
          <button onClick={() => setMode("design")} disabled={mode === "design"}>
            Design
          </button>
          <button
            onClick={() => setMode("flight")}
            disabled={mode === "flight" || !record}
          >
            Flight
          </button>
          <button onClick={() => setMode("review")} disabled={mode === "review"}>
            Flight Review
          </button>
        </nav>
      </header>

      {recovery && (
        <div
          style={{
            border: "1px solid #e8a33d",
            borderRadius: 6,
            padding: "8px 12px",
            marginBottom: 12,
            display: "flex",
            alignItems: "center",
            gap: 12,
          }}
        >
          <span>
            Ascent didn't exit cleanly last time. Restore the autosaved
            project “{recovery.name}”?
          </span>
          <button onClick={restoreRecovery}>Restore</button>
          <button onClick={dismissRecovery}>Discard</button>
        </div>
      )}

      {error && <div style={{ color: "#c74b3c", marginBottom: 12 }}>{error}</div>}

      {mode === "design" ? (
        <>
          <div style={{ display: "flex", gap: 32 }}>
            <div>
              {view3d ? <Viewport3D vehicle={doc.vehicle} /> : <Viewport design={design} />}
              <div style={{ marginTop: 4 }}>
                <button onClick={() => setView3d(false)} disabled={!view3d}>
                  2D
                </button>
                <button onClick={() => setView3d(true)} disabled={view3d}>
                  3D
                </button>
              </div>
            </div>
            <Inspector design={design} onChange={edit} />
            <MotorSelector motors={motors} design={design} onChange={edit} />
          </div>
          <DispersionView design={design} />
        </>
      ) : mode === "flight" ? (
        record && <FlightMode record={record} />
      ) : (
        <ReviewPanel />
      )}

      {spread && <SpreadPanel spread={spread} />}

      {record && (
        <footer style={{ marginTop: 20, fontSize: 11, color: "#9aa1ab" }}>
          run {record.summary.input_hash.slice(0, 12)} · deterministic · offline
          <EvidenceDrawer design={record.design} />
        </footer>
      )}
    </div>
  );
}
