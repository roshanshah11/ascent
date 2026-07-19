import { describe, expect, it } from "vitest";
import { markerY, meshGroups, PART_MATERIALS } from "./meshGroups";
import type { RocketMesh } from "./mesh";

const mesh: RocketMesh = {
  positions: new Float32Array(0),
  normals: new Float32Array(0),
  indices: new Uint32Array(120),
  parts: [
    { id: 1, kind: "nose_cone", start: 0, count: 48 },
    { id: 2, kind: "body_tube", start: 48, count: 48 },
    { id: 3, kind: "fin_set", start: 96, count: 24 },
  ],
};

describe("meshGroups", () => {
  it("maps every part range to a geometry group with its kind's material index", () => {
    const groups = meshGroups(mesh);
    expect(groups).toHaveLength(3);
    expect(groups[0]).toEqual({
      start: 0,
      count: 48,
      materialIndex: PART_MATERIALS.findIndex((m) => m.kind === "nose_cone"),
    });
    expect(groups[2].materialIndex).toBe(
      PART_MATERIALS.findIndex((m) => m.kind === "fin_set"),
    );
  });

  it("falls back to the body material for unknown part kinds", () => {
    const weird: RocketMesh = {
      ...mesh,
      parts: [{ id: 9, kind: "mystery", start: 0, count: 12 }],
    };
    expect(meshGroups(weird)[0].materialIndex).toBe(
      PART_MATERIALS.findIndex((m) => m.kind === "body_tube"),
    );
  });

  it("covers the whole index buffer with no gaps", () => {
    const groups = meshGroups(mesh);
    const covered = groups.reduce((sum, g) => sum + g.count, 0);
    expect(covered).toBe(mesh.indices.length);
  });
});

describe("markerY", () => {
  it("converts a station measured from the nose into mesh Y-up coordinates", () => {
    // Mesh convention: aft end at y = 0, nose tip at y = length.
    expect(markerY(1.0, 0)).toBe(1.0); // at the tip
    expect(markerY(1.0, 1.0)).toBe(0); // at the aft end
    expect(markerY(0.8, 0.3)).toBeCloseTo(0.5);
  });
});
