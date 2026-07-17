// Evidence drawer (Day 8): the provenance behind a run — hash, models,
// assumptions, motor certification, and live timestep-convergence status.
import { useEffect, useState } from "react";
import { fetchEvidence } from "../ipc";
import type { Design, EvidenceReport } from "../core/types";

export default function EvidenceDrawer({ design }: { design: Design }) {
  const [evidence, setEvidence] = useState<EvidenceReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!open) return;
    setEvidence(null);
    setError(null);
    fetchEvidence(design).then(setEvidence).catch((e) => setError(String(e)));
  }, [open, design]);

  return (
    <details
      open={open}
      onToggle={(e) => setOpen((e.target as HTMLDetailsElement).open)}
      style={{ marginTop: 16, fontSize: 12, color: "#9aa1ab" }}
    >
      <summary style={{ cursor: "pointer" }}>Evidence — why trust this number?</summary>
      {error && <div style={{ color: "#c74b3c", marginTop: 8 }}>{error}</div>}
      {!evidence && !error && open && <div style={{ marginTop: 8 }}>Computing evidence…</div>}
      {evidence && (
        <div style={{ marginTop: 8, display: "grid", gap: 8, maxWidth: 560 }}>
          <div>
            <b>Input hash</b>{" "}
            <code style={{ fontSize: 11 }}>{evidence.input_hash}</code>
          </div>
          <div>
            <b>Engine</b> {evidence.engine.id} {evidence.engine.version}
          </div>
          <div>
            <b>Convergence</b>{" "}
            {evidence.convergence.converged ? (
              <span style={{ color: "#3fb950" }}>
                converged — halving dt moves apogee{" "}
                {evidence.convergence.apogee_delta_m.toExponential(2)} m
              </span>
            ) : (
              <span style={{ color: "#c74b3c" }}>NOT converged at dt {evidence.convergence.dt_s} s</span>
            )}
          </div>
          <div>
            <b>Motor</b> {evidence.motor.manufacturer} {evidence.motor.designation} —{" "}
            <code style={{ fontSize: 11 }}>
              {JSON.stringify(evidence.motor.provenance)}
            </code>
          </div>
          <div>
            <b>Models</b>
            <ul style={{ margin: "4px 0 0 18px" }}>
              {evidence.models.map((m) => (
                <li key={m}>{m}</li>
              ))}
            </ul>
          </div>
          <div>
            <b>Assumptions</b>
            <ul style={{ margin: "4px 0 0 18px" }}>
              {evidence.assumptions.map((a) => (
                <li key={a}>{a}</li>
              ))}
            </ul>
          </div>
          <div>
            <b>External validation</b> {evidence.validation}
          </div>
        </div>
      )}
    </details>
  );
}
