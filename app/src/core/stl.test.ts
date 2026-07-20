import { describe, expect, it } from "vitest";
import { assertWatertight, exportVehicleBinaryStl, parseBinaryStl } from "./stl";
import type { Vehicle } from "./types";

const reference = (): Vehicle => ({
  name: "Estes Alpha III",
  parts: [
    { id: 1, kind: { type: "nose_cone", shape: "tangent_ogive", length_m: 0.075, base_radius_m: 0.0125, mass_g: 8 }, children: [] },
    { id: 2, kind: { type: "body_tube", length_m: 0.225, outer_radius_m: 0.0125, wall_mm: 0.5, mass_g: 15 }, children: [
      { id: 3, kind: { type: "fin_set", count: 3, root_chord_m: 0.05, tip_chord_m: 0.025, span_m: 0.0375, sweep_m: 0, thickness_mm: 3, mass_g: 6 }, children: [] },
    ] },
  ],
});

describe("binary STL export", () => {
  it("round-trips each model-tree part as a watertight binary STL", () => {
    const exports = exportVehicleBinaryStl(reference());
    expect(exports.map((part) => part.partId)).toEqual([1, 2, 3]);
    for (const part of exports) {
      const parsed = parseBinaryStl(part.bytes);
      expect(part.bytes.byteLength).toBe(84 + parsed.triangles.length * 50);
      expect(parsed.triangles.length).toBeGreaterThan(0);
      assertWatertight(parsed);
    }
  });

  it("is deterministic and responds to a fin thickness change", () => {
    const first = exportVehicleBinaryStl(reference());
    const again = exportVehicleBinaryStl(reference());
    expect(again.map((part) => [...part.bytes])).toEqual(first.map((part) => [...part.bytes]));
    const changed = reference();
    changed.parts[1].children[0].kind.thickness_mm = 4;
    const next = exportVehicleBinaryStl(changed);
    expect([...next.find((part) => part.partId === 3)!.bytes]).not.toEqual(
      [...first.find((part) => part.partId === 3)!.bytes],
    );
  });

  it("rejects malformed binary payloads", () => {
    expect(() => parseBinaryStl(new Uint8Array(83))).toThrow("header");
  });
});
