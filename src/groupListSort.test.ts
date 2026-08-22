import { describe, expect, it } from "vitest";
import {
  bumpUnread,
  clearUnread,
  formatUnreadBadge,
  sortGroupsForSidebar,
} from "./groupListSort";
import type { Group } from "./types";

function g(partial: Partial<Group> & Pick<Group, "id" | "name">): Group {
  return {
    workspacePath: ".",
    ownerMemberId: "o",
    adminMemberId: null,
    createdAt: 1,
    ...partial,
  };
}

describe("sortGroupsForSidebar", () => {
  it("puts unread groups first", () => {
    const sorted = sortGroupsForSidebar([
      g({ id: "a", name: "A", createdAt: 100, unreadCount: 0 }),
      g({ id: "b", name: "B", createdAt: 50, unreadCount: 2 }),
      g({ id: "c", name: "C", createdAt: 200, unreadCount: 0 }),
    ]);
    expect(sorted.map((x) => x.id)).toEqual(["b", "c", "a"]);
  });
});

describe("formatUnreadBadge", () => {
  it("caps at 99+", () => {
    expect(formatUnreadBadge(0)).toBe("");
    expect(formatUnreadBadge(3)).toBe("3");
    expect(formatUnreadBadge(100)).toBe("99+");
  });
});

describe("bump/clear unread", () => {
  it("bumps other groups and clears active", () => {
    const base = [g({ id: "a", name: "A", unreadCount: 0 }), g({ id: "b", name: "B", unreadCount: 1 })];
    const bumped = bumpUnread(base, "a", "b");
    expect(bumped.find((x) => x.id === "a")?.unreadCount).toBe(1);
    expect(bumped[0].id).toBe("a");
    const cleared = clearUnread(bumped, "a");
    expect(cleared.find((x) => x.id === "a")?.unreadCount).toBe(0);
  });

  it("does not bump active group", () => {
    const base = [g({ id: "a", name: "A", unreadCount: 0 })];
    expect(bumpUnread(base, "a", "a")).toEqual(base);
  });
});
