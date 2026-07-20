import { vehicleToMesh, type PartRange } from "./mesh";
import type { Vehicle, VehiclePart } from "./types";

type Vec3 = readonly [number, number, number];

export interface ParsedStl {
  header: Uint8Array;
  triangles: readonly [Vec3, Vec3, Vec3][];
}

export interface PartStl {
  partId: number;
  filename: string;
  bytes: Uint8Array;
}

const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const cross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const length = (v: Vec3): number => Math.hypot(v[0], v[1], v[2]);
const normalize = (v: Vec3): Vec3 => {
  const n = length(v);
  return n > 0 ? [v[0] / n, v[1] / n, v[2] / n] : [0, 1, 0];
};
const key = (v: Vec3): string => `${v[0]},${v[1]},${v[2]}`;

function findPart(parts: readonly VehiclePart[], id: number): VehiclePart | undefined {
  for (const part of parts) {
    if (part.id === id) return part;
    const child = findPart(part.children, id);
    if (child) return child;
  }
  return undefined;
}

function shellThicknessM(part: VehiclePart | undefined): number {
  if (!part) return 0.001;
  if (part.kind.type === "fin_set") return Math.max(0.0001, Number(part.kind.thickness_mm) / 1000);
  if (part.kind.type === "body_tube") return Math.max(0.0001, Number(part.kind.wall_mm) / 1000);
  return 0.001;
}

/** Turn one display surface range into a closed, finite-thickness solid.
 * This intentionally consumes `vehicleToMesh`: STL and the R3F viewport
 * share the same model-tree geometry, while the STL path adds the missing
 * back faces and boundary walls required by a printable solid. */
function solidify(
  positions: Float32Array,
  indices: Uint32Array,
  range: PartRange,
  thicknessM: number,
): [Vec3, Vec3, Vec3][] {
  const vertices: Vec3[] = [];
  const ids = new Map<string, number>();
  const faces: number[][] = [];
  const add = (v: Vec3): number => {
    const k = key(v);
    const present = ids.get(k);
    if (present !== undefined) return present;
    const next = vertices.length;
    vertices.push(v);
    ids.set(k, next);
    return next;
  };
  for (let i = range.start; i < range.start + range.count; i += 3) {
    const face = [indices[i], indices[i + 1], indices[i + 2]].map((index) => {
      const at = index * 3;
      return add([positions[at], positions[at + 1], positions[at + 2]]);
    });
    const [a, b, c] = face.map((index) => vertices[index]) as [Vec3, Vec3, Vec3];
    if (length(cross(sub(b, a), sub(c, a))) > 1e-12) faces.push(face);
  }
  if (faces.length === 0) return [];

  const normals: Vec3[] = vertices.map(() => [0, 0, 0]);
  const edges = new Map<string, { a: number; b: number; count: number }>();
  const edge = (a: number, b: number) => {
    const name = a < b ? `${a}:${b}` : `${b}:${a}`;
    const found = edges.get(name);
    if (found) found.count += 1;
    else edges.set(name, { a, b, count: 1 });
  };
  for (const [ia, ib, ic] of faces) {
    const n = cross(sub(vertices[ib], vertices[ia]), sub(vertices[ic], vertices[ia]));
    for (const index of [ia, ib, ic]) {
      const old = normals[index];
      normals[index] = [old[0] + n[0], old[1] + n[1], old[2] + n[2]];
    }
    edge(ia, ib); edge(ib, ic); edge(ic, ia);
  }
  const half = thicknessM / 2;
  const outer = vertices.map((v, i) => {
    const n = normalize(normals[i]);
    return [v[0] + n[0] * half, v[1] + n[1] * half, v[2] + n[2] * half] as Vec3;
  });
  const inner = vertices.map((v, i) => {
    const n = normalize(normals[i]);
    return [v[0] - n[0] * half, v[1] - n[1] * half, v[2] - n[2] * half] as Vec3;
  });
  const triangles: [Vec3, Vec3, Vec3][] = [];
  for (const [a, b, c] of faces) {
    triangles.push([outer[a], outer[b], outer[c]], [inner[c], inner[b], inner[a]]);
  }
  for (const boundary of edges.values()) {
    if (boundary.count !== 1) continue;
    const { a, b } = boundary;
    triangles.push([outer[a], inner[a], inner[b]], [outer[a], inner[b], outer[b]]);
  }
  return triangles;
}

function binaryStl(headerText: string, triangles: readonly [Vec3, Vec3, Vec3][]): Uint8Array {
  const bytes = new Uint8Array(84 + triangles.length * 50);
  bytes.set(new TextEncoder().encode(headerText).slice(0, 80));
  const view = new DataView(bytes.buffer);
  view.setUint32(80, triangles.length, true);
  let offset = 84;
  for (const triangle of triangles) {
    const normal = normalize(cross(sub(triangle[1], triangle[0]), sub(triangle[2], triangle[0])));
    for (const v of [normal, ...triangle]) {
      view.setFloat32(offset, v[0], true); view.setFloat32(offset + 4, v[1], true); view.setFloat32(offset + 8, v[2], true);
      offset += 12;
    }
    view.setUint16(offset, 0, true);
    offset += 2;
  }
  return bytes;
}

export function exportVehicleBinaryStl(vehicle: Vehicle): readonly PartStl[] {
  const mesh = vehicleToMesh(vehicle);
  return mesh.parts.flatMap((range) => {
    const triangles = solidify(mesh.positions, mesh.indices, range, shellThicknessM(findPart(vehicle.parts, range.id)));
    if (triangles.length === 0) return [];
    return [{
      partId: range.id,
      filename: `${vehicle.name.replace(/[^a-z0-9]+/gi, "-").replace(/^-|-$/g, "").toLowerCase() || "vehicle"}-part-${range.id}.stl`,
      bytes: binaryStl(`Ascent part ${range.id}`, triangles),
    }];
  });
}

export function parseBinaryStl(bytes: Uint8Array): ParsedStl {
  if (bytes.byteLength < 84) throw new Error("binary STL header is truncated");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const count = view.getUint32(80, true);
  const expected = 84 + count * 50;
  if (bytes.byteLength !== expected) throw new Error(`binary STL length ${bytes.byteLength} does not match declared triangle count ${count}`);
  const triangles: [Vec3, Vec3, Vec3][] = [];
  let offset = 84;
  for (let i = 0; i < count; i++) {
    offset += 12; // stored normal; recompute when validating
    const read = (): Vec3 => {
      const v: Vec3 = [view.getFloat32(offset, true), view.getFloat32(offset + 4, true), view.getFloat32(offset + 8, true)];
      offset += 12;
      return v;
    };
    triangles.push([read(), read(), read()]);
    offset += 2;
  }
  return { header: bytes.slice(0, 80), triangles };
}

export function assertWatertight(stl: ParsedStl): void {
  const edges = new Map<string, number>();
  const add = (a: Vec3, b: Vec3) => {
    const ka = key(a); const kb = key(b);
    const edge = ka < kb ? `${ka}|${kb}` : `${kb}|${ka}`;
    edges.set(edge, (edges.get(edge) ?? 0) + 1);
  };
  for (const triangle of stl.triangles) {
    if (triangle.some((v) => !v.every(Number.isFinite))) throw new Error("STL has non-finite vertex");
    if (length(cross(sub(triangle[1], triangle[0]), sub(triangle[2], triangle[0]))) <= 1e-12) throw new Error("STL has degenerate triangle");
    add(triangle[0], triangle[1]); add(triangle[1], triangle[2]); add(triangle[2], triangle[0]);
  }
  if (stl.triangles.length === 0) throw new Error("STL has no triangles");
  if ([...edges.values()].some((count) => count !== 2)) throw new Error("STL is not watertight");
}
