import { describe, expect, it } from "vitest";
import { diffDesign } from "./commandDiff";
import type { Design } from "./types";

const base: Design = {
  name: "Estes Alpha III",
  dry_mass_g: 34,
  diameter_mm: 25,
  cd: 0.6,
  chute: { enabled: true, diameter_cm: 30, cd: 0.75 },
  motor_designation: "C6",
  rail_length_m: 0.9,
};

describe("diffDesign", () => {
  it("returns nothing for an identical design", () => {
    expect(diffDesign(base, { ...base, chute: { ...base.chute } })).toEqual([]);
  });

  it("names a single scalar change", () => {
    expect(diffDesign(base, { ...base, cd: 0.7 })).toEqual([
      { cmd: "set_sim_param", param: "cd", value: 0.7 },
    ]);
  });

  it("uses dotted paths for chute fields", () => {
    expect(diffDesign(base, { ...base, chute: { ...base.chute, diameter_cm: 45 } })).toEqual([
      { cmd: "set_sim_param", param: "chute.diameter_cm", value: 45 },
    ]);
  });

  it("routes motor changes through select_motor", () => {
    expect(diffDesign(base, { ...base, motor_designation: "B6" })).toEqual([
      { cmd: "select_motor", designation: "B6" },
    ]);
  });

  it("emits one command per changed field", () => {
    const next: Design = {
      ...base,
      dry_mass_g: 40,
      chute: { ...base.chute, enabled: false },
      motor_designation: "B6",
    };
    expect(diffDesign(base, next)).toEqual([
      { cmd: "set_sim_param", param: "dry_mass_g", value: 40 },
      { cmd: "set_sim_param", param: "chute.enabled", value: false },
      { cmd: "select_motor", designation: "B6" },
    ]);
  });
});
