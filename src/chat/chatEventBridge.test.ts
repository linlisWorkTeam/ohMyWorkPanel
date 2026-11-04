import { describe, expect, it, vi } from "vitest";

import { dispatchChatEvent, type ChatEventBridgeDeps } from "./chatEventBridge";
import type { ChatEvent, GroupState } from "../types";

function event(partial: Partial<ChatEvent> & { kind: string }): ChatEvent {
  return {
    groupId: "g1",
    runId: null,
    messageId: null,
    delta: null,
    status: null,
    error: null,
    ...partial,
  } as ChatEvent;
}

function state(overrides: Partial<GroupState> = {}): GroupState {
  return {
    group: {
      id: "g1",
      name: "g1",
      workspacePath: "/tmp/g1",
      ownerMemberId: "u1",
      adminMemberId: null,
      createdAt: 1,
    },
    members: [],
    messages: [
      {
        id: "m1",
        groupId: "g1",
        senderMemberId: "a1",
        parentRunId: null,
        content: "",
        status: "streaming",
        createdAt: 2,
      },
    ],
    runs: [
      {
        id: "r1",
        groupId: "g1",
        rootMessageId: "m1",
        agentMemberId: "a1",
        parentRunId: null,
        depth: 0,
        status: "running",
        outputMessageId: "m1",
        errorMessage: null,
        reviewStatus: null,
        reviewerMemberId: null,
        createdAt: 2,
        startedAt: 3,
        completedAt: null,
      },
    ],
    ...overrides,
  };
}

function deps(overrides: Partial<ChatEventBridgeDeps> = {}) {
  let current: GroupState | null = state();
  let online = new Set<string>(["u1"]);
  const calls: string[] = [];
  const base: ChatEventBridgeDeps = {
    getActiveGroupId: () => "g1",
    setOnline: (updater) => {
      online = updater(online);
      calls.push("online");
    },
    bumpUnread: () => calls.push("unread"),
    setCurrent: (updater) => {
      current = updater(current);
    },
    refresh: vi.fn(async () => {
      calls.push("refresh");
    }),
    resync: vi.fn(async () => {
      calls.push("resync");
    }),
    reportError: () => calls.push("error"),
    ...overrides,
  };
  return { deps: base, calls, getCurrent: () => current, getOnline: () => online };
}

describe("dispatchChatEvent", () => {
  it("applies presence to the online set", () => {
    const harness = deps();
    dispatchChatEvent(event({ kind: "presence", userId: "u2", online: true }), harness.deps);
    expect(harness.getOnline().has("u2")).toBe(true);
    expect(harness.calls).toContain("online");
  });

  it("bumps unread and falls through to refresh on message_created", () => {
    const harness = deps();
    dispatchChatEvent(event({ kind: "message_created" }), harness.deps);
    expect(harness.calls).toContain("unread");
    expect(harness.calls).toContain("refresh");
  });

  it("refreshes the active group on orchestration status", () => {
    const harness = deps();
    dispatchChatEvent(event({ kind: "orchestration_status" }), harness.deps);
    expect(harness.calls).toEqual(["refresh"]);
  });

  it("runs full resync on reconnect", () => {
    const harness = deps();
    dispatchChatEvent(event({ kind: "ws_reconnected" }), harness.deps);
    expect(harness.calls).toEqual(["resync"]);
  });

  it("applies deltas and asks for refresh when the message is missing", () => {
    const harness = deps();
    dispatchChatEvent(event({ kind: "message_delta", messageId: "m1", channel: "final", delta: "hi" }), harness.deps);
    expect(harness.getCurrent()?.messages[0].content).toContain("hi");
    expect(harness.calls).not.toContain("refresh");

    dispatchChatEvent(event({ kind: "message_delta", messageId: "missing", delta: "x" }), harness.deps);
    expect(harness.calls).toContain("refresh");
  });

  it("updates heartbeat without a refresh", () => {
    const harness = deps();
    dispatchChatEvent(event({ kind: "run_heartbeat", runId: "r1", status: "running" }), harness.deps);
    expect(harness.getCurrent()?.runs[0].status).toBe("running");
    expect(harness.calls).toEqual([]);
  });

  it("refreshes after a terminal run status", () => {
    const harness = deps();
    dispatchChatEvent(event({ kind: "run_status", runId: "r1", status: "completed" }), harness.deps);
    expect(harness.getCurrent()?.runs[0].status).toBe("completed");
    expect(harness.calls).toContain("refresh");
  });
});
