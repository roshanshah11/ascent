import { CheckCircle, Warning } from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import type { Design, EvidenceReport } from "../core/types";
import { fetchEvidence } from "../ipc";

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
    <details className="evidence-drawer" open={open} onToggle={(event) => setOpen((event.target as HTMLDetailsElement).open)}>
      <summary><span>Evidence & credibility</span><small>Why trust this result?</small></summary>
      {error && <div className="inline-error"><Warning size={14} weight="fill" />{error}</div>}
      {!evidence && !error && open && <div className="evidence-loading">Computing evidence package…</div>}
      {evidence && (
        <div className="evidence-grid">
          <section className="evidence-facts">
            <div><span>Input hash</span><code>{evidence.input_hash}</code></div>
            <div><span>Engine</span><strong>{evidence.engine.id} {evidence.engine.version}</strong></div>
            <div><span>Motor source</span><strong>{evidence.motor.manufacturer} {evidence.motor.designation}</strong></div>
            <div><span>External validation</span><strong>{evidence.validation}</strong></div>
          </section>

          <section className="convergence-result">
            {evidence.convergence.converged ? <CheckCircle size={20} weight="fill" /> : <Warning size={20} weight="fill" />}
            <div>
              <span>Numerical convergence</span>
              <strong>{evidence.convergence.converged ? "CONVERGED" : "NOT CONVERGED"}</strong>
              <small>Halving dt moves apogee {evidence.convergence.apogee_delta_m.toExponential(2)} m</small>
            </div>
          </section>

          <section className="evidence-list"><h3>Models</h3><ul>{evidence.models.map((model) => <li key={model}>{model}</li>)}</ul></section>
          <section className="evidence-list"><h3>Assumptions</h3><ul>{evidence.assumptions.map((assumption) => <li key={assumption}>{assumption}</li>)}</ul></section>

          <section className="validation-ladder">
            <h3>Named validation evidence ladder</h3>
            {evidence.validation_cases.map((validationCase) => (
              <article key={validationCase.case_id} data-evidence-level={validationCase.evidence_level}>
                <header><strong>{validationCase.title}</strong><span>{validationCase.evidence_level.replaceAll("_", " ")}</span></header>
                <p>{validationCase.intended_use}</p>
                <small>Validity: {validationCase.validity_domain.join("; ")}</small>
                {[...validationCase.caveats, ...validationCase.known_mismatches].map((caveat) => <div className="validation-caveat" key={caveat}><Warning size={12} />{caveat}</div>)}
                <code>{validationCase.case_hash.slice(0, 16)}</code>
              </article>
            ))}
          </section>

          <section className="credibility-panel">
            <header><div><span className="panel-eyebrow">NASA-STD-7009-inspired</span><h3>Credibility scorecard</h3></div><small>Evidence summary · not a certification</small></header>
            <table><thead><tr><th>Factor</th><th>Score</th><th>Basis</th></tr></thead><tbody>{evidence.credibility.factors.map((factor) => <tr key={factor.name}><td>{factor.name}</td><td>{factor.score}/4</td><td>{factor.basis}</td></tr>)}</tbody></table>
            <div className="regime-tags">
              {evidence.credibility.quantities.map((quantity) => (
                <span key={quantity.quantity} className={quantity.regime.kind} title={quantity.regime.kind === "extrapolated" ? quantity.regime.reason : "Within the evidence-backed regime"}>
                  {quantity.quantity} · {quantity.regime.kind}
                </span>
              ))}
            </div>
          </section>
        </div>
      )}
    </details>
  );
}
