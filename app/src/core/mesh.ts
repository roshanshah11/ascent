// Procedural mesh generation from a Design (v0.2 Step 10). Pure module:
// no renderer imports, no DOM — the post-redesign renderer (WebGPU or
// wgpu-native) consumes the same arrays. Bodies of revolution + plates
// cover the market; there is deliberately no CAD kernel here.
//
// Geometry derivation is provisional until the vehicle editor lands
// (same status as planar_vehicle_for on the Rust side): nose = 3 calibers
// of tangent ogive, body = 9 calibers of cylinder (12-caliber stack,
// matching the planar model's provisional length), 3 fins. Only the
// diameter comes from the Design today; the proportions are the contract.

import type { Design } from "./types";

export const NOSE_CALIBERS = 3;
export const BODY_CALIBERS = 9;
export const FIN_COUNT = 3;
export const RADIAL_SEGMENTS = 24;
export const NOSE_RINGS = 16;

export interface PartRange {
  name: string;
  /** First index into `indices`. */
  start: number;
  /** Number of indices (triangles * 3). */
  count: number;
}

export interface RocketMesh {
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
  parts: PartRange[];
}

/**
 * Tangent-ogive profile radius at axial station x from the tip (0 ≤ x ≤ L).
 * Classic construction: ogive circle radius rho = (R² + L²) / (2R); then
 * y(x) = sqrt(rho² − (L − x)²) + R − rho. This is the same shape whose
 * center of pressure sits at ≈ 0.466·L in ascent-aero's Barrowman model —
 * the aero math and the rendered geometry describe one rocket.
 */
export function ogiveRadius(x: number, length: number, baseRadius: number): number {
  const rho = (baseRadius * baseRadius + length * length) / (2 * baseRadius);
  const dy = length - x;
  return Math.sqrt(rho * rho - dy * dy) + baseRadius - rho;
}

/** Overall stack height in meters for a given design (12 calibers). */
export function stackHeightM(design: Design): number {
  return (design.diameter_mm / 1000) * (NOSE_CALIBERS + BODY_CALIBERS);
}

export function designToMesh(design: Design): RocketMesh {
  const d = design.diameter_mm / 1000; // caliber in meters
  const r = d / 2;
  const noseLen = d * NOSE_CALIBERS;
  const bodyLen = d * BODY_CALIBERS;

  const positions: number[] = [];
  const normals: number[] = [];
  const indices: number[] = [];
  const parts: PartRange[] = [];

  // Y is up: base of the body at y = 0, nose tip at y = totalLen.
  // A ring is RADIAL_SEGMENTS vertices at height y with radius radius.
  const pushRing = (y: number, radius: number, slope: number): number => {
    const first = positions.length / 3;
    for (let s = 0; s < RADIAL_SEGMENTS; s++) {
      const a = (s / RADIAL_SEGMENTS) * Math.PI * 2;
      const cx = Math.cos(a);
      const cz = Math.sin(a);
      positions.push(radius * cx, y, radius * cz);
      // Surface normal of a body of revolution: radial component tilted by
      // the profile slope dr/dy; normalize (radial 1, axial -slope).
      const inv = 1 / Math.hypot(1, slope);
      normals.push(cx * inv, -slope * inv, cz * inv);
    }
    return first;
  };

  const stitchRings = (ringA: number, ringB: number) => {
    for (let s = 0; s < RADIAL_SEGMENTS; s++) {
      const sn = (s + 1) % RADIAL_SEGMENTS;
      indices.push(ringA + s, ringB + s, ringB + sn);
      indices.push(ringA + s, ringB + sn, ringA + sn);
    }
  };

  // --- Body cylinder: two rings, zero slope. --------------------------
  const bodyStart = indices.length;
  const bottom = pushRing(0, r, 0);
  const top = pushRing(bodyLen, r, 0);
  stitchRings(bottom, top);
  parts.push({ name: "body", start: bodyStart, count: indices.length - bodyStart });

  // --- Nose: tangent-ogive revolve, tip at totalLen. ------------------
  const noseStart = indices.length;
  let prevRing = top; // ogive base radius equals body radius by construction
  for (let i = 1; i <= NOSE_RINGS; i++) {
    // Station measured from the BASE of the nose (x in ogiveRadius runs
    // tip→base, so convert): ring i sits at height bodyLen + t*noseLen.
    const t = i / NOSE_RINGS;
    const x = noseLen * (1 - t); // distance from the tip
    const radius = i === NOSE_RINGS ? 0 : ogiveRadius(x, noseLen, r);
    // Slope dr/dy for the normal, via central difference on the profile.
    const h = noseLen / (NOSE_RINGS * 4);
    const slope =
      (ogiveRadius(Math.max(0, x - h), noseLen, r) -
        ogiveRadius(Math.min(noseLen, x + h), noseLen, r)) /
      (-2 * h);
    const ring = pushRing(bodyLen + t * noseLen, radius, slope);
    stitchRings(prevRing, ring);
    prevRing = ring;
  }
  parts.push({ name: "nose", start: noseStart, count: indices.length - noseStart });

  // --- Fins: flat trapezoid plates, evenly spaced around the base. ----
  // Root chord 2 calibers up from the base, tip chord 1 caliber, span 1.5.
  const rootChord = 2 * d;
  const tipChord = 1 * d;
  const span = 1.5 * d;
  for (let f = 0; f < FIN_COUNT; f++) {
    const finStart = indices.length;
    const a = (f / FIN_COUNT) * Math.PI * 2;
    const ux = Math.cos(a); // outward direction in the XZ plane
    const uz = Math.sin(a);
    const base = positions.length / 3;
    // Four corners in the fin's own plane (outward u, up y):
    //   root leading (r, rootChord) — root trailing (r, 0)
    //   tip trailing (r+span, 0)    — tip leading (r+span, tipChord... swept)
    const corners: Array<[number, number]> = [
      [r, rootChord], // root leading edge
      [r, 0], // root trailing edge
      [r + span, 0], // tip trailing edge
      [r + span, tipChord], // tip leading edge
    ];
    // Normal of the plate is perpendicular to the outward direction.
    const nx = -uz;
    const nz = ux;
    for (const [out, y] of corners) {
      positions.push(out * ux, y, out * uz);
      normals.push(nx, 0, nz);
    }
    indices.push(base, base + 1, base + 2);
    indices.push(base, base + 2, base + 3);
    parts.push({ name: `fin-${f}`, start: finStart, count: indices.length - finStart });
  }

  return {
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    indices: new Uint32Array(indices),
    parts,
  };
}
