import { describe, expect, it } from "vitest";
import {
  BODY_CALIBERS,
  designToMesh,
  FIN_COUNT,
  NOSE_CALIBERS,
  NOSE_RINGS,
  ogiveRadius,
  RADIAL_SEGMENTS,
  stackHeightM,
} from "./mesh";
import type { Design } from "./types";

const reference: Design = {
  name: "Estes Alpha III",
  dry_mass_g: 34,
  diameter_mm: 25,
  cd: 0.6,
  chute: { enabled: true, diameter_cm: 30, cd: 0.75 },
  motor_designation: "C6",
  rail_length_m: 0.9,
};

describe("ogiveRadius", () => {
  it("matches the tangent-ogive equation at tip, base, and midpoint", () => {
    const L = 0.075;
    const R = 0.0125;
    expect(ogiveRadius(0, L, R)).toBeCloseTo(0, 10); // tip
    expect(ogiveRadius(L, L, R)).toBeCloseTo(R, 10); // base meets the body
    const rho = (R * R + L * L) / (2 * R);
    const mid = Math.sqrt(rho * rho - (L / 2) ** 2) + R - rho;
    expect(ogiveRadius(L / 2, L, R)).toBeCloseTo(mid, 12);
  });

  it("is monotonically increasing from tip to base", () => {
    const L = 0.075;
    const R = 0.0125;
    let prev = -1;
    for (let i = 0; i <= 20; i++) {
      const y = ogiveRadius((i / 20) * L, L, R);
      expect(y).toBeGreaterThan(prev);
      prev = y;
    }
  });
});

describe("designToMesh", () => {
  const mesh = designToMesh(reference);
  const d = reference.diameter_mm / 1000;
  const r = d / 2;

  it("produces consistent array shapes", () => {
    expect(mesh.positions.length % 3).toBe(0);
    expect(mesh.normals.length).toBe(mesh.positions.length);
    expect(mesh.indices.length % 3).toBe(0);
    // Every index points at a real vertex.
    const vertexCount = mesh.positions.length / 3;
    for (const idx of mesh.indices) {
      expect(idx).toBeLessThan(vertexCount);
    }
  });

  it("covers exactly body + nose + fins in the part table", () => {
    const names = mesh.parts.map((p) => p.name);
    expect(names).toEqual(["body", "nose", "fin-0", "fin-1", "fin-2"]);
    // Part ranges tile the index buffer exactly, no gaps or overlaps.
    let cursor = 0;
    for (const part of mesh.parts) {
      expect(part.start).toBe(cursor);
      cursor += part.count;
    }
    expect(cursor).toBe(mesh.indices.length);
  });

  it("has the reference dimensions: 12-caliber height, correct radius", () => {
    let maxY = -Infinity;
    let maxBodyRadius = 0;
    // Body+nose vertices only (fins extend past the body radius).
    const finStart = mesh.parts[2].start;
    const revolveVertexMax = Math.min(...Array.from(mesh.indices.slice(finStart)));
    for (let i = 0; i < mesh.positions.length / 3; i++) {
      const x = mesh.positions[3 * i];
      const y = mesh.positions[3 * i + 1];
      const z = mesh.positions[3 * i + 2];
      maxY = Math.max(maxY, y);
      if (i < revolveVertexMax) {
        maxBodyRadius = Math.max(maxBodyRadius, Math.hypot(x, z));
      }
    }
    expect(maxY).toBeCloseTo(stackHeightM(reference), 6);
    expect(stackHeightM(reference)).toBeCloseTo(d * (NOSE_CALIBERS + BODY_CALIBERS), 12);
    expect(maxBodyRadius).toBeCloseTo(r, 6);
  });

  it("nose rings follow the tangent-ogive profile exactly", () => {
    const noseLen = d * NOSE_CALIBERS;
    const bodyLen = d * BODY_CALIBERS;
    // Nose vertices start after the two body rings.
    const noseFirstVertex = 2 * RADIAL_SEGMENTS;
    for (let ring = 1; ring <= NOSE_RINGS; ring++) {
      const v = noseFirstVertex + (ring - 1) * RADIAL_SEGMENTS;
      const x = mesh.positions[3 * v];
      const y = mesh.positions[3 * v + 1];
      const z = mesh.positions[3 * v + 2];
      const t = ring / NOSE_RINGS;
      expect(y).toBeCloseTo(bodyLen + t * noseLen, 6);
      const expected = ring === NOSE_RINGS ? 0 : ogiveRadius(noseLen * (1 - t), noseLen, r);
      expect(Math.hypot(x, z)).toBeCloseTo(expected, 6);
    }
  });

  it("places the fins at even angles around the base", () => {
    const finParts = mesh.parts.filter((p) => p.name.startsWith("fin-"));
    expect(finParts).toHaveLength(FIN_COUNT);
    const angles = finParts.map((part) => {
      // Centroid of the fin's vertices gives its azimuth.
      const seen = new Set<number>();
      let cx = 0;
      let cz = 0;
      for (let i = part.start; i < part.start + part.count; i++) {
        const v = mesh.indices[i];
        if (seen.has(v)) continue;
        seen.add(v);
        cx += mesh.positions[3 * v];
        cz += mesh.positions[3 * v + 2];
      }
      return Math.atan2(cz, cx);
    });
    const norm = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
    for (let f = 0; f < FIN_COUNT; f++) {
      expect(norm(angles[f])).toBeCloseTo(norm((f / FIN_COUNT) * 2 * Math.PI), 6);
    }
  });

  it("scales with the design diameter", () => {
    const fat = designToMesh({ ...reference, diameter_mm: 50 });
    expect(stackHeightM({ ...reference, diameter_mm: 50 })).toBeCloseTo(
      2 * stackHeightM(reference),
      12
    );
    // Same topology, scaled geometry.
    expect(fat.indices.length).toBe(mesh.indices.length);
    expect(fat.positions.length).toBe(mesh.positions.length);
  });

  it("has unit normals everywhere", () => {
    for (let i = 0; i < mesh.normals.length; i += 3) {
      const len = Math.hypot(mesh.normals[i], mesh.normals[i + 1], mesh.normals[i + 2]);
      expect(len).toBeCloseTo(1, 5);
    }
  });
});
