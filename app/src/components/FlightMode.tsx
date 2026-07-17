// Flight mode (Day 6): playback + plots + timeline + cards + warnings.
// Rendered exclusively from a stored RunRecord — no sim calls in here.
import { useEffect, useRef, useState } from "react";
import {
  deriveWarnings,
  flightDuration,
  sampleAt,
  timeline,
} from "../core/playback";
import type { RunRecord } from "../core/types";

function Plot({
  record,
  field,
  label,
  color,
  cursorT,
}: {
  record: RunRecord;
  field: "altitude_m" | "velocity_ms";
  label: string;
  color: string;
  cursorT: number;
}) {
  const w = 420;
  const h = 140;
  const pad = 6;
  const duration = flightDuration(record);
  const values = record.samples.map((s) => s[field]);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const pts = record.samples
    .map(
      (s) =>
        `${pad + (s.t / duration) * (w - 2 * pad)},${
          h - pad - ((s[field] - min) / span) * (h - 2 * pad)
        }`,
    )
    .join(" ");
  const cursor = sampleAt(record, cursorT);
  const cx = pad + (cursorT / duration) * (w - 2 * pad);
  const cy = h - pad - ((cursor[field] - min) / span) * (h - 2 * pad);
  return (
    <div>
      <div style={{ fontSize: 12, color: "#9aa1ab" }}>
        {label}: {cursor[field].toFixed(1)}
      </div>
      <svg width={w} height={h} style={{ background: "#161b22", borderRadius: 4 }}>
        <polyline points={pts} fill="none" stroke={color} strokeWidth="1.5" />
        <circle cx={cx} cy={cy} r={4} fill={color} />
      </svg>
    </div>
  );
}

export default function FlightMode({ record }: { record: RunRecord }) {
  const [t, setT] = useState(0);
  const [playing, setPlaying] = useState(false);
  const raf = useRef<number>();
  const duration = flightDuration(record);

  useEffect(() => {
    if (!playing) return;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = (now - last) / 1000;
      last = now;
      setT((prev) => {
        // 8x time compression so a 100 s flight plays in ~12 s.
        const next = prev + dt * 8;
        if (next >= duration) {
          setPlaying(false);
          return duration;
        }
        return next;
      });
      raf.current = requestAnimationFrame(tick);
    };
    raf.current = requestAnimationFrame(tick);
    return () => {
      if (raf.current) cancelAnimationFrame(raf.current);
    };
  }, [playing, duration]);

  const now = sampleAt(record, t);
  const apogee = record.summary.apogee_m;
  const warnings = deriveWarnings(record);
  const altFrac = apogee > 0 ? now.altitude_m / apogee : 0;

  return (
    <div style={{ display: "flex", gap: 24 }}>
      {/* animated launch column */}
      <div>
        <svg width="120" height="360" style={{ background: "#161b22", borderRadius: 4 }}>
          <line x1="10" y1="340" x2="110" y2="340" stroke="#4a5361" />
          <g transform={`translate(60, ${340 - altFrac * 310})`}>
            <polygon points="0,-14 -5,0 5,0" fill="#c74b3c" />
            <rect x="-5" y="0" width="10" height="18" fill="#d8dbe0" />
          </g>
        </svg>
        <div style={{ marginTop: 8 }}>
          <button
            onClick={() => {
              if (t >= duration) setT(0);
              setPlaying(!playing);
            }}
          >
            {playing ? "Pause" : t >= duration ? "Replay" : "Launch"}
          </button>
          <input
            type="range"
            min={0}
            max={duration}
            step={duration / 1000}
            value={t}
            style={{ width: 110 }}
            onChange={(e) => setT(Number(e.target.value))}
          />
          <div style={{ fontSize: 12, color: "#9aa1ab" }}>t = {t.toFixed(2)} s</div>
        </div>
      </div>

      <div>
        {/* summary cards */}
        <div style={{ display: "flex", gap: 12, marginBottom: 12 }}>
          {[
            ["Apogee", `${record.summary.apogee_m.toFixed(1)} m`],
            ["Max velocity", `${record.summary.max_velocity_ms.toFixed(1)} m/s`],
            ["Rail exit", `${record.summary.rail_exit_velocity_ms.toFixed(1)} m/s`],
            ["Landing", `${Math.abs(record.summary.landing_velocity_ms).toFixed(1)} m/s`],
          ].map(([k, v]) => (
            <div
              key={k}
              style={{ background: "#161b22", padding: "8px 14px", borderRadius: 4 }}
            >
              <div style={{ fontSize: 11, color: "#9aa1ab" }}>{k}</div>
              <div style={{ fontSize: 18 }}>{v}</div>
            </div>
          ))}
        </div>

        {warnings.length > 0 && (
          <ul style={{ color: "#e8a33d", fontSize: 13 }}>
            {warnings.map((w) => (
              <li key={w.id}>{w.message}</li>
            ))}
          </ul>
        )}

        <Plot record={record} field="altitude_m" label="Altitude (m)" color="#58a6ff" cursorT={t} />
        <Plot record={record} field="velocity_ms" label="Velocity (m/s)" color="#3fb950" cursorT={t} />

        {/* event timeline */}
        <table style={{ fontSize: 12, marginTop: 10, borderSpacing: "10px 2px" }}>
          <tbody>
            {timeline(record).map((e) => (
              <tr key={e.kind} style={{ opacity: e.t <= t ? 1 : 0.35 }}>
                <td>{e.kind}</td>
                <td>{e.t.toFixed(2)} s</td>
                <td>{e.altitude_m.toFixed(1)} m</td>
                <td>{e.velocity_ms.toFixed(1)} m/s</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
