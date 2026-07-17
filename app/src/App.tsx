import { useEffect, useReducer, useRef, useState } from "react";
import EvidenceDrawer from "./components/EvidenceDrawer";
import FlightMode from "./components/FlightMode";
import Inspector from "./components/Inspector";
import MotorSelector from "./components/MotorSelector";
import ReviewPanel from "./components/ReviewPanel";
import Viewport from "./components/Viewport";
import {
  canRedo,
  canUndo,
  emptyHistory,
  pushCommand,
  redo,
  replaceDesign,
  undo,
  type CommandHistory,
} from "./core/commands";
import { initialRunStatus, reduceRunStatus } from "./core/runState";
import type { Design, MotorInfo, RunRecord } from "./core/types";
import { fetchMotors, fetchReferenceDesign, runSimulation } from "./ipc";

const STATE_BADGE: Record<string, { label: string; color: string }> = {
  current: { label: "Current", color: "#3fb950" },
  dirty: { label: "Not run", color: "#e8a33d" },
  running: { label: "Running…", color: "#58a6ff" },
  stale: { label: "Stale result", color: "#c74b3c" },
};

export default function App() {
  const [design, setDesign] = useState<Design | null>(null);
  const [motors, setMotors] = useState<MotorInfo[]>([]);
  const [record, setRecord] = useState<RunRecord | null>(null);
  const [status, dispatch] = useReducer(reduceRunStatus, initialRunStatus);
  const [mode, setMode] = useState<"design" | "flight" | "review">("design");
  const [error, setError] = useState<string | null>(null);
  // Undo/redo history lives in a ref: commands close over design snapshots,
  // so the ref never needs to trigger renders itself — setDesign does that.
  const historyRef = useRef<CommandHistory>(emptyHistory);
  const [, bumpHistory] = useReducer((n: number) => n + 1, 0);

  useEffect(() => {
    fetchReferenceDesign().then(setDesign).catch((e) => setError(String(e)));
    fetchMotors().then(setMotors).catch((e) => setError(String(e)));
  }, []);

  if (!design) {
    return <div style={{ padding: 24 }}>{error ?? "Loading reference design…"}</div>;
  }

  const edit = (next: Design) => {
    const r = pushCommand(historyRef.current, replaceDesign(design, next), design);
    historyRef.current = r.history;
    setDesign(r.design);
    bumpHistory();
    dispatch({ type: "EDIT" });
  };

  // An undone design is a dirty design — the run-state machine stays authoritative.
  const doUndo = () => {
    if (!canUndo(historyRef.current)) return;
    const r = undo(historyRef.current, design);
    historyRef.current = r.history;
    setDesign(r.design);
    bumpHistory();
    dispatch({ type: "EDIT" });
  };

  const doRedo = () => {
    if (!canRedo(historyRef.current)) return;
    const r = redo(historyRef.current, design);
    historyRef.current = r.history;
    setDesign(r.design);
    bumpHistory();
    dispatch({ type: "EDIT" });
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

  // Demo reset: pristine reference design, no record, back to design mode.
  const reset = async () => {
    setError(null);
    setRecord(null);
    setMode("design");
    historyRef.current = emptyHistory;
    bumpHistory();
    dispatch({ type: "RESET" });
    try {
      setDesign(await fetchReferenceDesign());
    } catch (e) {
      setError(String(e));
    }
  };

  const badge = STATE_BADGE[status.state];

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "z") return;
    e.preventDefault();
    if (e.shiftKey) doRedo();
    else doUndo();
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
        <button onClick={reset} title="Restore the reference design and clear results">
          Reset demo
        </button>
        <button onClick={doUndo} disabled={!canUndo(historyRef.current)} title="Undo (⌘Z)">
          Undo
        </button>
        <button onClick={doRedo} disabled={!canRedo(historyRef.current)} title="Redo (⇧⌘Z)">
          Redo
        </button>
        <button onClick={doUndo} disabled={!canUndo(historyRef.current)} title="Undo (⌘Z)">
          Undo
        </button>
        <button onClick={doRedo} disabled={!canRedo(historyRef.current)} title="Redo (⇧⌘Z)">
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

      {error && <div style={{ color: "#c74b3c", marginBottom: 12 }}>{error}</div>}

      {mode === "design" ? (
        <div style={{ display: "flex", gap: 32 }}>
          <Viewport design={design} />
          <Inspector design={design} onChange={edit} />
          <MotorSelector motors={motors} design={design} onChange={edit} />
        </div>
      ) : mode === "flight" ? (
        record && <FlightMode record={record} />
      ) : (
        <ReviewPanel />
      )}

      {record && (
        <footer style={{ marginTop: 20, fontSize: 11, color: "#9aa1ab" }}>
          run {record.summary.input_hash.slice(0, 12)} · deterministic · offline
          <EvidenceDrawer design={record.design} />
        </footer>
      )}
    </div>
  );
}
