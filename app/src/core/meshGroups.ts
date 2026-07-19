// Pure adapter between the procedural mesh (mesh.ts, renderer-free) and
// the R3F viewport: maps PartRanges onto BufferGeometry groups so each
// part kind draws with its own material, and converts nose-referenced
// stations (CP/CG from `get_vehicle_markers`) into the mesh's Y-up frame.
// No three/renderer imports — this stays testable in plain node.

import type { RocketMesh } from "./mesh";

export interface MaterialSpec {
  kind: string;
  color: string;
}

/** One material per part kind; index = BufferGeometry material index. */
export const PART_MATERIALS: MaterialSpec[] = [
  { kind: "nose_cone", color: "#c74b3c" },
  { kind: "body_tube", color: "#d8dbe0" },
  { kind: "transition", color: "#b8bdc6" },
  { kind: "stage_coupler", color: "#8f96a3" },
  { kind: "fin_set", color: "#c74b3c" },
];

const BODY_INDEX = PART_MATERIALS.findIndex((m) => m.kind === "body_tube");

export interface GeometryGroup {
  start: number;
  count: number;
  materialIndex: number;
}

export function meshGroups(mesh: RocketMesh): GeometryGroup[] {
  return mesh.parts.map((part) => {
    const index = PART_MATERIALS.findIndex((m) => m.kind === part.kind);
    return {
      start: part.start,
      count: part.count,
      materialIndex: index === -1 ? BODY_INDEX : index,
    };
  });
}

/**
 * Mesh frame is Y-up with the aft end at y = 0 and the nose tip at
 * y = length; marker stations arrive as meters from the nose tip.
 */
export function markerY(lengthM: number, stationFromNoseM: number): number {
  return lengthM - stationFromNoseM;
}
