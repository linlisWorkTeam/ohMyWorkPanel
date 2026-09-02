import { describe, expect, it } from "vitest";
import { parseMarketingMarker } from "./markers";

describe("Self-Marketing message markers", () => {
  it("parses campaign and internal markers", () => {
    expect(parseMarketingMarker("[[MARKETING_CAMPAIGN:c-1]]")).toEqual({ kind: "campaign", campaignId: "c-1" });
    expect(parseMarketingMarker("[[MARKETING_INTERNAL:c-1:planning]]")).toEqual({ kind: "internal", campaignId: "c-1", stage: "planning" });
  });

  it("parses final channel message documents", () => {
    const content = JSON.stringify({ v: 1, parts: [{ channel: "final", text: "[[MARKETING_CAMPAIGN:c-2]]" }] });
    expect(parseMarketingMarker(content)).toEqual({ kind: "campaign", campaignId: "c-2" });
  });
});
