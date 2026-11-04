import { describe, expect, it, vi } from "vitest";

import { bootSession, type BootContext } from "./bootSession";
import type { Group } from "../types";

const group = (id: string): Group => ({
  id,
  name: id,
  workspacePath: `/tmp/${id}`,
  ownerMemberId: "u1",
  adminMemberId: null,
  createdAt: 1,
});

function context(overrides: Partial<BootContext> = {}): { ctx: BootContext; calls: string[] } {
  const calls: string[] = [];
  const ctx: BootContext = {
    api: {
      verify: vi.fn(async () => ({ sub: "u1", username: "root", isAdmin: true })),
      bootstrap: vi.fn(async () => ({ groups: [group("g1")] })),
      listPresence: vi.fn(async () => ({ onlineUserIds: ["u1"] })),
      getSettings: vi.fn(async () => ({
        maxConcurrentRuns: 2,
        runTimeoutSeconds: 600,
        contextMessageLimit: 20,
        maxDelegationDepth: 2,
      })),
      getAgentModels: vi.fn(async () => ({ adapters: { cursor: ["m"] } })),
      getPresetRoles: vi.fn(async () => []),
    },
    refresh: vi.fn(async () => {
      calls.push("refresh");
    }),
    isDisposed: () => false,
    onUser: () => calls.push("user"),
    onGroups: () => calls.push("groups"),
    onOnline: () => calls.push("online"),
    onFirstGroup: () => calls.push("first"),
    onFirstGroupLoaded: () => calls.push("firstLoaded"),
    onEmpty: () => calls.push("empty"),
    onSettings: () => calls.push("settings"),
    onModels: () => calls.push("models"),
    onPresetRoles: () => calls.push("preset"),
    onReady: () => calls.push("ready"),
    ...overrides,
  };
  return { ctx, calls };
}

describe("bootSession", () => {
  it("runs the web order: user → groups → presence → refresh → settings → models → preset → ready", async () => {
    const { ctx, calls } = context();
    await bootSession(ctx, { verify: true, presence: true, models: true });
    expect(calls).toEqual([
      "user",
      "groups",
      "online",
      "first",
      "refresh",
      "firstLoaded",
      "settings",
      "models",
      "preset",
      "ready",
    ]);
  });

  it("skips verify/presence/models for desktop options", async () => {
    const { ctx, calls } = context();
    await bootSession(ctx, { verify: false, presence: false, models: false });
    expect(calls).toEqual(["groups", "first", "refresh", "firstLoaded", "settings", "preset", "ready"]);
    expect(ctx.api.verify).not.toHaveBeenCalled();
    expect(ctx.api.listPresence).not.toHaveBeenCalled();
  });

  it("calls onEmpty when there is no group", async () => {
    const { ctx, calls } = context({
      api: {
        bootstrap: vi.fn(async () => ({ groups: [] })),
        getSettings: vi.fn(async () => ({
          maxConcurrentRuns: 2,
          runTimeoutSeconds: 600,
          contextMessageLimit: 20,
          maxDelegationDepth: 2,
        })),
        getPresetRoles: vi.fn(async () => []),
      },
    });
    await bootSession(ctx, { verify: false, presence: false, models: false });
    expect(calls).toContain("empty");
    expect(calls).not.toContain("refresh");
  });

  it("stops when disposed after bootstrap", async () => {
    const { ctx, calls } = context({ isDisposed: () => true });
    await bootSession(ctx, { verify: false, presence: false, models: false });
    expect(calls).toEqual([]);
  });

  it("propagates bootstrap errors", async () => {
    const { ctx } = context({
      api: {
        bootstrap: vi.fn(async () => {
          throw new Error("boom");
        }),
        getSettings: vi.fn(async () => ({
          maxConcurrentRuns: 2,
          runTimeoutSeconds: 600,
          contextMessageLimit: 20,
          maxDelegationDepth: 2,
        })),
        getPresetRoles: vi.fn(async () => []),
      },
    });
    await expect(bootSession(ctx, { verify: false, presence: false, models: false })).rejects.toThrow("boom");
  });
});
