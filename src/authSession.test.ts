import { describe, expect, it } from "vitest";
import { resolveSenderMemberId } from "./authSession";

describe("resolveSenderMemberId", () => {
  const members = [
    { id: "owner", authUserId: null, kind: "user" },
    { id: "guest", authUserId: "u-guest", kind: "user" },
    { id: "bot", authUserId: null, kind: "agent" },
  ];

  it("uses linked member for scoped user", () => {
    expect(resolveSenderMemberId(members, "owner", "u-guest", false)).toBe("guest");
  });

  it("admin falls back to owner", () => {
    expect(resolveSenderMemberId(members, "owner", "seed-user-root", true)).toBe("owner");
  });

  it("scoped user without membership cannot send", () => {
    expect(resolveSenderMemberId(members, "owner", "u-other", false)).toBeNull();
  });
});
