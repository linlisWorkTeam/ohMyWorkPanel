import { describe, expect, it } from "vitest";
import { chatbotSlotTaken, groupHasActiveChatbot } from "./memberForm";
import type { Group, Member } from "./types";

function member(partial: Partial<Member> & Pick<Member, "id" | "kind" | "isActive">): Member {
  return {
    groupId: "g",
    displayName: "x",
    avatarColor: "#000",
    roleDescription: "",
    tags: "",
    createdAt: 1,
    adapter: null,
    executablePath: null,
    runtimeStatus: null,
    ...partial,
  };
}

function group(kind: "chat" | "project"): Group {
  return {
    id: "g",
    name: "t",
    workspacePath: kind === "chat" ? "" : "/tmp",
    ownerMemberId: "o",
    adminMemberId: null,
    createdAt: 1,
    groupKind: kind,
    archived: false,
  };
}

describe("groupHasActiveChatbot", () => {
  it("detects an active chatbot", () => {
    expect(
      groupHasActiveChatbot([
        member({ id: "1", kind: "agent", isActive: true }),
        member({ id: "2", kind: "chatbot", isActive: true }),
      ]),
    ).toBe(true);
  });

  it("ignores inactive chatbot", () => {
    expect(
      groupHasActiveChatbot([member({ id: "2", kind: "chatbot", isActive: false })]),
    ).toBe(false);
  });
});

describe("chatbotSlotTaken", () => {
  it("blocks second chatbot only in project groups", () => {
    const bots = [member({ id: "2", kind: "chatbot", isActive: true })];
    expect(chatbotSlotTaken(group("project"), bots)).toBe(true);
    expect(chatbotSlotTaken(group("chat"), bots)).toBe(false);
  });
});
