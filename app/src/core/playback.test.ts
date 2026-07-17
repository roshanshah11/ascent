import { describe, expect, it } from "vitest";
import { deriveWarnings, flightDuration, sampleAt, timeline } from "./playback";
import type { RunRecord } from "./types";

function record(overrides: Partial<RunRecord> = {}): RunRecord {
  return {
    design: {
      name: "test",
      dry_mass_g: 34,
      diameter_mm: 25,
      cd: 0.6,
      chute: { enabled: true, diameter_cm: 30, cd: 0.75 },
      motor_designation: "C6",
      rail_length_m: 0.9,
    },
    summary: {
      apogee_m: 360,
      apogee_time_s: 7.8,
      max_velocity_ms: 112,
      burnout_time_s: 1.86,
      burnout_velocity_ms: 112,
      rail_exit_velocity_ms: 17.6,
      landing_time_s: 103,
      landing_velocity_ms: -3.8,
      input_hash: "a".repeat(64),
    },
    events: [
      { kind: "Liftoff", t: 0.015, altitude_m: 0, velocity_ms: 0 },
      { kind: "RailExit", t: 0.2, altitude_m: 0.9, velocity_ms: 17.6 },
      { kind: "Burnout", t: 1.86, altitude_m: 103, velocity_ms: 112 },
      { kind: "Apogee", t: 7.8, altitude_m: 360, velocity_ms: 0 },
      { kind: "RecoveryDeploy", t: 7.8, altitude_m: 360, velocity_ms: 0 },
      { kind: "Landing", t: 103, altitude_m: 0, velocity_ms: -3.8 },
    ],
    samples: [
      { t: 0, altitude_m: 0, velocity_ms: 0 },
      { t: 1, altitude_m: 30, velocity_ms: 60 },
      { t: 2, altitude_m: 110, velocity_ms: 100 },
      { t: 103, altitude_m: 0, velocity_ms: -3.8 },
    ],
    ...overrides,
  };
}

describe("playback engine", () => {
  it("duration is the last sample time", () => {
    expect(flightDuration(record())).toBe(103);
  });

  it("interpolates linearly between samples", () => {
    const s = sampleAt(record(), 0.5);
    expect(s.altitude_m).toBeCloseTo(15, 10);
    expect(s.velocity_ms).toBeCloseTo(30, 10);
  });

  it("clamps before start and after end", () => {
    expect(sampleAt(record(), -5).t).toBe(0);
    expect(sampleAt(record(), 999).altitude_m).toBe(0);
  });

  it("hits exact sample points", () => {
    const s = sampleAt(record(), 2);
    expect(s.altitude_m).toBe(110);
  });

  it("timeline is time-ordered", () => {
    const t = timeline(record());
    for (let i = 1; i < t.length; i++) {
      expect(t[i].t).toBeGreaterThanOrEqual(t[i - 1].t);
    }
    expect(t[0].kind).toBe("Liftoff");
  });
});

describe("warnings", () => {
  it("clean flight has no warnings", () => {
    expect(deriveWarnings(record())).toEqual([]);
  });

  it("no liftoff short-circuits everything else", () => {
    const r = record({ events: [] });
    const w = deriveWarnings(r);
    expect(w).toHaveLength(1);
    expect(w[0].id).toBe("no-liftoff");
  });

  it("slow rail exit warns", () => {
    const r = record();
    r.summary.rail_exit_velocity_ms = 9.0;
    expect(deriveWarnings(r).map((w) => w.id)).toContain("slow-rail-exit");
  });

  it("ballistic descent and hard landing warn together", () => {
    const r = record();
    r.events = r.events.filter((e) => e.kind !== "RecoveryDeploy");
    r.summary.landing_velocity_ms = -25;
    const ids = deriveWarnings(r).map((w) => w.id);
    expect(ids).toContain("ballistic-descent");
    expect(ids).toContain("hard-landing");
  });
});
