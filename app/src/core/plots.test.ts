import { describe, expect, it } from "vitest";
import {
  dispersionOverlay,
  dispersionSummaryOf,
  niceTicks,
  percentileSummaries,
  plotBounds,
  toCsv,
  trajectoryPlot,
} from "./plots";
import type { RunRecord, Study } from "./types";

const record = {
  samples: [
    { t: 0, altitude_m: 0, velocity_ms: 0 },
    { t: 1, altitude_m: 40, velocity_ms: 70 },
    { t: 2, altitude_m: 120, velocity_ms: 55 },
  ],
} as unknown as RunRecord;

const dispersionStudy = (id: number, name: string, runs: Array<[number, number]>): Study => ({
  id,
  name,
  kind: { kind: "dispersion", flights: runs.length },
  engine: "native",
  seed: 42,
  results: {
    input_hash: "hash",
    data: {
      seed: 42,
      samples: runs.length,
      vary: [],
      apogee_p5_m: 290,
      apogee_p50_m: 300,
      apogee_p95_m: 310,
      landing_mean_m: 55,
      landing_ellipse: { a_m: 20, b_m: 0, bearing_deg: 0 },
      runs: runs.map(([range, apogee]) => ({
        apogee_m: apogee,
        landing_range_m: range,
        max_aoa_deg: 2,
      })),
    },
  },
});

describe("trajectoryPlot", () => {
  it("maps playback samples to altitude and velocity series", () => {
    const plot = trajectoryPlot(record);
    expect(plot.series.map((s) => s.label)).toEqual(["altitude (m)", "velocity (m/s)"]);
    expect(plot.series[0].points).toEqual([
      { x: 0, y: 0 },
      { x: 1, y: 40 },
      { x: 2, y: 120 },
    ]);
    expect(plot.series[1].points[1]).toEqual({ x: 1, y: 70 });
  });
});

describe("dispersionSummaryOf", () => {
  it("returns the payload for a completed dispersion study", () => {
    const summary = dispersionSummaryOf(dispersionStudy(1, "a", [[50, 300]]));
    expect(summary?.apogee_p50_m).toBe(300);
  });

  it("rejects unrun studies, other kinds, and malformed payloads", () => {
    const unrun: Study = { id: 1, name: "u", kind: { kind: "dispersion", flights: 5 }, engine: "native", seed: 1 };
    expect(dispersionSummaryOf(unrun)).toBeNull();
    const single: Study = { id: 2, name: "s", kind: { kind: "single_flight" }, engine: "native", seed: 1, results: { input_hash: "h", data: {} } };
    expect(dispersionSummaryOf(single)).toBeNull();
    const malformed = dispersionStudy(3, "m", []);
    (malformed.results!.data as Record<string, unknown>).runs = "not an array";
    expect(dispersionSummaryOf(malformed)).toBeNull();
  });
});

describe("dispersionOverlay", () => {
  it("overlays completed studies as one labeled series each", () => {
    const plot = dispersionOverlay([
      dispersionStudy(1, "baseline", [[50, 300], [60, 310]]),
      { id: 2, name: "unrun", kind: { kind: "dispersion", flights: 5 }, engine: "native", seed: 1 },
      dispersionStudy(3, "heavy", [[40, 280]]),
    ]);
    expect(plot.series.map((s) => s.label)).toEqual(["baseline", "heavy"]);
    expect(plot.series[0].points).toEqual([
      { x: 50, y: 300 },
      { x: 60, y: 310 },
    ]);
    expect(plot.series[1].points).toEqual([{ x: 40, y: 280 }]);
  });
});

describe("percentileSummaries", () => {
  it("summarizes each completed dispersion study", () => {
    const cards = percentileSummaries([dispersionStudy(1, "baseline", [[50, 300]])]);
    expect(cards).toEqual([
      { label: "baseline", p5: 290, p50: 300, p95: 310, landingMean: 55, samples: 1 },
    ]);
  });
});

describe("plotBounds", () => {
  it("covers every series and returns null when empty", () => {
    const plot = dispersionOverlay([
      dispersionStudy(1, "a", [[50, 300], [60, 310]]),
      dispersionStudy(2, "b", [[40, 280]]),
    ]);
    expect(plotBounds(plot)).toEqual({ minX: 40, maxX: 60, minY: 280, maxY: 310 });
    expect(plotBounds({ title: "", xLabel: "", yLabel: "", series: [] })).toBeNull();
  });
});

describe("niceTicks", () => {
  it("uses the 1/2/5 ladder and covers the range", () => {
    expect(niceTicks(0, 10, 5)).toEqual([0, 2, 4, 6, 8, 10]);
    expect(niceTicks(0, 100, 5)).toEqual([0, 20, 40, 60, 80, 100]);
    const ticks = niceTicks(283, 317, 5);
    expect(ticks[0]).toBeGreaterThanOrEqual(283);
    expect(ticks[ticks.length - 1]).toBeLessThanOrEqual(317);
    expect(ticks.length).toBeGreaterThanOrEqual(2);
  });

  it("degrades safely on a zero-width range", () => {
    expect(niceTicks(5, 5)).toEqual([5, 6]);
  });
});

describe("toCsv", () => {
  it("exports exactly the plotted points, in series order", () => {
    const plot = dispersionOverlay([dispersionStudy(1, "baseline", [[50, 300], [60.5, 310.25]])]);
    expect(toCsv(plot)).toBe(
      "series,x,y\nbaseline,50,300\nbaseline,60.5,310.25\n",
    );
    // Every plotted point appears as a row: the export and the plot
    // cannot disagree.
    const rows = toCsv(plot).trim().split("\n").slice(1);
    const points = plot.series.flatMap((s) => s.points);
    expect(rows.length).toBe(points.length);
    points.forEach((p, i) => {
      expect(rows[i]).toBe(`baseline,${p.x},${p.y}`);
    });
  });

  it("escapes labels containing commas and quotes", () => {
    const csv = toCsv({
      title: "t",
      xLabel: "x",
      yLabel: "y",
      series: [{ label: 'heavy, "final"', points: [{ x: 1, y: 2 }] }],
    });
    expect(csv).toBe('series,x,y\n"heavy, ""final""",1,2\n');
  });
});
