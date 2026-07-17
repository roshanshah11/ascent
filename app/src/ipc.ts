// The only file that touches the Tauri bridge. Coarse calls only.
import { invoke } from "@tauri-apps/api/core";
import type {
  Design,
  EvidenceReport,
  MotorInfo,
  RepairResult,
  ReviewReport,
  RunRecord,
  SpreadResult,
} from "./core/types";

export function fetchReferenceDesign(): Promise<Design> {
  return invoke<Design>("reference_design");
}

export function fetchMotors(): Promise<MotorInfo[]> {
  return invoke<MotorInfo[]>("list_motors");
}

export function runSimulation(design: Design): Promise<RunRecord> {
  return invoke<RunRecord>("run_simulation", { design });
}

export function runSpread(design: Design): Promise<SpreadResult> {
  return invoke<SpreadResult>("run_spread", { design });
}

export function fetchEvidence(design: Design): Promise<EvidenceReport> {
  return invoke<EvidenceReport>("run_evidence", { design });
}

export function fetchReview(targetApogeeM: number): Promise<ReviewReport> {
  return invoke<ReviewReport>("flight_review", { targetApogeeM });
}

export function solveReview(targetApogeeM: number): Promise<RepairResult> {
  return invoke<RepairResult>("solve_review", { targetApogeeM });
}
