import { describe, expect, it } from "vitest";
import {
  agentReplyDefaultOpen,
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
