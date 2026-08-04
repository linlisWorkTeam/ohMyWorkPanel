import { describe, expect, it } from "vitest";
import { defaultModelForAdapter, modelsForAdapter } from "./agentModels";

describe("agentModels", () => {
  it("lists chatbot and cli models", () => {
    expect(modelsForAdapter("chatbot-deepseek")).toContain("deepseek-v4-flash");
    expect(modelsForAdapter("codex")).toContain("deepseek-v4-flash");
    expect(defaultModelForAdapter("codex")).toBe("deepseek-v4-flash");
    expect(modelsForAdapter("codex").length).toBeGreaterThan(0);
    expect(defaultModelForAdapter("cursor")).toBe("auto");
  });

  it("includes cursor grok and kimi-k3 ids from CLI", () => {
    const cursor = modelsForAdapter("cursor");
    expect(cursor).toContain("cursor-grok-4.5-high");
    expect(cursor).toContain("kimi-k3-max");
    expect(cursor).toContain("kimi-k3-high");
  });
});
