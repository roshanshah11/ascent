// Translate an Inspector-style whole-Design edit into the minimal set of
// journal commands (v0.3 Step 2). The Rust dispatcher is the only
// mutation path; this module just names the deltas. Pure — tested
// without IPC.

import type { Command, Design } from "./types";

const SCALAR_FIELDS = ["name", "dry_mass_g", "diameter_mm", "cd", "rail_length_m"] as const;
const CHUTE_FIELDS = [
  "enabled",
  "diameter_cm",
  "cd",
  "main_deploy_altitude_m",
  "drogue_diameter_cm",
  "drogue_cd",
] as const;

export function diffDesign(prev: Design, next: Design): Command[] {
  const cmds: Command[] = [];
  for (const field of SCALAR_FIELDS) {
    if (prev[field] !== next[field]) {
      cmds.push({ cmd: "set_sim_param", param: field, value: next[field] });
    }
  }
  for (const field of CHUTE_FIELDS) {
    if (prev.chute[field] !== next.chute[field]) {
      cmds.push({
        cmd: "set_sim_param",
        param: `chute.${field}`,
        value: next.chute[field] ?? null,
      });
    }
  }
  if (prev.motor_designation !== next.motor_designation) {
    cmds.push({ cmd: "select_motor", designation: next.motor_designation });
  }
  return cmds;
}
