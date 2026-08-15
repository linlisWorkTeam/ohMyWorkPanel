import { describe, expect, it, beforeEach } from "vitest";
import {
  applyAgentModelsPayload,
  defaultModelForAdapter,
  modelsForAdapter,
  _resetLiveOverlaysForTests,
} from "./agentModels";

describe("agentModels", () => {
  beforeEach(() => {
    _resetLiveOverlaysForTests();
  });

  it("lists chatbot and cli models", () => {
    expect(modelsForAdapter("chatbot-deepseek")).toContain("deepseek-v4-flash");
    expect(modelsForAdapter("codex")).toContain("deepseek-v4-flash");
    expect(defaultModelForAdapter("codex")).toBe("deepseek-v4-flash");
    expect(modelsForAdapter("codex").length).toBeGreaterThan(0);
    expect(defaultModelForAdapter("cursor")).toBe("auto");
  });

  it("includes cursor grok and kimi-k3 ids from CLI", () => {
    const cursor = modelsForAdapter("cursor");
    expect(cursor).toContain("cursor-grok-4.6-high-fast");
    expect(cursor).toContain("cursor-grok-4.6-xhigh");
    expect(cursor).toContain("cursor-grok-4.5-high");
    expect(cursor).toContain("kimi-k3-max");
    expect(cursor).toContain("kimi-k3-high");
  });

  it("applies live cursor overlay from server payload", () => {
    applyAgentModelsPayload({
      adapters: {
        cursor: ["auto", "cursor-grok-9.9-future"],
        codex: ["deepseek-v4-flash"],
      },
    });
    expect(modelsForAdapter("cursor")).toEqual(["auto", "cursor-grok-9.9-future"]);
    expect(modelsForAdapter("codex")).toEqual(["deepseek-v4-flash"]);
  });
});
