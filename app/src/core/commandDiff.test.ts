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

  it("journals every dual-deploy field and clears optional values with null", () => {
    const dual = {
      ...base,
      chute: {
        ...base.chute,
        main_deploy_altitude_m: 60,
        drogue_diameter_cm: 8,
        drogue_cd: 0.8,
      },
    };
    expect(diffDesign(base, dual)).toEqual([
      { cmd: "set_sim_param", param: "chute.main_deploy_altitude_m", value: 60 },
      { cmd: "set_sim_param", param: "chute.drogue_diameter_cm", value: 8 },
      { cmd: "set_sim_param", param: "chute.drogue_cd", value: 0.8 },
    ]);
    expect(
      diffDesign(dual, {
        ...dual,
        chute: {
          ...dual.chute,
          main_deploy_altitude_m: null,
          drogue_diameter_cm: null,
          drogue_cd: null,
        },
      }),
    ).toEqual([
      { cmd: "set_sim_param", param: "chute.main_deploy_altitude_m", value: null },
      { cmd: "set_sim_param", param: "chute.drogue_diameter_cm", value: null },
      { cmd: "set_sim_param", param: "chute.drogue_cd", value: null },
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
