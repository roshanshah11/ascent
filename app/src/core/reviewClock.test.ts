import { describe, expect, it } from "vitest";
import { createReviewClock, reduceReviewClock, resolveDomainTime, type ReviewEvent } from "./reviewClock";

describe("global review clock", () => {
  it("drives deterministic play pause speed range seek and end behavior", () => {
    let state = createReviewClock(20);
    state = reduceReviewClock(state, { type: "setRange", range: [5, 15] });
    state = reduceReviewClock(state, { type: "seek", time: 5 });
    state = reduceReviewClock(state, { type: "setSpeed", speed: 2 });
    state = reduceReviewClock(state, { type: "play" });
    state = reduceReviewClock(state, { type: "tick", elapsedSeconds: 3 });
    expect(state.time).toBe(11);
    state = reduceReviewClock(state, { type: "tick", elapsedSeconds: 3 });
    expect(state.time).toBe(15);
    expect(state.playing).toBe(false);
    expect(reduceReviewClock(state, { type: "seek", time: 7 }).time).toBe(7);
  });

  it("seeks typed events exactly and keeps detector evidence", () => {
    const event: ReviewEvent = {
      id: "burnout",
      type: "burnout",
      reviewTime: 1.86,
      lane: "flight-phase",
      detector: { id: "burnout-crossing", version: "1", confidence: 0.99, evidenceHashes: ["ab"] },
    };
    const state = reduceReviewClock(createReviewClock(10), { type: "selectEvent", event });
    expect(state.time).toBe(1.86);
    expect(state.selectedEvent).toEqual(event);
  });

  it("maps every consumer through explicit domain transforms", () => {
    expect(resolveDomainTime(10, { offset: -2, scale: 0.999 })).toBeCloseTo(7.99, 12);
    expect(() => resolveDomainTime(10, { offset: 0, scale: 0 })).toThrow(/scale/);
  });
});
