//! 前端内核宿主契约测试：内置插件装配、fail-open 降级、场景能力解析。

import { describe, expect, it } from "vitest";

import { builtinFrontendPlugins } from "./builtins";
import { createFrontendKernel, frontendKernel, resetFrontendKernel } from "./host";
import type { PluginSpec } from "./plugin";
import { openSceneGroupScope } from "../scene/host";
import { SCENE_GROUP_SERVICE } from "../scene/plugin";
import { roomSeedOf } from "../scene/sceneModel";

const SCENE_SERVICE = "svc.scene";

describe("frontend kernel host", () => {
  it("loads builtin plugins and exposes the scene capability", () => {
    const ctx = createFrontendKernel();

    expect(ctx.isDisposed()).toBe(false);
    const capability = ctx.require<{ View: unknown }>(SCENE_SERVICE);
    expect(typeof capability.View).toBe("function");
  });

  it("falls open to an empty kernel when a plugin fails", () => {
    const broken: PluginSpec = {
      id: "broken",
      provides: [],
      inject: [],
      install: () => {
        throw new Error("boom");
      },
    };

    const ctx = createFrontendKernel([broken]);

    expect(() => ctx.require(SCENE_SERVICE)).toThrow(/服务不存在/);
  });

  it("caches the process kernel until reset", () => {
    resetFrontendKernel();
    const first = frontendKernel();
    const second = frontendKernel();
    expect(first).toBe(second);

    resetFrontendKernel();
    expect(frontendKernel()).not.toBe(first);
  });

  it("declares at least one builtin plugin", () => {
    const ids = builtinFrontendPlugins().map((spec) => spec.id);
    expect(ids.length).toBeGreaterThan(0);
    expect(ids).toContain("builtin.scene");
  });

  it("opens an isolated per-group scene scope via fork", () => {
    const kernel = frontendKernel();

    const first = openSceneGroupScope("group-a");
    const second = openSceneGroupScope("group-b");
    expect(first).not.toBeNull();
    expect(second).not.toBeNull();
    expect(first?.roomSeed).toBe(roomSeedOf("group-a"));
    expect(first?.roomSeed).toBe(roomSeedOf("group-a"));
    expect(first?.groupId).toBe("group-a");

    // 子作用域通过 fork 提供群专属服务，根作用域看不到。
    const scopeService = first as unknown as { groupId: string };
    expect(scopeService.groupId).toBe("group-a");
    expect(() => kernel.require(SCENE_GROUP_SERVICE)).toThrow(/服务不存在/);

    first?.dispose();
    second?.dispose();
  });
});
