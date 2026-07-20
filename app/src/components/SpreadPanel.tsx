import type { RocketPySpread, SpreadResult } from "../core/types";

export function describeRocketPy(rocketpy: RocketPySpread): string {
  if (!rocketpy.available) {
    return `RocketPy unavailable: ${rocketpy.reason ?? "no reason supplied"}`;
  }
  return `RocketPy ${rocketpy.engine_version}`;
}

export default function SpreadPanel({ spread }: { spread: SpreadResult }) {
  const rocketpy = spread.rocketpy;
  return (
    <section className="spread-panel" aria-label="Cross-validation comparison">
      <header><span className="panel-eyebrow">Solver comparison</span><h3>Cross-validation</h3><small>Side-by-side only · never averaged</small></header>
      <div className="spread-metric"><span>Ascent native</span><strong>{spread.native.apogee_m.toFixed(1)} m</strong></div>
      {rocketpy.available && rocketpy.summary && spread.apogee_spread_m !== null ? (
        <>
          <div className="spread-metric"><span>RocketPy</span><strong>{rocketpy.summary.apogee_m.toFixed(1)} m</strong></div>
          <div className="spread-metric delta"><span>Apogee delta:</span><strong>{` ${spread.apogee_spread_m.toFixed(1)} m`}</strong></div>
        </>
      ) : (
        <div className="spread-unavailable">{describeRocketPy(rocketpy)}</div>
      )}
    </section>
  );
}
