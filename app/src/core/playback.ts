// Flight-mode playback engine (Day 6). Pure functions over a stored
// RunRecord — playback never calls the simulator.

import type { PlaybackSample, RunRecord } from "./types";

export function flightDuration(record: RunRecord): number {
  const last = record.samples[record.samples.length - 1];
  return last ? last.t : 0;
}

/** Linear interpolation of the trajectory at time t (clamped to the record). */
export function sampleAt(record: RunRecord, t: number): PlaybackSample {
  const samples = record.samples;
  if (samples.length === 0) return { t: 0, altitude_m: 0, velocity_ms: 0 };
  if (t <= samples[0].t) return samples[0];
  const last = samples[samples.length - 1];
  if (t >= last.t) return last;

  // Binary search for the bracketing pair.
  let lo = 0;
  let hi = samples.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (samples[mid].t <= t) lo = mid;
    else hi = mid;
  }
  const a = samples[lo];
  const b = samples[hi];
  const f = (t - a.t) / (b.t - a.t);
  return {
    t,
    altitude_m: a.altitude_m + f * (b.altitude_m - a.altitude_m),
    velocity_ms: a.velocity_ms + f * (b.velocity_ms - a.velocity_ms),
  };
}

export interface Warning {
  id: string;
  message: string;
}

/** Deterministic warnings derived from the stored record only. */
export function deriveWarnings(record: RunRecord): Warning[] {
  const warnings: Warning[] = [];
  const s = record.summary;
  const kinds = new Set(record.events.map((e) => e.kind));

  if (!kinds.has("Liftoff")) {
    warnings.push({
      id: "no-liftoff",
      message: "Rocket never lifted off — thrust does not exceed weight.",
    });
    return warnings; // nothing else is meaningful
  }
  if (s.rail_exit_velocity_ms < 15.0) {
    warnings.push({
      id: "slow-rail-exit",
      message: `Rail-exit velocity ${s.rail_exit_velocity_ms.toFixed(1)} m/s is low (hobby guidance ≥ 15 m/s for adequate fin authority).`,
    });
  }
  if (!kinds.has("RecoveryDeploy")) {
    warnings.push({
      id: "ballistic-descent",
      message: "No recovery device — ballistic descent.",
    });
  }
  if (Math.abs(s.landing_velocity_ms) > 8.0) {
    warnings.push({
      id: "hard-landing",
      message: `Landing at ${Math.abs(s.landing_velocity_ms).toFixed(1)} m/s is hard (typical safe chute descent < 8 m/s).`,
    });
  }
  return warnings;
}

/** Ordered timeline entries for display. */
export function timeline(record: RunRecord) {
  return [...record.events].sort((a, b) => a.t - b.t);
}
