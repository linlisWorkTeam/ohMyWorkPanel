import { describe, expect, it } from "vitest";
import {
  mergeHotWithOlder,
  nextVisibleCount,
  prependOlderMessages,
  shouldLoadOlderOnScroll,
  sliceVisibleMessages,
} from "./messageHistory";

describe("messageHistory", () => {
  it("merges hot page with older loaded", () => {
    const older = [
      { id: "a", createdAt: 1 },
      { id: "b", createdAt: 2 },
    ];
    const hot = [
      { id: "c", createdAt: 3 },
      { id: "d", createdAt: 4 },
    ];
    expect(mergeHotWithOlder(older, hot).map((m) => m.id)).toEqual(["a", "b", "c", "d"]);
  });

  it("drops older that fall inside hot window", () => {
    const older = [{ id: "c-old", createdAt: 3 }];
    const hot = [{ id: "c", createdAt: 3 }, { id: "d", createdAt: 4 }];
    expect(mergeHotWithOlder(older, hot).map((m) => m.id)).toEqual(["c", "d"]);
  });

  it("prepends unique older pages", () => {
    const cur = [{ id: "c", createdAt: 3 }];
    const page = [
      { id: "a", createdAt: 1 },
      { id: "c", createdAt: 3 },
    ];
    expect(prependOlderMessages(cur, page).map((m) => m.id)).toEqual(["a", "c"]);
  });

  it("slices visible tail", () => {
    const all = [1, 2, 3, 4, 5];
    expect(sliceVisibleMessages(all, 2)).toEqual([4, 5]);
    expect(sliceVisibleMessages(all, 99)).toEqual(all);
  });

  it("expands visible count and detects scroll top", () => {
    expect(nextVisibleCount(10, 100)).toBe(30);
    expect(nextVisibleCount(95, 100)).toBe(100);
    expect(shouldLoadOlderOnScroll(0)).toBe(true);
    expect(shouldLoadOlderOnScroll(200)).toBe(false);
  });
});
