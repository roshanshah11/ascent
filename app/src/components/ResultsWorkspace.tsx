// Post-processing workspace (v0.3 Step 7): a top-level mode, not a
// footer. All numbers come from core/plots.ts (pure, tested); this file
// only draws SVG and wires selection. CSV export serializes the exact
// series being rendered.
import { useMemo, useState } from "react";
import {
  dispersionOverlay,
  niceTicks,
  percentileSummaries,
  plotBounds,
  toCsv,
  trajectoryPlot,
  type PlotData,
} from "../core/plots";
import type { RunRecord, Study } from "../core/types";

const W = 560;
const H = 340;
const PAD = { left: 56, right: 16, top: 16, bottom: 40 };
const SERIES_COLORS = ["#58a6ff", "#e8a33d", "#3fb950", "#c74b3c", "#b083f0", "#9aa1ab"];

function Plot({ plot, scatter }: { plot: PlotData; scatter: boolean }) {
  const bounds = plotBounds(plot);
  if (!bounds) {
    return <div style={{ color: "#9aa1ab", fontSize: 12 }}>Nothing to plot yet.</div>;
  }
  // A degenerate axis (single x or flat y) still needs nonzero span.
  const spanX = bounds.maxX - bounds.minX || 1;
  const spanY = bounds.maxY - bounds.minY || 1;
  const sx = (x: number) =>
    PAD.left + ((x - bounds.minX) / spanX) * (W - PAD.left - PAD.right);
  const sy = (y: number) =>
    H - PAD.bottom - ((y - bounds.minY) / spanY) * (H - PAD.top - PAD.bottom);
  const xTicks = niceTicks(bounds.minX, bounds.maxX);
  const yTicks = niceTicks(bounds.minY, bounds.maxY);

  return (
    <svg width={W} height={H} role="img" aria-label={plot.title}>
      {xTicks.map((t) => (
        <g key={`x${t}`}>
          <line x1={sx(t)} y1={PAD.top} x2={sx(t)} y2={H - PAD.bottom} stroke="#21262d" />
          <text x={sx(t)} y={H - PAD.bottom + 16} fill="#9aa1ab" fontSize={10} textAnchor="middle">
            {t}
          </text>
        </g>
      ))}
      {yTicks.map((t) => (
        <g key={`y${t}`}>
          <line x1={PAD.left} y1={sy(t)} x2={W - PAD.right} y2={sy(t)} stroke="#21262d" />
          <text x={PAD.left - 6} y={sy(t) + 3} fill="#9aa1ab" fontSize={10} textAnchor="end">
            {t}
          </text>
        </g>
      ))}
      <text x={(PAD.left + W - PAD.right) / 2} y={H - 6} fill="#9aa1ab" fontSize={11} textAnchor="middle">
        {plot.xLabel}
      </text>
      <text
        x={12}
        y={(PAD.top + H - PAD.bottom) / 2}
        fill="#9aa1ab"
        fontSize={11}
        textAnchor="middle"
        transform={`rotate(-90 12 ${(PAD.top + H - PAD.bottom) / 2})`}
      >
        {plot.yLabel}
      </text>
      {plot.series.map((s, i) => {
        const color = SERIES_COLORS[i % SERIES_COLORS.length];
        return scatter ? (
          <g key={s.label}>
            {s.points.map((p, j) => (
              <circle key={j} cx={sx(p.x)} cy={sy(p.y)} r={2.5} fill={color} fillOpacity={0.75} />
            ))}
          </g>
        ) : (
          <polyline
            key={s.label}
            fill="none"
            stroke={color}
            strokeWidth={1.5}
            points={s.points.map((p) => `${sx(p.x)},${sy(p.y)}`).join(" ")}
          />
        );
      })}
      {/* Legend */}
      {plot.series.map((s, i) => (
        <g key={`legend-${s.label}`}>
          <rect
            x={PAD.left + 8}
            y={PAD.top + 6 + i * 16}
            width={10}
            height={10}
            fill={SERIES_COLORS[i % SERIES_COLORS.length]}
          />
          <text x={PAD.left + 22} y={PAD.top + 15 + i * 16} fill="#d8dbe0" fontSize={11}>
            {s.label}
          </text>
        </g>
      ))}
    </svg>
  );
}

function downloadCsv(name: string, csv: string) {
  const url = URL.createObjectURL(new Blob([csv], { type: "text/csv" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

export default function ResultsWorkspace({
  studies,
  record,
}: {
  studies: Study[];
  record: RunRecord | null;
}) {
  const [tab, setTab] = useState<"dispersion" | "trajectory">("dispersion");
  // Overlay selection: all completed dispersion studies start selected.
  const [deselected, setDeselected] = useState<Set<number>>(new Set());
  const selectable = studies.filter(
    (s) => s.kind.kind === "dispersion" && s.results !== undefined,
  );
  const selected = selectable.filter((s) => !deselected.has(s.id));

  const plot = useMemo(() => {
    if (tab === "trajectory") {
      return record ? trajectoryPlot(record) : null;
    }
    return dispersionOverlay(selected);
  }, [tab, record, studies, deselected]); // eslint-disable-line react-hooks/exhaustive-deps

  const cards = percentileSummaries(selected);

  const toggle = (id: number) => {
    setDeselected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <div style={{ display: "flex", gap: 24 }}>
      <div>
        <nav style={{ marginBottom: 8 }}>
          <button onClick={() => setTab("dispersion")} disabled={tab === "dispersion"}>
            Dispersion
          </button>
          <button
            onClick={() => setTab("trajectory")}
            disabled={tab === "trajectory" || !record}
            title={record ? "" : "Run a simulation first"}
          >
            Trajectory
          </button>
          {plot && plot.series.length > 0 && (
            <button
              style={{ marginLeft: 12 }}
              onClick={() => downloadCsv(`${plot.title.toLowerCase()}.csv`, toCsv(plot))}
            >
              Export CSV
            </button>
          )}
        </nav>
        {plot ? (
          <Plot plot={plot} scatter={tab === "dispersion"} />
        ) : (
          <div style={{ color: "#9aa1ab", fontSize: 12 }}>Run a simulation to see its trajectory.</div>
        )}
      </div>

      <aside style={{ minWidth: 220 }}>
        <h4 style={{ margin: "4px 0" }}>Studies</h4>
        {selectable.length === 0 && (
          <div style={{ color: "#9aa1ab", fontSize: 12 }}>
            No completed dispersion studies yet — run one from the Design workspace.
          </div>
        )}
        <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
          {selectable.map((s) => (
            <li key={s.id} style={{ marginBottom: 4 }}>
              <label style={{ fontSize: 13 }}>
                <input
                  type="checkbox"
                  checked={!deselected.has(s.id)}
                  onChange={() => toggle(s.id)}
                />{" "}
                {s.name}
              </label>
            </li>
          ))}
        </ul>
        {cards.map((c) => (
          <div
            key={c.label}
            style={{
              border: "1px solid #30363d",
              borderRadius: 6,
              padding: "6px 10px",
              marginTop: 8,
              fontSize: 12,
            }}
          >
            <strong>{c.label}</strong> · {c.samples} flights
            <div>apogee p5 / p50 / p95: {c.p5.toFixed(1)} / {c.p50.toFixed(1)} / {c.p95.toFixed(1)} m</div>
            <div>mean landing range: {c.landingMean.toFixed(1)} m</div>
          </div>
        ))}
      </aside>
    </div>
  );
}
