import { describe, expect, it } from "vitest";
import {
  detectMemoryPressure,
  effectiveHeartbeatSeconds,
  formatHeartbeatLabel,
} from "./heartbeatPolicy";

const base = {
  heartbeatAuto: true,
  heartbeatFocusSeconds: 1,
  heartbeatBackgroundSeconds: 5,
};

describe("heartbeatPolicy", () => {
  it("uses focus/background intervals in Auto", () => {
    expect(effectiveHeartbeatSeconds({ focused: true, settings: base })).toBe(1);
    expect(effectiveHeartbeatSeconds({ focused: false, settings: base })).toBe(5);
  });

  it("downgrades under memory pressure when Auto", () => {
    expect(
      effectiveHeartbeatSeconds({ focused: true, settings: base, memoryPressure: true }),
    ).toBe(5);
  });

  it("formats current rate label", () => {
    expect(formatHeartbeatLabel({ focused: true, settings: base })).toContain("当前 1s");
    expect(detectMemoryPressure(4)).toBe(true);
    expect(detectMemoryPressure(8)).toBe(false);
  });
});
