import { describe, expect, it } from "vitest";
import { BASE_RIGHT_TABS, coreRegistry, effectiveMotion, listContributions } from "./registry";
import { SLASH_COMMANDS } from "./slash";
import { parseDockGeom, shouldCollapseDock } from "./dockGeom";
import type { UiContribution } from "./types";

describe("contrib registry", () => {
  it("keeps Base right-tab ids stable", () => {
    expect(BASE_RIGHT_TABS.map((tab) => tab.id)).toEqual([
      "core.members",
      "core.queue",
      "core.details",
      "core.settings",
    ]);
    expect(coreRegistry().every((item) => item.origin === "base")).toBe(true);
  });

  it("appends fake Extend right-tab without changing Base ids", () => {
    const extra: UiContribution = {
      id: "ext.panellive.live",
      title: "Live",
      slot: "right-tab",
      origin: "extend",
      dockable: true,
      order: 80,
    };
    const tabs = listContributions([extra], "right-tab");
    expect(tabs.map((tab) => tab.id)).toEqual([
      "core.members",
      "core.queue",
      "core.details",
      "core.settings",
      "ext.panellive.live",
    ]);
  });

  it("ignores ambient motion on minimal and reduced-motion", () => {
    const ambient: UiContribution = {
      id: "ext.glow",
      title: "glow",
      slot: "status",
      origin: "extend",
      motion: "ambient",
    };
    expect(effectiveMotion(ambient, "cyberpunk", false)).toBe("ambient");
    expect(effectiveMotion(ambient, "minimal", false)).toBe("none");
    expect(effectiveMotion(ambient, "cyberpunk", true)).toBe("none");
  });

  it("keeps the supported slash command registry explicit", () => {
    expect(SLASH_COMMANDS.map((row) => row.cmd)).toEqual([
      "/board",
      "/approve",
      "/wave",
      "/market",
    ]);
  });

  it("parses dock geometry and collapses when the pane is too narrow", () => {
    expect(parseDockGeom(null)).toEqual({ dockedId: null, width: 260 });
    expect(parseDockGeom('{"dockedId":"ext:panellive:live","width":300}')).toEqual({
      dockedId: "ext:panellive:live",
      width: 300,
    });
    expect(shouldCollapseDock(400)).toBe(true);
    expect(shouldCollapseDock(520)).toBe(false);
  });
});
