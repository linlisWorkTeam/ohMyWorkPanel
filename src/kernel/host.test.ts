//! 前端内核宿主契约测试：内置插件装配、fail-open 降级、场景能力解析。

import { describe, expect, it } from "vitest";

import { builtinFrontendPlugins } from "./builtins";
import { createFrontendKernel, frontendKernel, resetFrontendKernel } from "./host";
import type { PluginSpec } from "./plugin";

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
});
