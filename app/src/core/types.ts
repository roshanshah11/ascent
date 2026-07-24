// DTOs mirroring crates/ascent-app/src/design.rs. If these drift from the
// Rust side, the IPC boundary breaks — change both together.

export interface ChuteSpec {
  enabled: boolean;
  diameter_cm: number;
  cd: number;
  main_deploy_altitude_m?: number | null;
  drogue_diameter_cm?: number | null;
  drogue_cd?: number | null;
}

export interface ProfileLayer {
  altitude_m: number;
  wind_speed_ms: number;
  wind_direction_deg: number;
  density_kg_m3?: number;
}

export interface AtmosphereProfile {
  name: string;
  layers: ProfileLayer[];
}

/** Versioned immutable telemetry evidence; channel details remain schema-driven. */
export interface TelemetryBundle {
  schema_version: number;
  bundle_id: string;
  raw_sources: unknown[];
  streams: unknown[];
  [key: string]: unknown;
}

export interface Design {
  name: string;
  dry_mass_g: number;
  diameter_mm: number;
  cd: number;
  chute: ChuteSpec;
  motor_designation: string;
  rail_length_m: number;
}

// --- Document layer (v0.3): Rust owns the state, we render snapshots. ---

export interface VehiclePart {
  id: number;
  kind: { type: string } & Record<string, unknown>;
  children: VehiclePart[];
}

export interface Vehicle {
  name: string;
  parts: VehiclePart[];
}

export type StudyKind =
  | { kind: "single_flight" }
  | { kind: "dispersion"; flights: number }
  | { kind: "motor_trade"; candidates: string[] }
  | { kind: "stability_sweep"; param: string; from: number; to: number; steps: number };

export interface StudyResults {
  input_hash: string;
  data: Record<string, unknown>;
}

export interface Study {
  id: number;
  name: string;
  kind: StudyKind;
  engine: string;
  seed: number;
  results?: StudyResults;
}

export interface DocumentState {
  vehicle: Vehicle;
  design: Design;
  studies: Study[];
  atmosphere?: AtmosphereProfile;
  telemetry?: TelemetryBundle[];
  alignment?: Record<string, unknown>;
  reconciliation?: Record<string, unknown>;
  can_undo: boolean;
  can_redo: boolean;
}

/** Mirrors crates/ascent-app/src/markers.rs — stations in meters from the nose tip. */
export interface VehicleMarkers {
  cp_from_nose_m: number;
  cg_ignition_from_nose_m: number;
  cg_burnout_from_nose_m: number;
  length_m: number;
  diameter_m: number;
  stability_ignition_cal: number;
  stability_burnout_cal: number;
}

export interface PartPreview {
  markers: VehicleMarkers;
  apogee_m: number;
}

export interface CommandCheck {
  index: number;
  text: string;
  error: string | null;
}

export interface ProposalDiff {
  vehicle_changed: boolean;
  design_changed: boolean;
  atmosphere_changed: boolean;
  parts_added: number;
  parts_removed: number;
  studies_added: number;
  studies_removed: number;
  studies_made_stale: number[];
}

export interface CommandProposal {
  valid: boolean;
  commands: CommandCheck[];
  diff: ProposalDiff | null;
}

export interface CounterfactualReview {
  proposal: CommandProposal;
  baseline_document_hash: string;
  proposed_document_hash: string;
  parent_hashes: string[];
  baseline_run: RunRecord;
  proposed_run: RunRecord;
  baseline_state: DocumentState;
  proposed_state: DocumentState;
  baseline_markers: VehicleMarkers;
  proposed_markers: VehicleMarkers;
  baseline_trace: FlightTrace;
  proposed_trace: FlightTrace;
  measured_trace: FlightTrace | null;
  baseline_evidence: EvidenceReport;
  proposed_evidence: EvidenceReport;
  baseline_review: ReviewReport;
  proposed_review: ReviewReport;
  baseline_reconciliation: ReconciliationResult | null;
  proposed_reconciliation: ReconciliationResult | null;
  qualifications: string[];
  baseline_report_html: string;
  report_preview_html: string;
}

export interface FlightTrace {
  trace_id: string;
  channels: Array<{
    id: string;
    kind: string;
    samples: Array<{ time: number; values: number[]; valid: boolean }>;
  }>;
}

export interface ReconciliationResult {
  channels: Array<{
    quantity: string;
    whole_flight: { bias: number; mae: number; rmse: number; max_absolute: number };
    completeness: number;
    residuals?: Array<{ review_time_s: number; value: number }>;
  }>;
  qualifications: string[];
}

/** Payloads of the "job-progress" / "job-done" Tauri events. */
export interface JobProgressEvent {
  event: "progress";
  job_id: number;
  study_id: number;
  completed: number;
  total: number;
}

export interface JobDoneEvent {
  event: "done";
  job_id: number;
  study_id: number;
  status: "queued" | "running" | "done" | "cancelled" | "failed";
  error: string | null;
}

/** Mirrors the serde tag layout of crates/ascent-app/src/command.rs. */
export type Command =
  | { cmd: "batch"; commands: Command[] }
  | { cmd: "add_part"; parent: number | null; kind: { type: string } & Record<string, unknown> }
  | { cmd: "remove_part"; id: number }
  | { cmd: "set_part_param"; id: number; param: string; value: unknown }
  | { cmd: "set_sim_param"; param: string; value: unknown }
  | { cmd: "select_motor"; designation: string }
  | { cmd: "set_design"; design: Design }
  | { cmd: "create_study"; name: string; kind: StudyKind; engine: string; seed: number }
  | { cmd: "delete_study"; id: number }
  | { cmd: "set_study_param"; id: number; param: string; value: unknown }
  | { cmd: "set_atmosphere"; profile: AtmosphereProfile | null }
  | { cmd: "set_telemetry"; bundles: TelemetryBundle[] }
  | { cmd: "set_alignment"; alignment: Record<string, unknown> | null }
  | { cmd: "set_reconciliation"; reconciliation: Record<string, unknown> | null };

export interface MotorInfo {
  designation: string;
  manufacturer: string;
  total_impulse_ns: number;
  burn_time_s: number;
  total_mass_g: number;
}

export interface PlaybackSample {
  t: number;
  altitude_m: number;
  velocity_ms: number;
}

export interface RunEvent {
  kind: string;
  t: number;
  altitude_m: number;
  velocity_ms: number;
}

export interface SimSummaryLite {
  apogee_m: number;
  apogee_time_s: number;
  max_velocity_ms: number;
  burnout_time_s: number;
  burnout_velocity_ms: number;
  rail_exit_velocity_ms: number;
  landing_time_s: number;
  landing_velocity_ms: number;
  input_hash: string;
  [key: string]: unknown;
}

export interface RunRecord {
  design: Design;
  summary: SimSummaryLite;
  events: RunEvent[];
  samples: PlaybackSample[];
}

export interface RocketPySpread {
  available: boolean;
  engine_id: string;
  engine_version: string;
  summary: SimSummaryLite | null;
  reason: string | null;
}

export interface SpreadResult {
  native: SimSummaryLite;
  rocketpy: RocketPySpread;
  apogee_spread_m: number | null;
}

export interface ConvergenceInfo {
  dt_s: number;
  apogee_m: [number, number, number];
  apogee_delta_m: number;
  apogee_delta_fine_m: number;
  converged: boolean;
}

export interface CredibilityFactor {
  name: string;
  score: number; // 0-4 per docs/CREDIBILITY.md
  basis: string;
}

export type RegimeFlag =
  | { kind: "validated" }
  | { kind: "extrapolated"; reason: string };

export interface QuantityFlag {
  quantity: string;
  regime: RegimeFlag;
}

export interface Scorecard {
  factors: CredibilityFactor[];
  quantities: QuantityFlag[];
}

export interface EvidenceReport {
  input_hash: string;
  engine: { id: string; version: string };
  models: string[];
  assumptions: string[];
  motor: {
    designation: string;
    manufacturer: string;
    provenance: unknown;
  };
  convergence: ConvergenceInfo;
  validation: string;
  validation_cases: Array<{
    case_id: string;
    title: string;
    evidence_level: "analytic" | "unit_verified" | "regression_compatible" | "cross_validated" | "flight_data_available" | "flight_validated";
    intended_use: string;
    validity_domain: string[];
    caveats: string[];
    known_mismatches: string[];
    case_hash: string;
  }>;
  credibility: Scorecard;
  study?: { id: number; result_input_hash: string; current_input_hash: string; is_stale: boolean };
  links: Array<{ relation: string; from_hash: string; to_hash: string }>;
  structural_checks: StructuralCheck[];
}

export interface RuleCheck {
  rule_id: string;
  description: string;
  citation: string;
  measured: number;
  required: number;
  comparator: "gte" | "lte";
  pass: boolean;
}

/** Mirrors ascent_review::structural::StructuralCheck. */
export interface StructuralCheck {
  check_id: string;
  label: string;
  value: number;
  limit: number;
  margin: number;
  units: string;
  pass: boolean;
  source: string;
}

export interface ReviewData {
  apogee_m: number;
  rail_exit_velocity_ms: number;
  checks: RuleCheck[];
  structural_checks: StructuralCheck[];
  feasible: boolean;
}

export interface ReviewReport {
  review: ReviewData;
  target_apogee_m: number;
  target_tolerance_m: number;
  target_met: boolean;
  mission_feasible: boolean;
}

export interface DiffEntry {
  field: string;
  before: string;
  after: string;
}

export interface RepairResult {
  review: ReviewData;
  target_apogee_m: number;
  achieved_apogee_m: number;
  diff: DiffEntry[];
}

export type VaryParam =
  | "thrust_pct"
  | "cd_pct"
  | "wind_speed_ms"
  | "launch_angle_deg"
  | "mass_g";

export interface Variation {
  param: VaryParam;
  sigma: number;
}

export interface DispersionRequest {
  seed: number;
  samples: number;
  vary: Variation[];
  base_wind_ms: number;
}

export interface CompactRun {
  apogee_m: number;
  landing_range_m: number;
  max_aoa_deg: number;
}

export interface DispersionSummary {
  seed: number;
  samples: number;
  vary: Variation[];
  apogee_p5_m: number;
  apogee_p50_m: number;
  apogee_p95_m: number;
  landing_mean_m: number;
  landing_ellipse: { a_m: number; b_m: number; bearing_deg: number };
  runs: CompactRun[];
}

export interface Project {
  schema_version: number;
  name: string;
  designs: Design[];
  runs: RunRecord[];
  studies?: Study[];
}
