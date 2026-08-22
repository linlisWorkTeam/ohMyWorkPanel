import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("ChatTranscript", () => {
  const src = readFileSync(resolve("src/shell/ChatTranscript.tsx"), "utf8");
  const css = readFileSync(resolve("src/shell/tokens.css"), "utf8");

  it("uses mock bubble corners and no agent-reply-fold", () => {
    expect(src).toContain("wp-row");
    expect(src).toContain("data-msg-id");
    expect(src).toContain("wp-bub");
    expect(src).not.toContain("agent-reply-fold");
    expect(src).toContain("run?.errorMessage");
    expect(css).toMatch(/\.wp-row\.me\s+\.wp-bub[\s\S]*border-radius:\s*16px 4px 16px 16px/);
    expect(css).toMatch(/\.wp-stop/);
  });
});
