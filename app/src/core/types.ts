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

export interface ConvergenceInfo {
  dt_s: number;
  apogee_m: [number, number, number];
  apogee_delta_m: number;
  apogee_delta_fine_m: number;
  converged: boolean;
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
