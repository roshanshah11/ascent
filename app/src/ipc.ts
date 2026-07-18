// The only file that touches the Tauri bridge. Coarse calls only.
import { invoke } from "@tauri-apps/api/core";
import type {
  Command,
  Design,
  DocumentState,
  DispersionRequest,
  DispersionSummary,
  EvidenceReport,
  MotorInfo,
  Project,
  RepairResult,
  ReviewReport,
  RunRecord,
  SpreadResult,
} from "./core/types";

export function fetchReferenceDesign(): Promise<Design> {
  return invoke<Design>("reference_design");
}

export function getDocument(): Promise<DocumentState> {
  return invoke<DocumentState>("get_document");
}

export function dispatchCommand(command: Command): Promise<DocumentState> {
  return invoke<DocumentState>("dispatch_command", { command });
}

export function undoDocument(): Promise<DocumentState> {
  return invoke<DocumentState>("undo_document");
}

export function redoDocument(): Promise<DocumentState> {
  return invoke<DocumentState>("redo_document");
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

export function runDispersion(
  design: Design,
  request: DispersionRequest,
): Promise<DispersionSummary> {
  return invoke<DispersionSummary>("run_dispersion", { design, request });
}

export function consoleExec(line: string): Promise<DocumentState> {
  return invoke<DocumentState>("console_exec", { line });
}

export function fetchSessionJournal(): Promise<string> {
  return invoke<string>("session_journal");
}

export function enqueueStudyJob(studyId: number): Promise<number> {
  return invoke<number>("enqueue_study_job", { studyId });
}

export function cancelJob(jobId: number): Promise<void> {
  return invoke<void>("cancel_job", { jobId });
}

export function autosaveProject(project: Project): Promise<string> {
  return invoke<string>("autosave_project", { project });
}

export function checkRecovery(): Promise<Project | null> {
  return invoke<Project | null>("check_recovery");
}

export function discardRecovery(): Promise<void> {
  return invoke<void>("discard_recovery");
}
