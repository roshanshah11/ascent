import { describe, expect, it } from "vitest";
import { describeRocketPy } from "./SpreadPanel";

describe("describeRocketPy", () => {
  it("states a concrete unavailable reason", () => {
    expect(
      describeRocketPy({
        available: false,
        engine_id: "rocketpy-bridge",
        engine_version: "ascent-rocketpy-bridge-v1",
        summary: null,
        reason: "failed to start RocketPy bridge: no such file",
      }),
    ).toBe("RocketPy unavailable: failed to start RocketPy bridge: no such file");
  });
});
