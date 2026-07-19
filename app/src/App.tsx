import { lazy, Suspense, useEffect, useReducer, useRef, useState } from "react";
import CommandPalette, { type PaletteAction } from "./components/CommandPalette";
import Console from "./components/Console";
import DispersionView from "./components/DispersionView";
import EvidenceDrawer from "./components/EvidenceDrawer";
import FlightMode from "./components/FlightMode";
import Inspector from "./components/Inspector";
import JobsPanel from "./components/JobsPanel";
import MotorSelector from "./components/MotorSelector";
import ResultsWorkspace from "./components/ResultsWorkspace";
import ReviewPanel from "./components/ReviewPanel";
import SpreadPanel from "./components/SpreadPanel";
import Viewport from "./components/Viewport";
import Workbench, { type Workspace } from "./components/Workbench";
import { diffDesign } from "./core/commandDiff";
import { initialRunStatus, reduceRunStatus } from "./core/runState";
import type {
  Design,
  DocumentState,
  MotorInfo,
  Project,
  RunRecord,
  SpreadResult,
  VehicleMarkers,
} from "./core/types";
import {
  autosaveProject,
  checkRecovery,
  consoleExec,
  discardRecovery,
  dispatchCommand,
  fetchCommandCatalogue,
  fetchMotors,
  fetchReferenceDesign,
  getDocument,
  getVehicleMarkers,
  redoDocument,
  runSimulation,
  runSpread,
  undoDocument,
} from "./ipc";
import type { GrammarCommand } from "./ipc";

const ViewportR3F = lazy(() => import("./components/ViewportR3F"));

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
  const [vehicleMarkers, setVehicleMarkers] = useState<VehicleMarkers | null>(null);
  const [motors, setMotors] = useState<MotorInfo[]>([]);
  const [grammarCommands, setGrammarCommands] = useState<GrammarCommand[]>([]);
  const [record, setRecord] = useState<RunRecord | null>(null);
  const [spread, setSpread] = useState<SpreadResult | null>(null);
  const [status, dispatch] = useReducer(reduceRunStatus, initialRunStatus);
  const [mode, setMode] = useState<Workspace>("design");
  const [error, setError] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
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
    fetchCommandCatalogue().then(setGrammarCommands).catch((e) => setError(String(e)));
    fetchMotors().then(setMotors).catch((e) => setError(String(e)));
    // A surviving autosave means the last session crashed — offer a restore.
    checkRecovery().then(setRecovery).catch(() => {});
  }, []);

  // Keyboard-first command palette: Cmd+K / Ctrl+K opens from anywhere,
  // not just while the workbench root has focus (unlike Undo/Redo below).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen(true);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
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

  // CP/CG overlay data: re-query whenever the document snapshot changes.
  // Read-only — a failed derivation (mid-edit invalid tree) just hides the
  // markers rather than surfacing an error.
  useEffect(() => {
    if (!doc) return;
    let cancelled = false;
    getVehicleMarkers()
      .then((m) => {
        if (!cancelled) setVehicleMarkers(m);
      })
      .catch(() => {
        if (!cancelled) setVehicleMarkers(null);
      });
    return () => {
      cancelled = true;
    };
  }, [doc]);

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
      setMode("simulate");
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

  // Palette grammar dispatch: the one text-command mutation path, same as
  // the console — `console_exec` re-parses and re-validates in Rust.
  const execFromPalette = async (line: string) => {
    const next = await consoleExec(line);
    markEdited(next);
  };

  const paletteActions: PaletteAction[] = [
    { id: "goto-design", title: "Switch to Design", keywords: "workspace tab", run: () => setMode("design") },
    {
      id: "goto-simulate",
      title: "Switch to Simulate",
      keywords: "workspace tab flight",
      run: () => record && setMode("simulate"),
    },
    { id: "goto-results", title: "Switch to Results", keywords: "workspace tab", run: () => setMode("results") },
    { id: "goto-review", title: "Switch to Review", keywords: "workspace tab flight review", run: () => setMode("review") },
    { id: "run-simulation", title: "Run simulation", run: () => void run() },
    { id: "compare-engines", title: "Compare engines", keywords: "rocketpy cross-validation", run: () => void compare() },
    { id: "reset-demo", title: "Reset demo", keywords: "restore reference design", run: () => void reset() },
    ...(doc.can_undo ? [{ id: "undo", title: "Undo", run: () => void doUndo() }] : []),
    ...(doc.can_redo ? [{ id: "redo", title: "Redo", run: () => void doRedo() }] : []),
  ];

  const badge = STATE_BADGE[status.state];

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "z") return;
    e.preventDefault();
    if (e.shiftKey) void doRedo();
    else void doUndo();
  };

  const toolbar = (
    <>
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
      <button onClick={() => setPaletteOpen(true)} title="Command palette (⌘K)">
        ⌘K
      </button>
    </>
  );

  const banner = (
    <>
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
      {error && <div style={{ color: "#c74b3c" }}>{error}</div>}
    </>
  );

  return (
    <div tabIndex={-1} onKeyDown={onKeyDown}>
      <Workbench
        workspace={mode}
        onWorkspaceChange={setMode}
        disabledWorkspaces={record ? [] : ["simulate"]}
        title="Ascent"
        toolbar={toolbar}
        banner={recovery || error ? banner : undefined}
      >
        {mode === "design" ? (
          <>
            <section className="design-workspace">
              <div className="viewport-panel">
                {view3d ? (
                  <Suspense
                    fallback={
                      <div className="viewport-loading">Loading 3D viewport…</div>
                    }
                  >
                    <ViewportR3F vehicle={doc.vehicle} markers={vehicleMarkers} />
                  </Suspense>
                ) : (
                  <Viewport design={design} />
                )}
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
            </section>
            <JobsPanel studies={doc.studies} onDocChange={setDoc} />
            <DispersionView design={design} />
            <Console onDocChange={markEdited} />
          </>
        ) : mode === "simulate" ? (
          record && <FlightMode record={record} />
        ) : mode === "results" ? (
          <ResultsWorkspace studies={doc.studies} record={record} />
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
      </Workbench>
      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        actions={paletteActions}
        grammarCommands={grammarCommands}
        onExec={execFromPalette}
      />
    </div>
  );
}
