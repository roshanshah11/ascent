import { describe, expect, it } from "vitest";
import { NOSE_RINGS, ogiveRadius, RADIAL_SEGMENTS, stackHeightM, vehicleToMesh } from "./mesh";
import type { Vehicle } from "./types";

// Mirror of ascent-domain's reference_vehicle(): the Alpha III tree.
const reference = (): Vehicle => ({
  name: "Estes Alpha III",
  parts: [
    {
      id: 1,
      kind: {
        type: "nose_cone",
        shape: "tangent_ogive",
        length_m: 0.075,
        base_radius_m: 0.0125,
        mass_g: 8,
      },
      children: [],
    },
    {
      id: 2,
      kind: { type: "body_tube", length_m: 0.225, outer_radius_m: 0.0125, wall_mm: 0.5, mass_g: 15 },
      children: [
        {
          id: 3,
          kind: {
            type: "fin_set",
            count: 3,
            root_chord_m: 0.05,
            tip_chord_m: 0.025,
            span_m: 0.0375,
            sweep_m: 0,
            thickness_mm: 3,
            mass_g: 6,
          },
          children: [],
        },
      ],
    },
  ],
});

describe("ogiveRadius", () => {
  it("is zero at the tip and the base radius at the base", () => {
    expect(ogiveRadius(0, 0.075, 0.0125)).toBeCloseTo(0, 10);
    expect(ogiveRadius(0.075, 0.075, 0.0125)).toBeCloseTo(0.0125, 10);
  });

  it("grows monotonically from tip to base", () => {
    let prev = 0;
    for (let i = 1; i <= 20; i++) {
      const r = ogiveRadius((i / 20) * 0.075, 0.075, 0.0125);
      expect(r).toBeGreaterThan(prev);
      prev = r;
    }
  });
});

describe("vehicleToMesh", () => {
  it("keeps positions, normals, and indices consistent", () => {
    const mesh = vehicleToMesh(reference());
    expect(mesh.positions.length % 3).toBe(0);
    expect(mesh.normals.length).toBe(mesh.positions.length);
    expect(mesh.indices.length % 3).toBe(0);
    const vertexCount = mesh.positions.length / 3;
    for (const index of mesh.indices) {
      expect(index).toBeLessThan(vertexCount);
    }
  });

  it("tiles the index buffer exactly with part ranges carrying PartIds", () => {
    const mesh = vehicleToMesh(reference());
    expect(mesh.parts.map((p) => p.id)).toEqual([1, 2, 3]);
    expect(mesh.parts.map((p) => p.kind)).toEqual(["nose_cone", "body_tube", "fin_set"]);
    let cursor = 0;
    const ordered = [...mesh.parts].sort((a, b) => a.start - b.start);
    for (const part of ordered) {
      expect(part.start).toBe(cursor);
      cursor += part.count;
    }
    expect(cursor).toBe(mesh.indices.length);
  });

  it("spans the tree's stack height with the tip at the top", () => {
    const v = reference();
    expect(stackHeightM(v)).toBeCloseTo(0.3, 12);
    const mesh = vehicleToMesh(v);
    let minY = Infinity;
    let maxY = -Infinity;
    for (let i = 1; i < mesh.positions.length; i += 3) {
      minY = Math.min(minY, mesh.positions[i]);
      maxY = Math.max(maxY, mesh.positions[i]);
    }
    expect(minY).toBeCloseTo(0, 6);
    expect(maxY).toBeCloseTo(0.3, 6);
  });

  it("shapes the nose rings by the ogive equation from the tree's dimensions", () => {
    const mesh = vehicleToMesh(reference());
    // The nose is emitted first: base ring at y = 0.225, tip at 0.3.
    for (let i = 0; i <= NOSE_RINGS; i++) {
      const t = i / NOSE_RINGS;
      const ringStart = i * RADIAL_SEGMENTS * 3;
      const x = mesh.positions[ringStart];
      const z = mesh.positions[ringStart + 2];
      const radius = Math.hypot(x, z);
      const expected = i === NOSE_RINGS ? 0 : ogiveRadius(0.075 * (1 - t), 0.075, 0.0125);
      expect(radius).toBeCloseTo(expected, 6);
      expect(mesh.positions[ringStart + 1]).toBeCloseTo(0.225 + t * 0.075, 6);
    }
  });

  it("renders a conical nose with a linear profile", () => {
    const v = reference();
    v.parts[0].kind.shape = "conical";
    const mesh = vehicleToMesh(v);
    const midRing = (NOSE_RINGS / 2) * RADIAL_SEGMENTS * 3;
    const radius = Math.hypot(mesh.positions[midRing], mesh.positions[midRing + 2]);
    expect(radius).toBeCloseTo(0.0125 / 2, 6);
  });

  it("emits one plate per fin at even azimuths, trailing edge flush aft", () => {
    const mesh = vehicleToMesh(reference());
    const fins = mesh.parts.find((p) => p.kind === "fin_set")!;
    expect(fins.count).toBe(3 * 2 * 3); // 3 fins × 2 triangles × 3 indices
    // First fin vertex is the root leading edge: y = root chord above the
    // parent's aft end (trailing edge flush at y = 0).
    const firstFinVertex = mesh.indices[fins.start] * 3;
    expect(mesh.positions[firstFinVertex + 1]).toBeCloseTo(0.05, 6);
    for (let f = 0; f < 3; f++) {
      const base = firstFinVertex + f * 4 * 3;
      const cx = (mesh.positions[base] + mesh.positions[base + 9]) / 2;
      const cz = (mesh.positions[base + 2] + mesh.positions[base + 11]) / 2;
      const angle = ((Math.atan2(cz, cx) * 180) / Math.PI + 360) % 360;
      expect(angle % 360).toBeCloseTo((f * 360) / 3, 3);
    }
  });

  it("scales with the tree's dimensions, keeping topology fixed", () => {
    const small = vehicleToMesh(reference());
    const big = reference();
    big.parts[0].kind.base_radius_m = 0.025;
    big.parts[1].kind.outer_radius_m = 0.025;
    const scaled = vehicleToMesh(big);
    expect(scaled.indices.length).toBe(small.indices.length);
    expect(scaled.positions.length).toBe(small.positions.length);
  });

  it("keeps every normal unit length", () => {
    const mesh = vehicleToMesh(reference());
    for (let i = 0; i < mesh.normals.length; i += 3) {
      const len = Math.hypot(mesh.normals[i], mesh.normals[i + 1], mesh.normals[i + 2]);
      expect(len).toBeCloseTo(1, 6);
    }
  });

  it("renders a stage coupler as a structural tube segment", () => {
    const v = reference();
    v.parts.push({
      id: 6,
      kind: { type: "stage_coupler", length_m: 0.02, outer_radius_m: 0.0125, mass_g: 4, separation_delay_s: 0 },
      children: [],
    });
    v.parts.push({
      id: 7,
      kind: { type: "body_tube", length_m: 0.09, outer_radius_m: 0.0125, wall_mm: 0.5, mass_g: 12 },
      children: [],
    });
    // Coupler counts toward the stack height…
    expect(stackHeightM(v)).toBeCloseTo(0.075 + 0.225 + 0.02 + 0.09, 9);
    // …and emits its own part range with tube topology (2 rings stitched).
    const mesh = vehicleToMesh(v);
    const coupler = mesh.parts.find((p) => p.kind === "stage_coupler")!;
    expect(coupler.id).toBe(6);
    expect(coupler.count).toBe(RADIAL_SEGMENTS * 2 * 3);
  });

  it("returns an empty mesh for an empty tree", () => {
    const mesh = vehicleToMesh({ name: "empty", parts: [] });
    expect(mesh.indices.length).toBe(0);
    expect(mesh.parts).toEqual([]);
  });
});
