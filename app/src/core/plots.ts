// Pure plot-data module (v0.3 Step 7). No DOM, no renderer imports: this
// file turns run records and study results into series, axes, and CSV.
// The workspace component only draws what comes out of here, so every
// number on screen is unit-testable — and the exported CSV is generated
// from the same series the plot renders, so they can never disagree.

import type { DispersionSummary, RunRecord, Study } from "./types";

export interface SeriesPoint {
  x: number;
  y: number;
}

export interface Series {
  label: string;
  points: SeriesPoint[];
}

export interface PlotData {
  title: string;
  xLabel: string;
  yLabel: string;
  series: Series[];
}

/** Altitude and velocity against time from a flight's playback samples. */
export function trajectoryPlot(record: RunRecord): PlotData {
  return {
    title: "Trajectory",
    xLabel: "t (s)",
    yLabel: "altitude (m) / velocity (m/s)",
    series: [
      {
        label: "altitude (m)",
        points: record.samples.map((s) => ({ x: s.t, y: s.altitude_m })),
      },
      {
        label: "velocity (m/s)",
        points: record.samples.map((s) => ({ x: s.t, y: s.velocity_ms })),
      },
    ],
  };
}

/**
 * A study's results payload as a DispersionSummary, or null when the
 * study is not a completed dispersion. The payload was serialized from
 * the Rust DispersionSummary, so presence of `runs` is the shape check.
 */
export function dispersionSummaryOf(study: Study): DispersionSummary | null {
  if (study.kind.kind !== "dispersion" || !study.results) return null;
  const data = study.results.data as unknown;
  if (!data || typeof data !== "object" || !Array.isArray((data as { runs?: unknown }).runs)) {
    return null;
  }
  return data as unknown as DispersionSummary;
}

/**
 * Landing range vs apogee scatter, one series per completed dispersion
 * study — the overlay comparison. Studies without results are skipped.
 */
export function dispersionOverlay(studies: Study[]): PlotData {
  const series: Series[] = [];
  for (const study of studies) {
    const summary = dispersionSummaryOf(study);
    if (!summary) continue;
    series.push({
      label: study.name,
      points: summary.runs.map((r) => ({ x: r.landing_range_m, y: r.apogee_m })),
    });
  }
  return {
    title: "Dispersion",
    xLabel: "landing range (m)",
    yLabel: "apogee (m)",
    series,
  };
}

export interface PercentileSummary {
  label: string;
  p5: number;
  p50: number;
  p95: number;
  landingMean: number;
  samples: number;
}

/** The percentile card for each completed dispersion study. */
export function percentileSummaries(studies: Study[]): PercentileSummary[] {
  const out: PercentileSummary[] = [];
  for (const study of studies) {
    const summary = dispersionSummaryOf(study);
    if (!summary) continue;
    out.push({
      label: study.name,
      p5: summary.apogee_p5_m,
      p50: summary.apogee_p50_m,
      p95: summary.apogee_p95_m,
      landingMean: summary.landing_mean_m,
      samples: summary.samples,
    });
  }
  return out;
}

/** Inclusive bounds over every point of every series, or null when empty. */
export function plotBounds(
  plot: PlotData,
): { minX: number; maxX: number; minY: number; maxY: number } | null {
  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;
  for (const s of plot.series) {
    for (const p of s.points) {
      minX = Math.min(minX, p.x);
      maxX = Math.max(maxX, p.x);
      minY = Math.min(minY, p.y);
      maxY = Math.max(maxY, p.y);
    }
  }
  if (minX > maxX) return null;
  return { minX, maxX, minY, maxY };
}

/**
 * Round tick positions covering [min, max]: a 1/2/5 step ladder, the
 * usual axis convention. Always returns at least two ticks.
 */
export function niceTicks(min: number, max: number, target = 5): number[] {
  if (!(max > min)) return [min, min + 1];
  const rawStep = (max - min) / Math.max(target, 1);
  const power = Math.pow(10, Math.floor(Math.log10(rawStep)));
  const step = [1, 2, 5, 10].map((m) => m * power).find((s) => s >= rawStep) ?? 10 * power;
  const ticks: number[] = [];
  for (let t = Math.ceil(min / step) * step; t <= max + step * 1e-9; t += step) {
    // Snap away float drift so tick labels stay clean.
    ticks.push(Math.round(t / step) * step);
  }
  return ticks.length >= 2 ? ticks : [min, max];
}

/**
 * CSV of exactly the plotted series: `series,x,y` rows in series order,
 * full float precision. Tested to match the plot's points one-to-one.
 */
export function toCsv(plot: PlotData): string {
  const escape = (s: string) => (/[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);
  const lines = ["series,x,y"];
  for (const s of plot.series) {
    for (const p of s.points) {
      lines.push(`${escape(s.label)},${p.x},${p.y}`);
    }
  }
  return lines.join("\n") + "\n";
}
