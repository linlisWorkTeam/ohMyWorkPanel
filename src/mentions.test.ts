import { describe, expect, it } from "vitest";
import { findMentionedMemberIds } from "./mentions";
import type { Member } from "./types";

const member = (id: string, displayName: string): Member => ({
  id, groupId: "g", kind: "agent", displayName, avatarColor: "#000", roleDescription: "",
  isActive: true, adapter: "mock", executablePath: null, runtimeStatus: "ready", tags: "", createdAt: 0
});

describe("findMentionedMemberIds", () => {
  it("matches separate mentions and prefers full names", () => {
    const members = [member("short", "Codex"), member("long", "Codex 审查")];
    expect(findMentionedMemberIds("请 @Codex 审查 以及 @Codex", members)).toEqual(["long", "short"]);
  });
});
