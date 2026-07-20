// The only file that touches the Tauri bridge. Coarse calls only.
import { invoke } from "@tauri-apps/api/core";
import type {
  Command,
  CommandProposal,
  CounterfactualReview,
  Design,
  DocumentState,
  DispersionRequest,
  DispersionSummary,
  EvidenceReport,
  MotorInfo,
  PartPreview,
  Project,
  RepairResult,
  ReviewReport,
  RunRecord,
  SpreadResult,
  VehicleMarkers,
} from "./core/types";

export interface GrammarCommand {
  verb: string;
  usage: string;
  description: string;
}

export function fetchReferenceDesign(): Promise<Design> {
  return invoke<Design>("reference_design");
}

export function getDocument(): Promise<DocumentState> {
  return invoke<DocumentState>("get_document");
}

export function fetchCommandCatalogue(): Promise<GrammarCommand[]> {
  return invoke<GrammarCommand[]>("command_catalogue");
}

export function getVehicleMarkers(): Promise<VehicleMarkers> {
  return invoke<VehicleMarkers>("get_vehicle_markers");
}

export function previewPartParam(
  id: number,
  param: string,
  value: number,
): Promise<PartPreview> {
  return invoke<PartPreview>("preview_part_param", { id, param, value });
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

/** Deterministic flight-readiness report (Markdown). Structural and rule
 *  sections degrade to "not yet computed" when the review can't run. */
export function fetchReadiness(targetApogeeM: number): Promise<string> {
  return invoke<string>("flight_readiness", { targetApogeeM });
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

export function proposeCommands(lines: string[]): Promise<CommandProposal> {
  return invoke<CommandProposal>("propose_commands", { lines });
}

export function fetchCounterfactualReview(lines: string[]): Promise<CounterfactualReview> {
  return invoke<CounterfactualReview>("counterfactual_review", { lines });
}

export function applyProposal(lines: string[]): Promise<DocumentState> {
  return invoke<DocumentState>("apply_proposal", { lines });
}

export function importAtmosphere(name: string, csv: string): Promise<DocumentState> {
  return invoke<DocumentState>("import_atmosphere", { name, csv });
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
