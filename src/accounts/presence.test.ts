import { describe, expect, it } from "vitest";

import { applyPresence, shouldBumpUnread } from "./presence";

describe("applyPresence", () => {
  it("replaces the whole set on a snapshot", () => {
    const next = applyPresence(new Set(["a"]), {
      kind: "presence_snapshot",
      onlineUserIds: ["b", "c", "", null as unknown as string],
    });
    expect([...next].sort()).toEqual(["b", "c"]);
  });

  it("adds a user on presence online", () => {
    const next = applyPresence(new Set(["a"]), { kind: "presence", userId: "b", online: true });
    expect([...next].sort()).toEqual(["a", "b"]);
  });

  it("removes a user on presence offline", () => {
    const next = applyPresence(new Set(["a", "b"]), { kind: "presence", userId: "a", online: false });
    expect([...next]).toEqual(["b"]);
  });

  it("keeps the same reference for unrelated events", () => {
    const base = new Set(["a"]);
    expect(applyPresence(base, { kind: "run_status" })).toBe(base);
  });
});

describe("shouldBumpUnread", () => {
  it("bumps on message_created with a group", () => {
    expect(shouldBumpUnread({ kind: "message_created", groupId: "g1" })).toBe(true);
  });

  it("bumps on completed run status", () => {
    expect(shouldBumpUnread({ kind: "run_status", groupId: "g1", status: "completed" })).toBe(true);
    expect(shouldBumpUnread({ kind: "run_status", groupId: "g1", status: "running" })).toBe(false);
  });

  it("never bumps without a group", () => {
    expect(shouldBumpUnread({ kind: "message_created", groupId: null })).toBe(false);
  });
});
