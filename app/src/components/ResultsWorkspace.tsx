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

const WIDTH = 920;
const HEIGHT = 440;
const PAD = { left: 62, right: 22, top: 24, bottom: 46 };
const SERIES_COLORS = ["#68a7d8", "#d5a953", "#70b98a", "#d06a60", "#a58ac4", "#a6abb3"];

function Plot({ plot, scatter }: { plot: PlotData; scatter: boolean }) {
  const bounds = plotBounds(plot);
  if (!bounds) return <div className="results-empty">Nothing to plot yet.</div>;
  const spanX = bounds.maxX - bounds.minX || 1;
  const spanY = bounds.maxY - bounds.minY || 1;
  const x = (value: number) => PAD.left + ((value - bounds.minX) / spanX) * (WIDTH - PAD.left - PAD.right);
  const y = (value: number) => HEIGHT - PAD.bottom - ((value - bounds.minY) / spanY) * (HEIGHT - PAD.top - PAD.bottom);
  const xTicks = niceTicks(bounds.minX, bounds.maxX);
  const yTicks = niceTicks(bounds.minY, bounds.maxY);

  return (
    <svg className="results-plot" viewBox={`0 0 ${WIDTH} ${HEIGHT}`} role="img" aria-label={plot.title}>
      {xTicks.map((tick) => (
        <g key={`x${tick}`}>
          <line x1={x(tick)} y1={PAD.top} x2={x(tick)} y2={HEIGHT - PAD.bottom} className="plot-gridline" />
          <text x={x(tick)} y={HEIGHT - PAD.bottom + 18} textAnchor="middle">{tick}</text>
        </g>
      ))}
      {yTicks.map((tick) => (
        <g key={`y${tick}`}>
          <line x1={PAD.left} y1={y(tick)} x2={WIDTH - PAD.right} y2={y(tick)} className="plot-gridline" />
          <text x={PAD.left - 8} y={y(tick) + 3} textAnchor="end">{tick}</text>
        </g>
      ))}
      <line x1={PAD.left} y1={HEIGHT - PAD.bottom} x2={WIDTH - PAD.right} y2={HEIGHT - PAD.bottom} className="plot-axis" />
      <line x1={PAD.left} y1={PAD.top} x2={PAD.left} y2={HEIGHT - PAD.bottom} className="plot-axis" />
      <text x={(PAD.left + WIDTH - PAD.right) / 2} y={HEIGHT - 8} textAnchor="middle" className="plot-axis-label">{plot.xLabel}</text>
      <text x="15" y={(PAD.top + HEIGHT - PAD.bottom) / 2} textAnchor="middle" transform={`rotate(-90 15 ${(PAD.top + HEIGHT - PAD.bottom) / 2})`} className="plot-axis-label">{plot.yLabel}</text>
      {plot.series.map((series, index) => {
        const color = SERIES_COLORS[index % SERIES_COLORS.length];
        return scatter ? (
          <g key={series.label}>
            {series.points.map((point, pointIndex) => (
              <circle key={pointIndex} cx={x(point.x)} cy={y(point.y)} r="3" fill={color} fillOpacity="0.74" stroke="var(--bg-canvas)" strokeWidth="0.8" />
            ))}
          </g>
        ) : (
          <polyline key={series.label} fill="none" stroke={color} strokeWidth="1.6" vectorEffect="non-scaling-stroke" points={series.points.map((point) => `${x(point.x)},${y(point.y)}`).join(" ")} />
        );
      })}
      {plot.series.map((series, index) => (
        <g key={`legend-${series.label}`} transform={`translate(${PAD.left + 10}, ${PAD.top + 11 + index * 18})`}>
          <rect width="12" height="2" fill={SERIES_COLORS[index % SERIES_COLORS.length]} />
          <text x="18" y="4" className="plot-legend-label">{series.label}</text>
        </g>
      ))}
    </svg>
  );
}

function downloadCsv(name: string, csv: string) {
  const url = URL.createObjectURL(new Blob([csv], { type: "text/csv" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  URL.revokeObjectURL(url);
}

export default function ResultsWorkspace({ studies, record }: { studies: Study[]; record: RunRecord | null }) {
  const [tab, setTab] = useState<"dispersion" | "trajectory">("dispersion");
  const [deselected, setDeselected] = useState<Set<number>>(new Set());
  const selectable = studies.filter((study) => study.kind.kind === "dispersion" && study.results !== undefined);
  const selected = selectable.filter((study) => !deselected.has(study.id));

  const plot = useMemo(() => {
    if (tab === "trajectory") return record ? trajectoryPlot(record) : null;
    return dispersionOverlay(selected);
  }, [tab, record, studies, deselected]); // eslint-disable-line react-hooks/exhaustive-deps

  const summaries = percentileSummaries(selected);

  const toggle = (id: number) => {
    setDeselected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <div className="results-workspace">
      <aside className="results-browser">
        <div className="results-browser-heading"><span className="panel-eyebrow">Datasets</span><h2>Result Sources</h2></div>
        <div className="result-source-group">
          <span className="result-source-label">Completed studies</span>
          {selectable.length === 0 && <div className="results-empty compact">No completed dispersion studies yet — run one from the Design workspace.</div>}
          {selectable.map((study) => (
            <label className="result-source" key={study.id}>
              <input type="checkbox" checked={!deselected.has(study.id)} onChange={() => toggle(study.id)} />
              <span><strong>{study.name}</strong><small>{study.engine} · seed {study.seed}</small></span>
            </label>
          ))}
        </div>
      </aside>

      <section className="results-canvas">
        <header className="results-toolbar">
          <div>
            <span className="panel-eyebrow">Post-processing</span>
            <h2>{tab === "dispersion" ? "Landing dispersion" : "Trajectory history"}</h2>
          </div>
          <nav className="result-tabs" aria-label="Result plot">
            <button onClick={() => setTab("dispersion")} disabled={tab === "dispersion"}>Dispersion</button>
            <button onClick={() => setTab("trajectory")} disabled={tab === "trajectory" || !record} title={record ? "" : "Run a simulation first"}>Trajectory</button>
          </nav>
          {plot && plot.series.length > 0 && <button className="button-secondary export-button" onClick={() => downloadCsv(`${plot.title.toLowerCase()}.csv`, toCsv(plot))}>Export CSV</button>}
        </header>
        <div className="results-plot-wrap">
          {plot ? <Plot plot={plot} scatter={tab === "dispersion"} /> : <div className="results-empty">Run a simulation to see its trajectory.</div>}
        </div>
      </section>

      <aside className="results-inspector">
        <div className="results-browser-heading"><span className="panel-eyebrow">Statistics</span><h2>Percentiles</h2></div>
        {summaries.length === 0 && <div className="results-empty compact">Select a completed study to inspect its statistical envelope.</div>}
        {summaries.map((summary) => (
          <section className="result-summary" key={summary.label}>
            <header><strong>{summary.label}</strong><span>{summary.samples} flights</span></header>
            <div className="summary-metric"><span>Apogee p5</span><strong>{summary.p5.toFixed(1)} m</strong></div>
            <div className="summary-metric"><span>Apogee p50</span><strong>{summary.p50.toFixed(1)} m</strong></div>
            <div className="summary-metric"><span>Apogee p95</span><strong>{summary.p95.toFixed(1)} m</strong></div>
            <div className="summary-metric"><span>Mean landing</span><strong>{summary.landingMean.toFixed(1)} m</strong></div>
            <p>apogee p5 / p50 / p95</p>
          </section>
        ))}
      </aside>
    </div>
  );
}
