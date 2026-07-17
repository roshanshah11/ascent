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
    <section
      style={{ marginTop: 16, padding: 12, background: "#161b22", borderRadius: 4, maxWidth: 520 }}
      aria-label="Cross-validation comparison"
    >
      <div style={{ fontSize: 12, color: "#9aa1ab", marginBottom: 6 }}>
        Cross-validation — side-by-side only, never averaged
      </div>
      <div>Ascent native apogee: {spread.native.apogee_m.toFixed(1)} m</div>
      {rocketpy.available && rocketpy.summary && spread.apogee_spread_m !== null ? (
        <>
          <div>RocketPy apogee: {rocketpy.summary.apogee_m.toFixed(1)} m</div>
          <div>Apogee delta: {spread.apogee_spread_m.toFixed(1)} m</div>
        </>
      ) : (
        <div style={{ color: "#e8a33d" }}>{describeRocketPy(rocketpy)}</div>
      )}
    </section>
  );
}
