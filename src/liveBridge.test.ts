import { describe, expect, it } from "vitest";
import {
  appendGroupIdToLiveUrl,
  buildLiveMentionMessage,
  messageToPlainText,
  projectLiveChatLines,
  resolveLiveResponder,
} from "./liveBridge";
import type { Group, Member, Message } from "./types";

function member(partial: Partial<Member> & Pick<Member, "id" | "kind" | "isActive" | "displayName">): Member {
  return {
    groupId: "g",
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

describe("liveBridge", () => {
  it("projects chat lines and prefers admin responder", () => {
    const members = [
      member({ id: "u", kind: "user", isActive: true, displayName: "Alice" }),
      member({ id: "bot", kind: "chatbot", isActive: true, displayName: "Bot" }),
      member({ id: "a", kind: "agent", isActive: true, displayName: "Cursor" }),
    ];
    const group: Group = {
      id: "g",
      name: "g",
      workspacePath: "",
      ownerMemberId: "u",
      adminMemberId: "a",
      createdAt: 1,
      groupKind: "chat",
      archived: false,
    };
    expect(resolveLiveResponder(group, members)?.id).toBe("a");
    const msgs: Message[] = [
      {
        id: "m1",
        groupId: "g",
        senderMemberId: "u",
        parentRunId: null,
        content: "hello",
        status: "completed",
        createdAt: 1,
      },
    ];
    const lines = projectLiveChatLines(msgs, members);
    expect(lines[0].role).toBe("user");
    expect(lines[0].text).toBe("hello");
  });

  it("builds mention content and appends groupId", () => {
    const bot = member({ id: "bot", kind: "chatbot", isActive: true, displayName: "Bot" });
    expect(buildLiveMentionMessage("你好", bot)).toEqual({
      content: "@Bot 你好",
      mentionIds: ["bot"],
    });
    expect(appendGroupIdToLiveUrl("/api/extensions/panellive/live.html", "g1")).toBe(
      "/api/extensions/panellive/live.html?groupId=g1",
    );
  });

  it("reads final channel from parts json", () => {
    const content = JSON.stringify({
      v: 1,
      parts: [{ channel: "thinking", text: "..." }, { channel: "final", text: "简短回复" }],
    });
    expect(messageToPlainText(content)).toBe("简短回复");
  });
});
