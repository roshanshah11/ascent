// Procedural mesh from the vehicle model tree (v0.3 Step 3). Pure module:
// no renderer imports, no DOM — the post-redesign renderer consumes the
// same arrays. The tree that drives this mesh is the same tree that
// drives Barrowman and the mass rollup, so the rendered rocket and the
// simulated rocket can never disagree. Part ranges carry PartIds so the
// viewport can highlight tree selections.

import type { Vehicle, VehiclePart } from "./types";

export const RADIAL_SEGMENTS = 24;
export const NOSE_RINGS = 16;

export interface PartRange {
  /** PartId of the tree node this range renders. */
  id: number;
  /** Part kind tag ("nose_cone", "body_tube", "fin_set", …). */
  kind: string;
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
 * y(x) = sqrt(rho² − (L − x)²) + R − rho. This is the same shape family
 * whose center of pressure sits at ≈ 0.466·L in ascent-aero's Barrowman
 * model — the aero math and the rendered geometry describe one rocket.
 */
export function ogiveRadius(x: number, length: number, baseRadius: number): number {
  const rho = (baseRadius * baseRadius + length * length) / (2 * baseRadius);
  const dy = length - x;
  return Math.sqrt(rho * rho - dy * dy) + baseRadius - rho;
}

const num = (part: VehiclePart, field: string): number => {
  const value = part.kind[field];
  return typeof value === "number" ? value : 0;
};

const isStructural = (part: VehiclePart): boolean =>
  part.kind.type === "nose_cone" ||
  part.kind.type === "body_tube" ||
  part.kind.type === "transition";

const structuralLength = (part: VehiclePart): number =>
  isStructural(part) ? num(part, "length_m") : 0;

/** Airframe stack height in meters (structural parts only). */
export function stackHeightM(vehicle: Vehicle): number {
  return vehicle.parts.reduce((sum, p) => sum + structuralLength(p), 0);
}

export function vehicleToMesh(vehicle: Vehicle): RocketMesh {
  const totalLen = stackHeightM(vehicle);
  const positions: number[] = [];
  const normals: number[] = [];
  const indices: number[] = [];
  const parts: PartRange[] = [];

  // Y is up: aft end of the stack at y = 0, nose tip at y = totalLen.
  // Tree stations x are measured from the tip, so y = totalLen − x.
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

  const beginPart = (): number => indices.length;
  const endPart = (part: VehiclePart, start: number) => {
    parts.push({
      id: part.id,
      kind: part.kind.type,
      start,
      count: indices.length - start,
    });
  };

  const emitFins = (part: VehiclePart, parentForeX: number, parentLen: number, r: number) => {
    const start = beginPart();
    const count = Math.max(1, Math.round(num(part, "count")));
    const rootChord = num(part, "root_chord_m");
    const tipChord = num(part, "tip_chord_m");
    const span = num(part, "span_m");
    const sweep = num(part, "sweep_m");
    // Trailing edge flush with the parent's aft end (VEHICLE_TREE.md).
    const rootLeX = parentForeX + parentLen - rootChord;
    const yRootLe = totalLen - rootLeX;
    const yRootTe = yRootLe - rootChord;
    const yTipLe = yRootLe - sweep;
    const yTipTe = yTipLe - tipChord;
    for (let f = 0; f < count; f++) {
      const a = (f / count) * Math.PI * 2;
      const ux = Math.cos(a); // outward direction in the XZ plane
      const uz = Math.sin(a);
      const base = positions.length / 3;
      const corners: Array<[number, number]> = [
        [r, yRootLe],
        [r, yRootTe],
        [r + span, yTipTe],
        [r + span, yTipLe],
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
    }
    endPart(part, start);
  };

  let cursor = 0; // station of the current part's fore end, from the tip
  for (const part of vehicle.parts) {
    const foreX = cursor;
    const length = structuralLength(part);
    const yFore = totalLen - foreX;
    const yAft = yFore - length;

    if (part.kind.type === "nose_cone") {
      const start = beginPart();
      const baseR = num(part, "base_radius_m");
      const shape = part.kind.shape as string;
      const radiusAt = (x: number) =>
        shape === "conical" ? (baseR * x) / length : ogiveRadius(x, length, baseR);
      // Rings run base → tip; station x measured from the tip.
      let prevRing = pushRing(yAft, baseR, 0);
      for (let i = 1; i <= NOSE_RINGS; i++) {
        const t = i / NOSE_RINGS;
        const x = length * (1 - t); // distance from the tip
        const radius = i === NOSE_RINGS ? 0 : radiusAt(x);
        // Slope dr/dy for the normal, via central difference on the profile.
        const h = length / (NOSE_RINGS * 4);
        const slope =
          (radiusAt(Math.max(0, x - h)) - radiusAt(Math.min(length, x + h))) / (-2 * h);
        const ring = pushRing(yAft + t * length, radius, slope);
        stitchRings(prevRing, ring);
        prevRing = ring;
      }
      endPart(part, start);
    } else if (part.kind.type === "body_tube") {
      const start = beginPart();
      const r = num(part, "outer_radius_m");
      const bottom = pushRing(yAft, r, 0);
      const top = pushRing(yFore, r, 0);
      stitchRings(bottom, top);
      endPart(part, start);
    } else if (part.kind.type === "transition") {
      const start = beginPart();
      const rFore = num(part, "fore_radius_m");
      const rAft = num(part, "aft_radius_m");
      const slope = (rAft - rFore) / (length || 1);
      const bottom = pushRing(yAft, rAft, -slope);
      const top = pushRing(yFore, rFore, -slope);
      stitchRings(bottom, top);
      endPart(part, start);
    }

    for (const child of part.children) {
      if (child.kind.type === "fin_set") {
        const r =
          part.kind.type === "body_tube"
            ? num(part, "outer_radius_m")
            : num(part, "base_radius_m");
        emitFins(child, foreX, length, r);
      }
    }
    cursor += length;
  }

  return {
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    indices: new Uint32Array(indices),
    parts,
  };
}
