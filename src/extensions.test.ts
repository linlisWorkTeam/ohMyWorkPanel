import { describe, expect, it } from "vitest";
import {
  collectExtensionTabViews,
  extMainViewKey,
  extensionEntryUrl,
  liveEntryUrl,
  liveTabEnabled,
  panelliveStatus,
  parseExtMainView,
  tabPeerAllowed,
} from "./extensions";
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

  it("tolerates missing tabs/baseUrl without throwing", () => {
    expect(liveEntryUrl(ext({ tabs: undefined as unknown as ExtensionStatus["tabs"] }))).toBeNull();
    expect(liveEntryUrl(ext({ baseUrl: "" }))).toBeNull();
  });

  it("builds generic entry urls and view keys", () => {
    const hotel = ext({
      id: "ai-hotel",
      name: "AI 酒馆",
      baseUrl: "/api/extensions/ai-hotel",
      tabs: [{ id: "tavern", title: "酒馆", route: "tab://tavern", entry: "tavern.html", peerOf: ["chat"] }],
    });
    expect(extensionEntryUrl(hotel, hotel.tabs[0])).toBe("/api/extensions/ai-hotel/tavern.html");
    expect(extMainViewKey("ai-hotel", "tavern")).toBe("ext:ai-hotel:tavern");
    expect(parseExtMainView("ext:ai-hotel:tavern")).toEqual({ extId: "ai-hotel", tabId: "tavern" });
  });

  it("collects tabs for header rendering", () => {
    const views = collectExtensionTabViews(
      [
        ext({}),
        ext({
          id: "ai-hotel",
          name: "AI 酒馆",
          baseUrl: "/api/extensions/ai-hotel",
          enabled: false,
          tabs: [{ id: "tavern", title: "酒馆", route: "tab://tavern", entry: "/tavern.html", peerOf: ["chat"] }],
        }),
      ],
      "chat",
    );
    expect(views.map((v) => v.tab.title)).toEqual(["Live", "酒馆"]);
    expect(tabPeerAllowed({ id: "x", title: "x", route: "r", entry: "/x", peerOf: ["project"] }, "chat")).toBe(false);
  });
});
