import { describe, expect, it } from "vitest";
import {
  agentReplyDefaultOpen,
  firstUnreadIndex,
  formatQuotePrefix,
  isNearBottom,
  previewAgentReply,
} from "./chatUi";

describe("chatUi scroll helpers", () => {
  it("detects near-bottom within threshold", () => {
    expect(isNearBottom(920, 1000, 80, 80)).toBe(true);
    expect(isNearBottom(0, 1000, 80, 80)).toBe(false);
  });
});

describe("chatUi agent fold defaults", () => {
  it("keeps agent reply open only while streaming", () => {
    expect(agentReplyDefaultOpen(true)).toBe(true);
    expect(agentReplyDefaultOpen(false)).toBe(false);
  });

  it("builds a short preview for collapsed summary", () => {
    expect(previewAgentReply("hello world", 5)).toBe("hello…");
    expect(previewAgentReply("short", 20)).toBe("short");
    expect(previewAgentReply("", 20)).toBe("Agent 答复");
  });
});

describe("quote prefix and unread jump", () => {
  it("formats a WeChat-style quote line without extra schema", () => {
    expect(formatQuotePrefix("Cursor Agent", "把设置放进右栏")).toBe("「引用 Cursor Agent：把设置放进右栏」\n");
    expect(formatQuotePrefix("root", "a".repeat(90)).startsWith("「引用 root：")).toBe(true);
    expect(formatQuotePrefix("root", "a".repeat(90)).includes("…")).toBe(true);
  });

  it("maps unreadCount onto the visible message list", () => {
    expect(firstUnreadIndex(10, 0)).toBe(-1);
    expect(firstUnreadIndex(10, 3)).toBe(7);
    expect(firstUnreadIndex(2, 9)).toBe(0);
  });
});
