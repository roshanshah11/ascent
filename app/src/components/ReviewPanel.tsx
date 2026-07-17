// Flight Review panel (Day 7): constraint rows bound to the cited rule
// pack, mission target check, and the one-click repair with its diff.
import { useEffect, useState } from "react";
import { fetchReview, solveReview } from "../ipc";
import type { RepairResult, ReviewReport } from "../core/types";

function Row({
  label,
  measured,
  required,
  pass,
  citation,
}: {
  label: string;
  measured: string;
  required: string;
  pass: boolean;
  citation?: string;
}) {
  return (
    <tr style={{ color: pass ? "#3fb950" : "#c74b3c" }} title={citation}>
      <td>{pass ? "●" : "✕"}</td>
      <td style={{ paddingRight: 12 }}>{label}</td>
      <td style={{ paddingRight: 12 }}>{measured}</td>
      <td>{required}</td>
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

  if (!reportData) return <div>{error ?? "Evaluating design against IREC 2026…"}</div>;

  const shown = repairData ? repairData.review : reportData.review;
  const apogee = repairData ? repairData.achieved_apogee_m : reportData.review.apogee_m;
  const targetMet = Math.abs(apogee - target) <= reportData.target_tolerance_m;

  return (
    <div>
      <label>
        Target apogee (m):{" "}
        <input
          type="number"
          value={target}
          style={{ width: 80 }}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (Number.isFinite(v) && v > 0) setTarget(v);
          }}
        />
      </label>

      <table style={{ fontSize: 13, marginTop: 12, borderSpacing: "6px 3px" }}>
        <tbody>
          <Row
            label={`Target apogee ±${reportData.target_tolerance_m} m`}
            measured={`${apogee.toFixed(1)} m`}
            required={`${target} m`}
            pass={targetMet}
          />
          {shown.checks.map((c) => (
            <Row
              key={c.rule_id}
              label={c.description}
              measured={c.measured.toFixed(2)}
              required={`${c.comparator === "gte" ? "≥" : "≤"} ${c.required}`}
              pass={c.pass}
              citation={c.citation}
            />
          ))}
        </tbody>
      </table>
      <div style={{ fontSize: 11, color: "#9aa1ab", marginTop: 4 }}>
        hover a rule for its IREC 2026 citation
      </div>

      {!repairData && !(reportData.mission_feasible && targetMet) && (
        <button onClick={runSolver} disabled={busy} style={{ marginTop: 12 }}>
          {busy ? "Searching…" : "Find feasible configuration"}
        </button>
      )}

      {repairData && (
        <div style={{ marginTop: 12 }}>
          <h4 style={{ margin: "4px 0" }}>Repair applied</h4>
          <ul style={{ fontSize: 13 }}>
            {repairData.diff.map((d) => (
              <li key={d.field}>
                {d.field}: {d.before} → {d.after}
              </li>
            ))}
          </ul>
          <div style={{ fontSize: 13, color: "#3fb950" }}>
            apogee {repairData.achieved_apogee_m.toFixed(1)} m (target{" "}
            {repairData.target_apogee_m} m) · every rule green
          </div>
        </div>
      )}

      {error && <div style={{ color: "#c74b3c", marginTop: 8 }}>{error}</div>}
    </div>
  );
}
