import { describe, expect, it } from "vitest";
import {
  nextLinkStateOnClose,
  RELEASING_WINDOW_MS,
  releasingBannerText,
  shouldKeepReleasing,
} from "./releasingState";

describe("releasingState", () => {
  it("stays releasing until 60s then timeout", () => {
    expect(nextLinkStateOnClose(10_000, 0)).toBe("releasing");
    expect(nextLinkStateOnClose(RELEASING_WINDOW_MS, 0)).toBe("timeout");
  });

  it("keeps waiting when health is up but ws is down", () => {
    expect(shouldKeepReleasing(true, false, 5_000)).toBe(true);
    expect(shouldKeepReleasing(true, true, 5_000)).toBe(false);
    expect(shouldKeepReleasing(false, false, RELEASING_WINDOW_MS)).toBe(false);
  });

  it("renders banner copy only after 30s quiet period", () => {
    expect(releasingBannerText("connected", 0)).toBeNull();
    expect(releasingBannerText("releasing", 0)).toBeNull();
    expect(releasingBannerText("releasing", 29_999)).toBeNull();
    expect(releasingBannerText("releasing", 30_000)).toMatch(/剩余约 30s/);
    expect(releasingBannerText("releasing", 53_000)).toMatch(/剩余约 7s/);
    expect(releasingBannerText("timeout", RELEASING_WINDOW_MS)).toMatch(/手动刷新/);
  });
});
