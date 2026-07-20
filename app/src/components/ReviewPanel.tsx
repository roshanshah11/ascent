import { CheckCircle, ShieldCheck, Warning, Wrench, XCircle } from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import type { RepairResult, ReviewReport } from "../core/types";
import { fetchReview, solveReview } from "../ipc";

function Row({
  label,
  measured,
  required,
  pass,
  citation,
  category,
}: {
  label: string;
  measured: string;
  required: string;
  pass: boolean;
  citation?: string;
  category: string;
}) {
  return (
    <tr className={pass ? "review-pass" : "review-fail"} title={citation}>
      <td>{pass ? <CheckCircle size={15} weight="fill" /> : <XCircle size={15} weight="fill" />}</td>
      <td><span>{label}</span><small>{category}</small></td>
      <td>{measured}</td>
      <td>{required}</td>
      <td>{pass ? "PASS" : "ACTION"}</td>
    </tr>
  );
}

export default function ReviewPanel() {
  const [target, setTarget] = useState(350);
  const [reportData, setReportData] = useState<ReviewReport | null>(null);
  const [repairData, setRepairData] = useState<RepairResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setRepairData(null);
    fetchReview(target).then(setReportData).catch((e) => setError(String(e)));
  }, [target]);

  const runSolver = async () => {
    setBusy(true);
    setError(null);
    try {
      setRepairData(await solveReview(target));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!reportData) {
    return (
      <div className="review-loading">
        <ShieldCheck size={24} weight="duotone" />
        <span>{error ?? "Evaluating active requirements and engineering margins…"}</span>
      </div>
    );
  }

  const shown = repairData ? repairData.review : reportData.review;
  const apogee = repairData ? repairData.achieved_apogee_m : reportData.review.apogee_m;
  const targetMet = Math.abs(apogee - target) <= reportData.target_tolerance_m;
  const rulePasses = shown.checks.filter((check) => check.pass).length;
  const structuralPasses = shown.structural_checks.filter((check) => check.pass).length;
  const allPass = reportData.mission_feasible && targetMet;

  return (
    <div className="review-workspace">
      <header className="review-header">
        <div className={`review-verdict ${allPass ? "ready" : "attention"}`}>
          {allPass ? <ShieldCheck size={30} weight="fill" /> : <Warning size={30} weight="fill" />}
          <div><span>Mission assurance</span><strong>{allPass ? "READY FOR REVIEW" : "ACTION REQUIRED"}</strong></div>
        </div>
        <div className="review-target">
          <label htmlFor="target-apogee">Target apogee</label>
          <div><input id="target-apogee" type="number" value={target} onChange={(event) => { const value = Number(event.target.value); if (Number.isFinite(value) && value > 0) setTarget(value); }} /><span>m AGL</span></div>
        </div>
        <div className="review-score"><span>Active requirements</span><strong>{rulePasses}/{shown.checks.length}</strong><small>passing</small></div>
        <div className="review-score"><span>Structural checks</span><strong>{structuralPasses}/{shown.structural_checks.length}</strong><small>passing</small></div>
      </header>

      <section className="review-checks">
        <div className="review-section-heading">
          <div><span className="panel-eyebrow">Verification matrix</span><h2>Flight Readiness Checks</h2></div>
          <span>Hover any check for its governing source</span>
        </div>
        <table>
          <thead><tr><th>Status</th><th>Requirement</th><th>Measured</th><th>Threshold</th><th>Disposition</th></tr></thead>
          <tbody>
            <Row label={`Target apogee ±${reportData.target_tolerance_m} m`} measured={`${apogee.toFixed(1)} m`} required={`${target} m`} pass={targetMet} category="Mission objective" />
            {shown.checks.map((check) => (
              <Row key={check.rule_id} label={check.description} measured={check.measured.toFixed(2)} required={`${check.comparator === "gte" ? "≥" : "≤"} ${check.required}`} pass={check.pass} citation={check.citation} category={check.rule_id} />
            ))}
            {shown.structural_checks.map((check) => (
              <Row key={check.check_id} label={check.label} measured={`${check.value.toFixed(2)} ${check.units}`} required={`≥ ${check.limit.toFixed(2)} ${check.units}`} pass={check.pass} citation={check.source} category={`Structural · margin ${check.margin.toFixed(2)} ${check.units}`} />
            ))}
          </tbody>
        </table>
      </section>

      <aside className="review-actions">
        <div className="review-section-heading"><div><span className="panel-eyebrow">Deterministic repair</span><h2>Configuration Solver</h2></div></div>
        {!repairData && !(reportData.mission_feasible && targetMet) && (
          <div className="repair-empty">
            <Wrench size={25} weight="duotone" />
            <p>Search the bounded design space for the smallest configuration change that satisfies the target and every active rule.</p>
            <button className="button-primary" onClick={runSolver} disabled={busy}>{busy ? "Searching…" : "Find feasible configuration"}</button>
          </div>
        )}
        {!repairData && reportData.mission_feasible && targetMet && (
          <div className="repair-empty ready"><CheckCircle size={25} weight="fill" /><p>No repair is required for the current target.</p></div>
        )}
        {repairData && (
          <div className="repair-result">
            <div className="repair-result-heading"><CheckCircle size={18} weight="fill" /><div><strong>Feasible configuration found</strong><span>Achieved {repairData.achieved_apogee_m.toFixed(1)} m against {repairData.target_apogee_m} m target</span></div></div>
            <table><thead><tr><th>Parameter</th><th>Before</th><th>After</th></tr></thead><tbody>{repairData.diff.map((change) => <tr key={change.field}><td>{change.field}</td><td>{change.before}</td><td>{change.after}</td></tr>)}</tbody></table>
          </div>
        )}
        {error && <div className="inline-error"><Warning size={14} weight="fill" />{error}</div>}
      </aside>
    </div>
  );
}
