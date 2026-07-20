import {
  ArrowClockwise,
  ArrowCounterClockwise,
  ArrowsClockwise,
  BracketsCurly,
  CheckCircle,
  Crosshair,
  Cube,
  FrameCorners,
  Play,
  Robot,
  Rows,
  TerminalWindow,
  Warning,
} from "@phosphor-icons/react";
import { lazy, Suspense, useEffect, useReducer, useRef, useState } from "react";
import AtmospherePanel from "./components/AtmospherePanel";
import CommandPalette, { type PaletteAction } from "./components/CommandPalette";
import Console from "./components/Console";
import CopilotPanel from "./components/CopilotPanel";
import DispersionView from "./components/DispersionView";
import FlightMode from "./components/FlightMode";
import Inspector from "./components/Inspector";
import JobsPanel from "./components/JobsPanel";
import LazyPanelBoundary from "./components/LazyPanelBoundary";
import MotorSelector from "./components/MotorSelector";
import PartInspector from "./components/PartInspector";
import ProjectExplorer from "./components/ProjectExplorer";
import ResultsWorkspace from "./components/ResultsWorkspace";
import ReviewPanel from "./components/ReviewPanel";
import SpreadPanel from "./components/SpreadPanel";
import Viewport from "./components/Viewport";
import Workbench, { type Workspace } from "./components/Workbench";
import { diffDesign } from "./core/commandDiff";
import { initialRunStatus, reduceRunStatus } from "./core/runState";
import type {
  Command,
  CounterfactualReview,
  Design,
  DocumentState,
  MotorInfo,
  PartPreview,
  Project,
  RunRecord,
  SpreadResult,
  VehicleMarkers,
  VehiclePart,
} from "./core/types";
import {
  autosaveProject,
  checkRecovery,
  consoleExec,
  discardRecovery,
  dispatchCommand,
  previewPartParam,
  fetchCommandCatalogue,
  fetchMotors,
  fetchReferenceDesign,
  getDocument,
  getVehicleMarkers,
  importAtmosphere,
  redoDocument,
  runSimulation,
  runSpread,
  undoDocument,
} from "./ipc";
import type { GrammarCommand } from "./ipc";

const ViewportR3F = lazy(() => import("./components/ViewportR3F"));

const STATE_BADGE: Record<string, { label: string; tone: string }> = {
  current: { label: "Result current", tone: "ok" },
  dirty: { label: "Inputs modified", tone: "warn" },
  running: { label: "Solver running", tone: "active" },
  stale: { label: "Result stale", tone: "danger" },
};

type DockTab = "copilot" | "jobs" | "console";

function countParts(parts: VehiclePart[]): number {
  return parts.reduce((count, part) => count + 1 + countParts(part.children), 0);
}

export default function App() {
  const [doc, setDoc] = useState<DocumentState | null>(null);
  const [view3d, setView3d] = useState(false);
  const [vehicleMarkers, setVehicleMarkers] = useState<VehicleMarkers | null>(null);
  const [counterfactual, setCounterfactual] = useState<CounterfactualReview | null>(null);
  // Physics-in-the-loop drag: the read-only preview overrides the live
  // markers in the viewport while a part parameter is being dragged, so
  // CP/CG and stability move under the cursor. The committed value lands
  // through the normal dispatcher on release; `previewMarkers` clears then.
  const [partPreview, setPartPreview] = useState<PartPreview | null>(null);
  const [partPreviewing, setPartPreviewing] = useState(false);
  const [partPreviewError, setPartPreviewError] = useState<string | null>(null);
  const [previewMarkers, setPreviewMarkers] = useState<VehicleMarkers | null>(null);
  const [motors, setMotors] = useState<MotorInfo[]>([]);
  const [grammarCommands, setGrammarCommands] = useState<GrammarCommand[]>([]);
  const [record, setRecord] = useState<RunRecord | null>(null);
  const [spread, setSpread] = useState<SpreadResult | null>(null);
  const [status, dispatch] = useReducer(reduceRunStatus, initialRunStatus);
  const [mode, setMode] = useState<Workspace>("design");
  const [dockTab, setDockTab] = useState<DockTab>("copilot");
  const [selectedNode, setSelectedNode] = useState("vehicle");
  const [error, setError] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const comparisonGeneration = useRef(0);
  const [recovery, setRecovery] = useState<Project | null>(null);
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
    checkRecovery().then(setRecovery).catch(() => {});
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen(true);
        return;
      }
      if (!e.metaKey && !e.ctrlKey && !e.altKey && /^[1-4]$/.test(e.key)) {
        const target = e.target as HTMLElement | null;
        if (target?.matches("input, textarea, select, [contenteditable='true']")) return;
        const next = (["design", "simulate", "results", "review"] as Workspace[])[Number(e.key) - 1];
        if (next !== "simulate" || record) setMode(next);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [record]);

  useEffect(() => {
    const timer = setInterval(() => {
      const { design: currentDesign, record: currentRecord, dirty } = autosaveRef.current;
      if (!currentDesign || !dirty) return;
      const project: Project = {
        schema_version: 1,
        name: currentDesign.name,
        designs: [currentDesign],
        runs: currentRecord ? [currentRecord] : [],
      };
      autosaveProject(project).catch(() => {});
    }, 30_000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!doc) return;
    let cancelled = false;
    getVehicleMarkers()
      .then((markers) => {
        if (!cancelled) setVehicleMarkers(markers);
      })
      .catch(() => {
        if (!cancelled) setVehicleMarkers(null);
      });
    return () => {
      cancelled = true;
    };
  }, [doc]);

  if (!doc) {
    return (
      <div className="boot-screen">
        <div className="boot-mark"><FrameCorners size={28} weight="duotone" /></div>
        <div>
          <strong>ASCENT WORKBENCH</strong>
          <span>{error ?? "Opening engineering document…"}</span>
        </div>
      </div>
    );
  }

  const design = doc.design;
  const badge = STATE_BADGE[status.state];
  const partCount = countParts(doc.vehicle.parts);

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
    const commands = diffDesign(design, next);
    if (commands.length === 0) return;
    try {
      let state = doc;
      for (const command of commands) state = await dispatchCommand(command);
      markEdited(state);
    } catch (e) {
      setError(String(e));
    }
  };

  // Read-only preview during a part-parameter drag: never dispatches,
  // never journals. Feeds the viewport markers so they move while dragging.
  const onPartPreview = async (candidate: {
    id: number;
    param: string;
    value: number;
  }) => {
    setPartPreviewing(true);
    try {
      const preview = await previewPartParam(candidate.id, candidate.param, candidate.value);
      setPartPreview(preview);
      setPreviewMarkers(preview.markers);
      setPartPreviewError(null);
    } catch (e) {
      // Hold the last good preview; an out-of-range drag just doesn't update.
      setPartPreviewError(String(e));
    } finally {
      setPartPreviewing(false);
    }
  };

  // Release: exactly one journaled command lands through the dispatcher.
  // Clearing the preview lets the `doc` effect refresh the true markers.
  const onPartCommit = async (command: Command) => {
    try {
      markEdited(await dispatchCommand(command));
    } catch (e) {
      setError(String(e));
    } finally {
      setPartPreview(null);
      setPreviewMarkers(null);
      setPartPreviewError(null);
    }
  };

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
      if (generation === comparisonGeneration.current) {
        setSpread(result);
        setDockTab("jobs");
      }
    } catch (e) {
      if (generation === comparisonGeneration.current) setError(String(e));
    }
  };

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
        markEdited(await dispatchCommand({ cmd: "set_design", design: recovery.designs[0] }));
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

  const execFromPalette = async (line: string) => {
    markEdited(await consoleExec(line));
  };

  const importAtmosphereCsv = async (name: string, csv: string) => {
    setError(null);
    try {
      markEdited(await importAtmosphere(name, csv));
    } catch (e) {
      setError(String(e));
    }
  };

  const clearAtmosphere = async () => {
    setError(null);
    try {
      markEdited(await dispatchCommand({ cmd: "set_atmosphere", profile: null }));
    } catch (e) {
      setError(String(e));
    }
  };

  const paletteActions: PaletteAction[] = [
    { id: "goto-design", title: "Switch to Design", keywords: "workspace tab", run: () => setMode("design") },
    { id: "goto-simulate", title: "Switch to Simulate", keywords: "workspace tab flight", run: () => record && setMode("simulate") },
    { id: "goto-results", title: "Switch to Results", keywords: "workspace tab", run: () => setMode("results") },
    { id: "goto-review", title: "Switch to Verification", keywords: "workspace tab requirements mission assurance", run: () => setMode("review") },
    { id: "show-copilot", title: "Open AI Command Copilot", keywords: "agent proposal commands", run: () => { setMode("design"); setDockTab("copilot"); } },
    { id: "run-simulation", title: "Run simulation", run: () => void run() },
    { id: "compare-engines", title: "Compare engines", keywords: "rocketpy cross-validation", run: () => void compare() },
    { id: "reset-demo", title: "Reset demo", keywords: "restore reference design", run: () => void reset() },
    ...(doc.can_undo ? [{ id: "undo", title: "Undo", run: () => void doUndo() }] : []),
    ...(doc.can_redo ? [{ id: "redo", title: "Redo", run: () => void doRedo() }] : []),
  ];

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "z") return;
    e.preventDefault();
    if (e.shiftKey) void doRedo();
    else void doUndo();
  };

  const toolbar = (
    <>
      <span className={`run-state ${badge.tone}`}><span />{badge.label}</span>
      <span className="toolbar-divider" />
      <button className="toolbar-button" onClick={doUndo} disabled={!doc.can_undo} title="Undo (⌘Z)">
        <ArrowCounterClockwise size={15} /><span>Undo</span>
      </button>
      <button className="toolbar-button" onClick={doRedo} disabled={!doc.can_redo} title="Redo (⇧⌘Z)">
        <ArrowClockwise size={15} /><span>Redo</span>
      </button>
      <button className="toolbar-button" onClick={() => setPaletteOpen(true)} title="Command palette (⌘K)">
        <BracketsCurly size={15} /><span>Commands</span><kbd>⌘K</kbd>
      </button>
      <button className="toolbar-button ai-entry" onClick={() => { setMode("design"); setDockTab("copilot"); }}>
        <Robot size={16} weight="duotone" /><span>AI Copilot</span>
      </button>
      <button className="run-button" onClick={run} disabled={status.state === "running"}>
        <Play size={14} weight="fill" /><span>Run simulation</span>
      </button>
      <button className="toolbar-overflow" onClick={compare} title="Run native and RocketPy cross-validation">
        <ArrowsClockwise size={15} /><span>Compare engines</span>
      </button>
      <button className="toolbar-overflow" onClick={reset} title="Restore the reference design and clear results">
        <span>Reset demo</span>
      </button>
    </>
  );

  const banner = (
    <>
      {recovery && (
        <div className="notice-bar warning">
          <Warning size={16} weight="fill" />
          <span>Autosave recovered from an unclean exit: <strong>{recovery.name}</strong></span>
          <button onClick={restoreRecovery}>Restore</button>
          <button onClick={dismissRecovery}>Discard</button>
        </div>
      )}
      {error && <div className="notice-bar danger"><Warning size={16} weight="fill" /><span>{error}</span></div>}
    </>
  );

  const statusbar = (
    <>
      <div className="status-group">
        <span className="status-item"><CheckCircle size={12} weight="fill" /> Local document</span>
        <span className="status-item">{partCount} components</span>
        <span className="status-item">{doc.studies.length} studies</span>
        {vehicleMarkers && <span className="status-item">Stability {vehicleMarkers.stability_ignition_cal.toFixed(2)} cal</span>}
      </div>
      <div className="status-group status-right">
        <span className="status-item">SI units</span>
        <span className="status-item">Deterministic</span>
        {record && <span className="status-item status-hash">{record.summary.input_hash.slice(0, 12)}</span>}
      </div>
    </>
  );

  return (
    <div tabIndex={-1} onKeyDown={onKeyDown}>
      <Workbench
        workspace={mode}
        onWorkspaceChange={setMode}
        disabledWorkspaces={record ? [] : ["simulate"]}
        title="Ascent"
        projectName={design.name}
        toolbar={toolbar}
        banner={recovery || error ? banner : undefined}
        statusbar={statusbar}
      >
        {mode === "design" ? (
          <div className="engineering-layout">
            <aside className="model-browser">
              <ProjectExplorer
                vehicle={doc.vehicle}
                studies={doc.studies}
                atmosphere={doc.atmosphere ?? null}
                selected={selectedNode}
                onSelect={setSelectedNode}
              />
            </aside>

            <section className="viewport-workspace" aria-label="Engineering canvas">
              <header className="panel-chrome">
                <div>
                  <span className="panel-eyebrow">Geometry</span>
                  <h2>Vehicle View</h2>
                </div>
                <div className="canvas-toolbar" role="toolbar" aria-label="Viewport controls">
                  <button type="button" title="Frame vehicle"><FrameCorners size={15} /></button>
                  <button type="button" title="Centerline"><Crosshair size={15} /></button>
                  <span className="toolbar-divider" />
                  <button type="button" className={!view3d ? "active" : ""} onClick={() => setView3d(false)} disabled={!view3d}>2D</button>
                  <button type="button" className={view3d ? "active" : ""} onClick={() => setView3d(true)} disabled={view3d}>3D</button>
                </div>
              </header>
              <div className="viewport-stage">
                <div className="viewport-axis-tag">Y+</div>
                <div className="viewport-mode-tag">{view3d ? "PERSPECTIVE" : "ORTHOGRAPHIC · SIDE"}</div>
                {counterfactual ? (
                  <div className="counterfactual-viewport" aria-label="Baseline and proposed geometry">
                    <div className="counterfactual-baseline"><Viewport design={design} markers={vehicleMarkers} vehicle={doc.vehicle} /></div>
                    <div className="counterfactual-proposed"><Viewport design={counterfactual.proposed_state.design} markers={counterfactual.proposed_markers} vehicle={counterfactual.proposed_state.vehicle} /></div>
                    <span className="counterfactual-legend">BASELINE · PROPOSAL</span>
                  </div>
                ) : view3d ? (
                  <LazyPanelBoundary message="Unable to load the 3D viewport.">
                    <Suspense fallback={<div className="viewport-loading">Loading 3D viewport…</div>}>
                      <ViewportR3F vehicle={doc.vehicle} markers={previewMarkers ?? vehicleMarkers} />
                    </Suspense>
                  </LazyPanelBoundary>
                ) : (
                  <Viewport design={design} markers={previewMarkers ?? vehicleMarkers} vehicle={doc.vehicle} />
                )}
                <div className="viewport-readout">
                  <div><span>DRY MASS</span><strong>{design.dry_mass_g.toFixed(1)} g</strong></div>
                  <div><span>DIAMETER</span><strong>{design.diameter_mm.toFixed(1)} mm</strong></div>
                  <div><span>DRAG CD</span><strong>{design.cd.toFixed(3)}</strong></div>
                  <div><span>MOTOR</span><strong>{design.motor_designation}</strong></div>
                </div>
              </div>
            </section>

            <aside className="properties-panel">
              <div className="panel-heading properties-heading">
                <div>
                  <span className="panel-eyebrow">Properties</span>
                  <h2>Flight Configuration</h2>
                </div>
                <Cube size={16} weight="duotone" />
              </div>
              <div className="properties-scroll">
                <Inspector design={design} onChange={edit} />
                <PartInspector
                  vehicle={doc.vehicle}
                  preview={partPreview}
                  previewing={partPreviewing}
                  error={partPreviewError}
                  onPreview={onPartPreview}
                  onCommit={onPartCommit}
                />
                <MotorSelector motors={motors} design={design} onChange={edit} />
                <AtmospherePanel
                  profile={doc.atmosphere ?? null}
                  onImport={importAtmosphereCsv}
                  onClear={clearAtmosphere}
                />
              </div>
            </aside>

            <section className="workbench-dock">
              <header className="dock-header">
                <nav className="dock-tabs" aria-label="Workbench dock">
                  <button type="button" className={dockTab === "copilot" ? "active" : ""} onClick={() => setDockTab("copilot")}>
                    <Robot size={14} weight="duotone" /> AI Copilot <span className="dock-live" />
                  </button>
                  <button type="button" className={dockTab === "jobs" ? "active" : ""} onClick={() => setDockTab("jobs")}>
                    <Rows size={14} /> Jobs <span className="dock-count">{doc.studies.length}</span>
                  </button>
                  <button type="button" className={dockTab === "console" ? "active" : ""} onClick={() => setDockTab("console")}>
                    <TerminalWindow size={14} /> Console
                  </button>
                </nav>
                <span className="dock-context">Selection: {selectedNode.replaceAll("-", " ")}</span>
              </header>
              <div className="dock-content">
                {dockTab === "copilot" && <CopilotPanel onDocChange={markEdited} onApplied={() => setError(null)} onUniverseChange={setCounterfactual} />}
                {dockTab === "jobs" && (
                  <>
                    <JobsPanel studies={doc.studies} onDocChange={setDoc} />
                    <DispersionView design={design} />
                    {spread && <div className="dock-comparison"><SpreadPanel spread={spread} /></div>}
                  </>
                )}
                {dockTab === "console" && <Console onDocChange={markEdited} />}
              </div>
            </section>
          </div>
        ) : mode === "simulate" ? (
          <div className="workspace-full simulate-surface">{record && <FlightMode record={record} counterfactual={counterfactual} />}</div>
        ) : mode === "results" ? (
          <div className="workspace-full results-surface"><ResultsWorkspace studies={doc.studies} record={record} /></div>
        ) : (
          <div className="workspace-full review-surface"><ReviewPanel /></div>
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
