import { describe, expect, it } from "vitest";
import { appendChannelDelta, hasRenderableContent, parseMessageContent, partsToPlainText } from "./messageContent";

describe("messageContent", () => {
  it("appends by channel and upgrades legacy text", () => {
    let content = "";
    content = appendChannelDelta(content, "thinking", "t1");
    content = appendChannelDelta(content, "final", "hello");
    content = appendChannelDelta(content, "final", " world");
    const doc = parseMessageContent(content);
    expect(doc?.parts).toHaveLength(2);
    expect(partsToPlainText(content)).toBe("hello world");
    expect(hasRenderableContent(content)).toBe(true);

    const upgraded = appendChannelDelta("legacy", "artifact", " x");
    expect(parseMessageContent(upgraded)?.parts[0]).toEqual({ channel: "final", text: "legacy" });

    const replaced = appendChannelDelta(content, "final", "ONLY", true);
    expect(partsToPlainText(replaced)).toBe("ONLY");
  });
});
