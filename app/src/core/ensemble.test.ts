import { describe, expect, it } from "vitest";
import {
  apogeeShells,
  ensembleArcs,
  ensembleBounds,
  landingEllipseRing,
  landingPoints,
  memberArc,
} from "./ensemble";
import type { DispersionSummary } from "./types";

function summary(overrides: Partial<DispersionSummary> = {}): DispersionSummary {
  return {
    seed: 1,
    samples: 3,
    vary: [],
    apogee_p5_m: 340,
    apogee_p50_m: 360,
    apogee_p95_m: 380,
    landing_mean_m: 20,
    landing_ellipse: { a_m: 10, b_m: 0, bearing_deg: 0 },
    runs: [
      { apogee_m: 350, landing_range_m: 15, max_aoa_deg: 2 },
      { apogee_m: 360, landing_range_m: 20, max_aoa_deg: 3 },
      { apogee_m: 370, landing_range_m: 25, max_aoa_deg: 4 },
    ],
    ...overrides,
  };
}

describe("landingPoints", () => {
  it("places one ground point per member at its landing range", () => {
    const pts = landingPoints(summary());
    expect(pts).toHaveLength(3);
    expect(pts.map((p) => p.x)).toEqual([15, 20, 25]);
    expect(pts.every((p) => p.y === 0 && p.z === 0)).toBe(true);
  });
});

describe("memberArc", () => {
  it("starts at launch, ends at landing range, and peaks at apogee", () => {
    const arc = memberArc({ apogee_m: 100, landing_range_m: 40, max_aoa_deg: 0 }, 20);
    expect(arc).toHaveLength(21);
    const first = arc[0];
    const last = arc[arc.length - 1];
    expect(first.x).toBeCloseTo(0, 9);
    expect(first.y).toBeCloseTo(0, 9);
    expect(last.x).toBeCloseTo(40, 9);
    expect(last.y).toBeCloseTo(0, 9);
    // Quadratic Bézier peaks exactly at the midpoint sample (n even).
    const mid = arc[10];
    expect(mid.x).toBeCloseTo(20, 9);
    expect(mid.y).toBeCloseTo(100, 9);
    // Never dips below ground.
    expect(arc.every((p) => p.y >= -1e-9)).toBe(true);
  });

  it("is deterministic", () => {
    const run = { apogee_m: 123, landing_range_m: 45, max_aoa_deg: 1 };
    expect(memberArc(run, 8)).toEqual(memberArc(run, 8));
  });
});

describe("landingEllipseRing", () => {
  it("closes into a ring centered on the mean landing range", () => {
    const ring = landingEllipseRing(summary(), 16);
    expect(ring).toHaveLength(16);
    // b_m = 0 (planar): every point lies on the ground and on the X axis
    // (degenerate ellipse), spanning mean ± a_m.
    expect(ring.every((p) => Math.abs(p.z) < 1e-9 && p.y === 0)).toBe(true);
    const xs = ring.map((p) => p.x);
    expect(Math.max(...xs)).toBeCloseTo(30, 9); // 20 + a_m
    expect(Math.min(...xs)).toBeCloseTo(10, 9); // 20 - a_m
  });

  it("rotates the major axis to the bearing and grows crossrange with b_m", () => {
    const ring = landingEllipseRing(
      summary({ landing_ellipse: { a_m: 10, b_m: 4, bearing_deg: 90 } }),
      32,
    );
    // With a 90° bearing, the a-axis maps onto Z and b onto X.
    const zs = ring.map((p) => p.z);
    expect(Math.max(...zs)).toBeCloseTo(10, 6);
    expect(ring.some((p) => Math.abs(p.z) > 1e-9)).toBe(true);
  });
});

describe("apogeeShells", () => {
  it("returns p5/p50/p95 in order", () => {
    expect(apogeeShells(summary())).toEqual([
      { label: "p5", apogee_m: 340 },
      { label: "p50", apogee_m: 360 },
      { label: "p95", apogee_m: 380 },
    ]);
  });
});

describe("ensembleArcs", () => {
  it("concatenates member arcs with a fixed per-member vertex count", () => {
    const { points, perMember } = ensembleArcs(summary(), 10);
    expect(perMember).toBe(11);
    expect(points).toHaveLength(3 * 11);
  });
});

describe("ensembleBounds", () => {
  it("spans the widest landing and the highest apogee", () => {
    const bounds = ensembleBounds(summary());
    expect(bounds).not.toBeNull();
    expect(bounds!.max_range_m).toBe(25);
    expect(bounds!.max_apogee_m).toBe(380); // p95 exceeds any single run
  });

  it("returns null for an empty ensemble", () => {
    expect(ensembleBounds(summary({ runs: [] }))).toBeNull();
  });
});
