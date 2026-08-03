import { describe, expect, it } from "vitest";
import { shouldSendOnKey, sendKeyHint } from "./sendKey";

describe("shouldSendOnKey", () => {
  it("enter mode sends on Enter without shift", () => {
    expect(shouldSendOnKey("enter", "Enter", false, false, false)).toBe(true);
    expect(shouldSendOnKey("enter", "Enter", true, false, false)).toBe(false);
  });

  it("ctrlEnter mode requires ctrl/meta", () => {
    expect(shouldSendOnKey("ctrlEnter", "Enter", false, false, false)).toBe(false);
    expect(shouldSendOnKey("ctrlEnter", "Enter", false, true, false)).toBe(true);
    expect(shouldSendOnKey("ctrlEnter", "Enter", false, false, true)).toBe(true);
  });

  it("hints differ by mode", () => {
    expect(sendKeyHint("enter")).toContain("Enter 发送");
    expect(sendKeyHint("ctrlEnter")).toContain("Ctrl+Enter");
  });
});
