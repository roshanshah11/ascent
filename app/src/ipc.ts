// The only file that touches the Tauri bridge. Coarse calls only.
import { invoke } from "@tauri-apps/api/core";
import type { Design, MotorInfo, RunRecord } from "./core/types";

export function fetchReferenceDesign(): Promise<Design> {
  return invoke<Design>("reference_design");
}

export function fetchMotors(): Promise<MotorInfo[]> {
  return invoke<MotorInfo[]>("list_motors");
}

export function runSimulation(design: Design): Promise<RunRecord> {
  return invoke<RunRecord>("run_simulation", { design });
}
