import {
  ArrowRight,
  CheckCircle,
  CirclesThreePlus,
  Robot,
  ShieldCheck,
  Warning,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import type { CounterfactualReview, DocumentState } from "../core/types";
import { applyProposal, fetchCounterfactualReview } from "../ipc";

interface Props {
  onDocChange: (doc: DocumentState) => void;
  onApplied: () => void;
  onUniverseChange?: (universe: CounterfactualReview | null) => void;
}

const STARTER_COMMANDS = [
  { label: "Refine drag model", command: "set-sim-param cd 0.58" },
  { label: "Use a longer rail", command: "set-sim-param rail_length_m 1.5" },
  { label: "Select C6 motor", command: "select-motor C6" },
];

export default function CopilotPanel({ onDocChange, onApplied, onUniverseChange }: Props) {
  const [draft, setDraft] = useState(STARTER_COMMANDS[0].command);
  const [universe, setUniverse] = useState<CounterfactualReview | null>(null);
  const proposal = universe?.proposal ?? null;
  const [busy, setBusy] = useState<"preview" | "apply" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const lines = useMemo(
    () => draft.split("\n").map((line) => line.trim()).filter(Boolean),
    [draft],
  );

  const preview = async () => {
    if (lines.length === 0) return;
    setBusy("preview");
    setError(null);
    try {
      const next = await fetchCounterfactualReview(lines);
      setUniverse(next);
      onUniverseChange?.(next);
    } catch (e) {
      setUniverse(null);
      onUniverseChange?.(null);
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const apply = async () => {
    if (!proposal?.valid) return;
    setBusy("apply");
    setError(null);
    try {
      onDocChange(await applyProposal(lines));
      onApplied();
      setUniverse(null);
      onUniverseChange?.(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const diff = proposal?.diff;

  useEffect(() => {
    if (!proposal) return;
    const rejectOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setUniverse(null);
        onUniverseChange?.(null);
        setError(null);
      }
    };
    window.addEventListener("keydown", rejectOnEscape);
    return () => window.removeEventListener("keydown", rejectOnEscape);
  }, [proposal, onUniverseChange]);
  const changeCount = diff
    ? Number(diff.vehicle_changed) +
      Number(diff.design_changed) +
      Number(diff.atmosphere_changed) +
      diff.parts_added +
      diff.parts_removed +
      diff.studies_added +
      diff.studies_removed
    : 0;

  return (
    <div className="copilot-panel">
      <div className="copilot-intro">
        <div className="copilot-mark"><Robot size={20} weight="duotone" /></div>
        <div>
          <div className="copilot-title-row">
            <h3>AI Command Copilot</h3>
            <span className="agent-state"><span /> Agent bridge ready</span>
          </div>
          <p>
            Agents use the same journaled command layer as the GUI. Every batch is dry-run,
            checked for stale evidence, and requires your approval.
          </p>
        </div>
      </div>

      <div className="copilot-body">
        <div className="copilot-compose">
          <label htmlFor="copilot-command">Proposed command batch</label>
          <textarea
            id="copilot-command"
            value={draft}
            spellCheck={false}
            onChange={(event) => {
              setDraft(event.target.value);
              setUniverse(null);
              onUniverseChange?.(null);
            }}
            placeholder="set-part-param 3 span_m 0.05"
          />
          <div className="copilot-suggestions" aria-label="Suggested command batches">
            {STARTER_COMMANDS.map((starter) => (
              <button
                type="button"
                key={starter.label}
                onClick={() => {
                  setDraft(starter.command);
                  setUniverse(null);
                  onUniverseChange?.(null);
                }}
              >
                <CirclesThreePlus size={13} />
                {starter.label}
              </button>
            ))}
          </div>
        </div>

        <div className="proposal-review" aria-live="polite">
          {!proposal && !error && (
            <div className="proposal-empty">
              <ShieldCheck size={23} weight="duotone" />
              <span>Preview to inspect the exact diff and evidence impact.</span>
            </div>
          )}
          {error && <div className="proposal-error"><Warning size={16} />{error}</div>}
          {proposal && (
            <>
              <div className={`proposal-verdict ${proposal.valid ? "valid" : "invalid"}`}>
                {proposal.valid ? <CheckCircle size={17} weight="fill" /> : <Warning size={17} weight="fill" />}
                <strong>{proposal.valid ? "Validated" : "Revision required"}</strong>
                <span>{proposal.commands.length} command{proposal.commands.length === 1 ? "" : "s"}</span>
              </div>
              <div className="proposal-lines">
                {proposal.commands.map((command) => (
                  <div key={command.index} className={command.error ? "has-error" : ""}>
                    <code>{command.text}</code>
                    {command.error && <span>{command.error}</span>}
                  </div>
                ))}
              </div>
              {diff && (
                <div className="proposal-impact">
                  <span><strong>{changeCount}</strong> state changes</span>
                  <span className={diff.studies_made_stale.length > 0 ? "warning" : ""}>
                    <strong>{diff.studies_made_stale.length}</strong> studies made stale
                  </span>
                  <span><strong>1</strong> atomic journal entry</span>
                </div>
              )}
              {universe && (
                <div className="counterfactual-consequences" data-testid="counterfactual-universe">
                  <h4>Counterfactual review universe</h4>
                  <div className="counterfactual-grid">
                    <span>Apogee</span><code>{universe.baseline_run.summary.apogee_m.toFixed(2)} → {universe.proposed_run.summary.apogee_m.toFixed(2)} m</code>
                    <span>CG ignition</span><code>{universe.baseline_markers.cg_ignition_from_nose_m.toFixed(4)} → {universe.proposed_markers.cg_ignition_from_nose_m.toFixed(4)} m</code>
                    <span>CP</span><code>{universe.baseline_markers.cp_from_nose_m.toFixed(4)} → {universe.proposed_markers.cp_from_nose_m.toFixed(4)} m</code>
                    <span>Stability</span><code>{universe.baseline_markers.stability_ignition_cal.toFixed(3)} → {universe.proposed_markers.stability_ignition_cal.toFixed(3)} cal</code>
                    <span>Evidence</span><code>{universe.baseline_evidence.input_hash.slice(0, 12)} → {universe.proposed_evidence.input_hash.slice(0, 12)}</code>
                    <span>Residual RMSE</span><code>{reconciliationRmse(universe.baseline_reconciliation)} → {reconciliationRmse(universe.proposed_reconciliation)}</code>
                  </div>
                  {universe.qualifications.map((qualification) => <p className="proposal-qualification" key={qualification}>{qualification}</p>)}
                  <details><summary>Proposed report preview</summary><iframe title="Proposed flight-readiness report" srcDoc={universe.report_preview_html} sandbox="" /></details>
                </div>
              )}
            </>
          )}
        </div>

        <div className="copilot-actions">
          {proposal && (
            <button type="button" className="button-secondary" onClick={() => { setUniverse(null); onUniverseChange?.(null); }} disabled={busy !== null}>
              Reject preview
            </button>
          )}
          <button type="button" className="button-secondary" onClick={() => void preview()} disabled={busy !== null || lines.length === 0}>
            {busy === "preview" ? "Checking…" : "Preview changes"}
          </button>
          <button type="button" className="button-primary" onClick={() => void apply()} disabled={!proposal?.valid || busy !== null}>
            {busy === "apply" ? "Applying…" : "Approve & apply"}
            <ArrowRight size={14} weight="bold" />
          </button>
        </div>
      </div>
    </div>
  );
}

function reconciliationRmse(result: CounterfactualReview["baseline_reconciliation"]): string {
  const value = result?.channels[0]?.whole_flight.rmse;
  return value === undefined ? "qualified unavailable" : value.toFixed(4);
}
