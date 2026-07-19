// DTOs mirroring crates/ascent-app/src/design.rs. If these drift from the
// Rust side, the IPC boundary breaks — change both together.

export interface ChuteSpec {
  enabled: boolean;
  diameter_cm: number;
  cd: number;
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
  | { cmd: "add_part"; parent: number | null; kind: { type: string } & Record<string, unknown> }
  | { cmd: "remove_part"; id: number }
  | { cmd: "set_part_param"; id: number; param: string; value: unknown }
  | { cmd: "set_sim_param"; param: string; value: unknown }
  | { cmd: "select_motor"; designation: string }
  | { cmd: "set_design"; design: Design }
  | { cmd: "create_study"; name: string; kind: StudyKind; engine: string; seed: number }
  | { cmd: "delete_study"; id: number }
  | { cmd: "set_study_param"; id: number; param: string; value: unknown };

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
  credibility: Scorecard;
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

export interface ReviewData {
  apogee_m: number;
  rail_exit_velocity_ms: number;
  checks: RuleCheck[];
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
