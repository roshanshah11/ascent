// Monte Carlo dispersion panel (v0.2 Step 6). Thin, disposable skin over
// the run_dispersion IPC — the real UI arrives with the redesign pass.
import { lazy, Suspense, useState } from "react";
import type { Design, DispersionSummary } from "../core/types";
import { runDispersion } from "../ipc";
import LazyPanelBoundary from "./LazyPanelBoundary";

const EnsembleViewport = lazy(() => import("./EnsembleViewport"));

const DEFAULT_REQUEST = {
  seed: 42,
  samples: 200,
  vary: [
    { param: "thrust_pct" as const, sigma: 3.0 },
    { param: "cd_pct" as const, sigma: 5.0 },
    { param: "wind_speed_ms" as const, sigma: 1.5 },
    { param: "launch_angle_deg" as const, sigma: 2.0 },
    { param: "mass_g" as const, sigma: 1.0 },
  ],
  base_wind_ms: 3.0,
};

export default function DispersionView({ design }: { design: Design }) {
  const [summary, setSummary] = useState<DispersionSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      setSummary(await runDispersion(design, DEFAULT_REQUEST));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <details className="dispersion-panel">
      <summary>Dispersion envelope</summary>
      <div className="dispersion-body">
        <div className="dispersion-setup">
          {DEFAULT_REQUEST.samples} flights · ±3% thrust · ±5% Cd · ±1.5 m/s
          wind · ±2° rail · ±1 g mass · 3 m/s mean wind · seed{" "}
          {DEFAULT_REQUEST.seed}
        </div>
        <button className="button-secondary" onClick={run} disabled={busy}>
          {busy ? "Flying the fleet…" : "Run dispersion"}
        </button>
        {error && <div className="inline-error">{error}</div>}
        {summary && (
          <>
            <div className="dispersion-metric">
              <b>Apogee</b> p5 {summary.apogee_p5_m.toFixed(1)} m · p50{" "}
              {summary.apogee_p50_m.toFixed(1)} m · p95{" "}
              {summary.apogee_p95_m.toFixed(1)} m
            </div>
            <div className="dispersion-metric">
              <b>Landing</b> mean {summary.landing_mean_m.toFixed(1)} m
              downrange · ±2σ span {summary.landing_ellipse.a_m.toFixed(1)} m
              along the wind axis
            </div>
            <div className="dispersion-note">
              Deterministic: seed {summary.seed} reproduces these{" "}
              {summary.runs.length} flights exactly. Crossrange spread arrives
              with the 3D wind model.
            </div>
            <div className="dispersion-viewport">
              <LazyPanelBoundary message="Unable to load the ensemble viewport.">
                <Suspense
                  fallback={<div className="viewport-loading">Loading ensemble viewport…</div>}
                >
                  <EnsembleViewport summary={summary} />
                </Suspense>
              </LazyPanelBoundary>
            </div>
            <div className="dispersion-note">
              Blue arcs are schematic (launch→apogee→landing) from each
              member's apogee and range, not re-integrated paths. Amber points
              are landings; the red loop is the ±2σ landing ellipse; green
              rings mark p5/p50/p95 apogee.
            </div>
          </>
        )}
      </div>
    </details>
  );
}
