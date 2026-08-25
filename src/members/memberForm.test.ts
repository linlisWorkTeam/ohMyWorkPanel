import { describe, expect, it } from "vitest";
import {
  canSubmitUserMember,
  chatbotSlotTaken,
  groupHasActiveChatbot,
  memberRosterAction,
} from "./memberForm";
import type { Group, Member } from "../types";

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

describe("canSubmitUserMember", () => {
  it("requires password for create and id for link; invite needs only display name", () => {
    expect(canSubmitUserMember("create", { loginUsername: "a", loginPassword: "p", existingAuthUserId: "" })).toBe(true);
    expect(canSubmitUserMember("create", { loginUsername: "a", loginPassword: "", existingAuthUserId: "" })).toBe(false);
    expect(canSubmitUserMember("link", { loginUsername: "", loginPassword: "", existingAuthUserId: "u1" })).toBe(true);
    expect(canSubmitUserMember("link", { loginUsername: "a", loginPassword: "p", existingAuthUserId: "" })).toBe(false);
    expect(canSubmitUserMember("invite", { loginUsername: "", loginPassword: "", existingAuthUserId: "" })).toBe(true);
  });
});

describe("memberRosterAction", () => {
  it("uses delete for inactive or pending invite", () => {
    expect(memberRosterAction(member({ id: "1", kind: "user", isActive: true }))).toBe("remove");
    expect(memberRosterAction(member({ id: "1", kind: "user", isActive: false }))).toBe("delete");
    expect(
      memberRosterAction(member({ id: "1", kind: "user", isActive: true, invitePending: true })),
    ).toBe("delete");
  });
});
