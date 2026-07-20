import { describe, expect, it } from "vitest";
import { semanticTimeline } from "./semanticTimeline";
import type { CounterfactualReview, RunRecord } from "./types";

const record = {
  samples: [{ t: 0, altitude_m: 0, velocity_ms: 0 }, { t: 1, altitude_m: 10, velocity_ms: 20 }, { t: 2, altitude_m: 0, velocity_ms: -8 }],
  events: [{ kind: "RailExit", t: 0.2, altitude_m: 1, velocity_ms: 4 }, { kind: "Apogee", t: 1, altitude_m: 10, velocity_ms: 0 }],
  summary: { input_hash: "a".repeat(64), apogee_m: 10, max_velocity_ms: 20, rail_exit_velocity_ms: 4, landing_velocity_ms: -8, landing_time_s: 2 },
} as RunRecord;

describe("semanticTimeline", () => {
  it("orders flight, limit, evidence, residual, and decision lanes on one clock", () => {
    const universe = {
      proposed_document_hash: "b".repeat(64),
      baseline_state: { alignment: { scale: 1, offset_s: 0.5 } },
      measured_trace: { trace_id: "measured", channels: [{ id: "altitude", kind: "scalar", samples: [{ time: 0, values: [0], valid: true }, { time: 1, values: [9], valid: true }] }] },
      proposed_reconciliation: { channels: [{ quantity: "altitude", residuals: [{ review_time_s: 0.5, value: 1 }, { review_time_s: 1.5, value: 3 }] }] },
    } as unknown as CounterfactualReview;
    const events = semanticTimeline(record, universe);
    expect(new Set(events.map((event) => event.lane))).toEqual(new Set(["flight-phase", "limit", "evidence", "residual", "decision"]));
    expect(events.find((event) => event.lane === "residual")?.reviewTime).toBe(1.5);
    expect(events.find((event) => event.type === "measured evidence begins")?.reviewTime).toBe(0.5);
    expect(events.map((event) => event.reviewTime)).toEqual([...events].map((event) => event.reviewTime).sort((a, b) => a - b));
  });
});
