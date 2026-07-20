// Pure geometry for the dispersion-ensemble viewport (v0.5 Step 7).
// Zero renderer imports — like core/mesh.ts, this module turns a
// DispersionSummary into plain point arrays that the R3F layer feeds into
// (instanced) geometry. Kept pure so the ensemble math is unit-tested
// without a canvas, and so the render layer stays a consumer of snapshots.
//
// Coordinate frame matches the vehicle viewport: +Y is altitude, +X is
// downrange (the wind/flight-plane axis the planar solver drifts along),
// +Z is crossrange. The current planar dispersion has no crossrange, so
// landing points fall on the X axis and the ellipse degenerates to its
// downrange axis — represented honestly here, not faked into a disc.
//
// Provenance note: the compact dispersion summary stores only apogee and
// landing range per member, not re-integrated trajectories. The member
// "ribbons" are therefore SCHEMATIC arcs — a quadratic through
// launch → apogee → landing that conveys ensemble spread, not exact
// flight paths. Callers must label them as such.

import type { CompactRun, DispersionSummary } from "./types";

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

/** One point per ensemble member at its landing range, on the ground. */
export function landingPoints(summary: DispersionSummary): Vec3[] {
  return summary.runs.map((run) => ({ x: run.landing_range_m, y: 0, z: 0 }));
}

/**
 * Closed ring approximating the landing ellipse, centered on the mean
 * landing range. `segments` points (the ring is closed by the consumer
 * repeating the first, or by line-loop rendering). A zero crossrange axis
 * (planar solver) yields a flat back-and-forth segment along X — the true
 * shape of the current model, not a fabricated disc.
 */
export function landingEllipseRing(
  summary: DispersionSummary,
  segments: number,
): Vec3[] {
  const { a_m, b_m, bearing_deg } = summary.landing_ellipse;
  const cx = summary.landing_mean_m;
  const bearing = (bearing_deg * Math.PI) / 180;
  const cos = Math.cos(bearing);
  const sin = Math.sin(bearing);
  const points: Vec3[] = [];
  const n = Math.max(3, Math.floor(segments));
  for (let i = 0; i < n; i++) {
    const theta = (2 * Math.PI * i) / n;
    // Ellipse in its own axes, then rotate the major axis to the bearing
    // in the ground (X–Z) plane.
    const ex = a_m * Math.cos(theta);
    const ez = b_m * Math.sin(theta);
    points.push({
      x: cx + ex * cos - ez * sin,
      y: 0,
      z: ex * sin + ez * cos,
    });
  }
  return points;
}

export interface ApogeeShell {
  label: string;
  apogee_m: number;
}

/** Apogee percentile heights (p5/p50/p95) the viewport draws as shells. */
export function apogeeShells(summary: DispersionSummary): ApogeeShell[] {
  return [
    { label: "p5", apogee_m: summary.apogee_p5_m },
    { label: "p50", apogee_m: summary.apogee_p50_m },
    { label: "p95", apogee_m: summary.apogee_p95_m },
  ];
}

/**
 * A schematic arc for one member: a quadratic Bézier from launch (origin)
 * through a peak at (range/2, apogee) to landing (range, 0), sampled into
 * `segments + 1` points. NOT a re-integrated trajectory — see the module
 * note. Peak of the sampled curve equals `apogee` only at the midpoint
 * control weighting used here (control y = 2·apogee), which is exact for a
 * quadratic Bézier midpoint.
 */
export function memberArc(run: CompactRun, segments: number): Vec3[] {
  const range = run.landing_range_m;
  const apogee = run.apogee_m;
  const n = Math.max(1, Math.floor(segments));
  const cxp = range / 2;
  const cyp = 2 * apogee; // control point so the curve peaks at `apogee`
  const points: Vec3[] = [];
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const mt = 1 - t;
    // Quadratic Bézier: P0=(0,0), P1=(cxp,cyp), P2=(range,0).
    const x = 2 * mt * t * cxp + t * t * range;
    const y = 2 * mt * t * cyp;
    points.push({ x, y, z: 0 });
  }
  return points;
}

/** Flatten every member arc into one contiguous point array plus the
 *  per-member vertex count, for building one instanced/segmented buffer. */
export function ensembleArcs(
  summary: DispersionSummary,
  segments: number,
): { points: Vec3[]; perMember: number } {
  const perMember = Math.max(1, Math.floor(segments)) + 1;
  const points: Vec3[] = [];
  for (const run of summary.runs) {
    for (const p of memberArc(run, segments)) points.push(p);
  }
  return { points, perMember };
}

/** Axis-aligned bounds over the ensemble (landings + apogee shells), for
 *  camera framing. Returns null for an empty ensemble. */
export function ensembleBounds(
  summary: DispersionSummary,
): { max_range_m: number; max_apogee_m: number } | null {
  if (summary.runs.length === 0) return null;
  let maxRange = 0;
  let maxApogee = 0;
  for (const run of summary.runs) {
    maxRange = Math.max(maxRange, Math.abs(run.landing_range_m));
    maxApogee = Math.max(maxApogee, run.apogee_m);
  }
  maxApogee = Math.max(maxApogee, summary.apogee_p95_m);
  return { max_range_m: maxRange, max_apogee_m: maxApogee };
}
