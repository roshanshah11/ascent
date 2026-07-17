import { useEffect, useReducer, useState } from "react";
import FlightMode from "./components/FlightMode";
import Inspector from "./components/Inspector";
import MotorSelector from "./components/MotorSelector";
import ReviewPanel from "./components/ReviewPanel";
import Viewport from "./components/Viewport";
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

  useEffect(() => {
    fetchReferenceDesign().then(setDesign).catch((e) => setError(String(e)));
    fetchMotors().then(setMotors).catch((e) => setError(String(e)));
  }, []);

  if (!design) {
    return <div style={{ padding: 24 }}>{error ?? "Loading reference design…"}</div>;
  }

  const edit = (next: Design) => {
    setDesign(next);
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

  const badge = STATE_BADGE[status.state];

  return (
    <div style={{ padding: 20 }}>
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
        </footer>
      )}
    </div>
  );
}
