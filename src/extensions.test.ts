import { describe, expect, it } from "vitest";
import { liveEntryUrl, liveTabEnabled, panelliveStatus } from "./extensions";
import type { ExtensionStatus } from "./types";

function ext(partial: Partial<ExtensionStatus>): ExtensionStatus {
  return {
    id: "panellive",
    name: "PanelLive",
    version: "0.1.0",
    kind: "extend",
    enabled: true,
    healthy: true,
    healthDetail: "ok",
    baseUrl: "/api/extensions/panellive",
    tabs: [{ id: "live", title: "Live", route: "tab://live", entry: "/live.html", peerOf: ["chat"], disabledWhenUnloaded: true }],
    a2aSkills: ["live.session.start"],
    mediaPlane: "local",
    ...partial,
  };
}

describe("extensions helpers", () => {
  it("resolves panellive and live entry", () => {
    const list = [ext({})];
    expect(panelliveStatus(list)?.id).toBe("panellive");
    expect(liveTabEnabled(list[0])).toBe(true);
    expect(liveEntryUrl(list[0])).toBe("/api/extensions/panellive/live.html");
  });

  it("disables tab when unloaded", () => {
    expect(liveTabEnabled(ext({ enabled: false }))).toBe(false);
    expect(liveEntryUrl(ext({ enabled: false }))).toBeNull();
  });
});
