//! 前端内核契约测试：与 Rust 侧 `kernel::*` 的单测一一对应。

import { describe, expect, it } from "vitest";

import { Context, KernelError } from "./context";
import { EventBus } from "./events";
import { loadPlugins, type PluginSpec } from "./plugin";

describe("Context", () => {
  it("provides and requires a service by key", () => {
    const ctx = new Context();
    ctx.provide("svc.a", { value: 42 });

    expect(ctx.require<{ value: number }>("svc.a").value).toBe(42);
    expect(ctx.has("svc.a")).toBe(true);
  });

  it("rejects duplicate keys and unknown keys", () => {
    const ctx = new Context();
    ctx.provide("svc.a", 1);

    expect(() => ctx.provide("svc.a", 2)).toThrow(KernelError);
    expect(() => ctx.require("svc.b")).toThrow(/服务不存在/);
  });

  it("reports every missing dependency at once", () => {
    const ctx = new Context();
    ctx.provide("svc.a", 1);

    expect(ctx.missing(["svc.a", "svc.b", "svc.c"])).toEqual(["svc.b", "svc.c"]);
    try {
      ctx.inject(["svc.a", "svc.b", "svc.c"]);
      throw new Error("should not reach here");
    } catch (error) {
      expect(error).toBeInstanceOf(KernelError);
      expect((error as KernelError).keys).toEqual(["svc.b", "svc.c"]);
    }
  });

  it("runs effects in reverse order exactly once on dispose", () => {
    const ctx = new Context();
    const order: string[] = [];
    const first = ctx.effect(() => order.push("first"));
    const second = ctx.effect(() => order.push("second"));

    ctx.dispose();
    ctx.dispose();

    expect(order).toEqual(["second", "first"]);
    expect(first.released).toBe(true);
    expect(second.released).toBe(true);
    expect(ctx.isDisposed()).toBe(true);
    expect(() => ctx.provide("svc.late", 1)).toThrow(/上下文已释放/);
  });
});

describe("EventBus", () => {
  it("emit awaits handlers in registration order", async () => {
    const bus = new EventBus();
    const order: string[] = [];
    bus.on("topic", async () => {
      await Promise.resolve();
      order.push("first");
    });
    bus.on("topic", () => {
      order.push("second");
    });

    const count = await bus.emit("topic", { n: 1 });

    expect(count).toBe(2);
    expect(order).toEqual(["first", "second"]);
  });

  it("serial collects results, parallel waits for all", async () => {
    const bus = new EventBus();
    bus.on("topic", (payload) => ({ doubled: (payload as number) * 2 }));
    bus.on("topic", () => undefined);

    expect(await bus.serial("topic", 2)).toEqual([{ doubled: 4 }, undefined]);
    expect(await bus.parallel("topic", 3)).toEqual([{ doubled: 6 }, undefined]);
  });

  it("bail stops at the first defined result", async () => {
    const bus = new EventBus();
    const seen: string[] = [];
    bus.on("topic", () => {
      seen.push("first");
      return undefined;
    });
    bus.on("topic", () => {
      seen.push("second");
      return "stop";
    });
    bus.on("topic", () => {
      seen.push("third");
      return "later";
    });

    expect(await bus.bail("topic", undefined)).toBe("stop");
    expect(seen).toEqual(["first", "second"]);
  });

  it("off removes a single handler", () => {
    const bus = new EventBus();
    const sub = bus.on("topic", () => undefined);
    bus.on("topic", () => undefined);

    expect(bus.handlerCount("topic")).toBe(2);
    expect(bus.off(sub)).toBe(true);
    expect(bus.handlerCount("topic")).toBe(1);
  });
});

function recorder(): string[] {
  return [];
}

describe("loadPlugins", () => {
  it("loads dependencies before consumers", () => {
    const ctx = new Context();
    const order = recorder();
    const provider: PluginSpec = {
      id: "provider",
      provides: ["svc.a"],
      inject: [],
      install: (target) => target.provide("svc.a", 1),
    };
    const consumer: PluginSpec = {
      id: "consumer",
      provides: [],
      inject: ["svc.a"],
      install: () => order.push("consumer"),
    };

    const report = loadPlugins(ctx, [consumer, provider]);

    expect(report.loaded).toEqual(["provider", "consumer"]);
    expect(order).toEqual(["consumer"]);
  });

  it("keeps input order for independent plugins", () => {
    const ctx = new Context();
    const order = recorder();
    const first: PluginSpec = {
      id: "first",
      provides: [],
      inject: [],
      install: () => order.push("first"),
    };
    const second: PluginSpec = {
      id: "second",
      provides: [],
      inject: [],
      install: () => order.push("second"),
    };

    const report = loadPlugins(ctx, [first, second]);

    expect(report.loaded).toEqual(["first", "second"]);
    expect(order).toEqual(["first", "second"]);
  });

  it("reports a missing dependency and disposes the context", () => {
    const ctx = new Context();
    const spec: PluginSpec = { id: "needy", provides: [], inject: ["nope"], install: () => {} };

    expect(() => loadPlugins(ctx, [spec])).toThrow(/缺少依赖/);
    expect(ctx.isDisposed()).toBe(true);
  });

  it("rejects duplicate ids", () => {
    const ctx = new Context();
    const make = (): PluginSpec => ({ id: "dup", provides: [], inject: [], install: () => {} });

    try {
      loadPlugins(ctx, [make(), make()]);
      throw new Error("should not reach here");
    } catch (error) {
      expect(error).toMatchObject({ kind: "duplicate_id", plugin: "dup" });
    }
  });

  it("disposes the context when install throws", () => {
    const ctx = new Context();
    const spec: PluginSpec = {
      id: "boom",
      provides: [],
      inject: [],
      install: () => {
        throw new Error("kaboom");
      },
    };

    expect(() => loadPlugins(ctx, [spec])).toThrow(/kaboom/);
    expect(ctx.isDisposed()).toBe(true);
  });

  it("rejects a plugin that declares but never registers a service", () => {
    const ctx = new Context();
    const spec: PluginSpec = {
      id: "liar",
      provides: ["svc.missing"],
      inject: [],
      install: () => {},
    };

    try {
      loadPlugins(ctx, [spec]);
      throw new Error("should not reach here");
    } catch (error) {
      expect(error).toMatchObject({
        kind: "provides_not_registered",
        plugin: "liar",
        missing: ["svc.missing"],
      });
    }
    expect(ctx.isDisposed()).toBe(true);
  });

  it("reports cyclic dependencies", () => {
    const ctx = new Context();
    const a: PluginSpec = { id: "a", provides: ["svc.a"], inject: ["svc.b"], install: () => {} };
    const b: PluginSpec = { id: "b", provides: ["svc.b"], inject: ["svc.a"], install: () => {} };

    try {
      loadPlugins(ctx, [a, b]);
      throw new Error("should not reach here");
    } catch (error) {
      expect(error).toMatchObject({ kind: "cycle" });
    }
    expect(ctx.isDisposed()).toBe(true);
  });
});

describe("fork", () => {
  it("inherits parent services", () => {
    const root = new Context();
    root.provide("svc.a", 7);
    const child = root.fork();

    expect(child.require<number>("svc.a")).toBe(7);
    expect(child.inject(["svc.a"])).toBeUndefined();
  });

  it("may shadow a parent service without conflicting", () => {
    const root = new Context();
    root.provide("svc.a", "parent");
    const child = root.fork();
    child.provide("svc.a", "child");

    expect(child.require<string>("svc.a")).toBe("child");
    expect(root.require<string>("svc.a")).toBe("parent");
  });

  it("disposes only its own effects", () => {
    const root = new Context();
    root.provide("svc.a", 1);
    const order: string[] = [];
    const child = root.fork();
    child.effect(() => order.push("child"));

    child.dispose();

    expect(child.isDisposed()).toBe(true);
    expect(order).toEqual(["child"]);
    expect(root.require<number>("svc.a")).toBe(1);
  });

  it("stops serving children once the parent is disposed", () => {
    const root = new Context();
    root.provide("svc.a", 1);
    const child = root.fork();

    root.dispose();

    expect(() => child.require("svc.a")).toThrow(/服务不存在/);
    expect(child.missing(["svc.a"])).toEqual(["svc.a"]);
  });
});
