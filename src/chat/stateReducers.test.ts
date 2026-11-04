import { describe, expect, it } from "vitest";

import { applyMessageDelta, applyRunStatus, isTerminalRunStatus } from "./stateReducers";
import type { ChatEvent, Group, GroupState, Message, TaskRun } from "../types";

const group: Group = {
  id: "g1",
  name: "测试群",
  workspacePath: "/tmp/g1",
  ownerMemberId: "u1",
  adminMemberId: "a1",
  createdAt: 1,
};

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

function message(overrides: Partial<Message> = {}): Message {
  return {
    id: "m1",
    groupId: "g1",
    senderMemberId: "a1",
    parentRunId: null,
    content: "",
    status: "streaming",
    createdAt: 2,
    ...overrides,
  };
}

function run(overrides: Partial<TaskRun> = {}): TaskRun {
  return {
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
    ...overrides,
  };
}

function state(overrides: Partial<GroupState> = {}): GroupState {
  return {
    group,
    members: [],
    messages: [message()],
    runs: [run()],
    ...overrides,
  };
}

describe("applyMessageDelta", () => {
  it("appends final-channel deltas to the message content", () => {
    const first = applyMessageDelta(
      state(),
      event({ kind: "message_delta", messageId: "m1", channel: "final", delta: "你好", status: "streaming" }),
    );
    expect(first.missing).toBe(false);
    expect(first.state?.messages[0].content).toContain("你好");

    const second = applyMessageDelta(
      first.state,
      event({ kind: "message_delta", messageId: "m1", channel: "final", delta: "世界" }),
    );
    expect(second.state?.messages[0].content).toContain("世界");
  });

  it("replace=true overwrites the channel instead of appending", () => {
    const base = state({
      messages: [message({ content: JSON.stringify({ v: 1, parts: [{ channel: "final", text: "old" }] }) })],
    });
    const result = applyMessageDelta(
      base,
      event({ kind: "message_delta", messageId: "m1", channel: "final", delta: "new", replace: true }),
    );
    expect(result.state?.messages[0].content).toContain("new");
    expect(result.state?.messages[0].content).not.toContain("old");
  });

  it("lazy channels only flip hasThinking/hasArtifact", () => {
    const result = applyMessageDelta(state(), event({ kind: "message_delta", messageId: "m1", channel: "thinking" }));
    expect(result.state?.messages[0].hasThinking).toBe(true);
    expect(result.state?.messages[0].content).toBe("");
    expect(result.state?.messages[0].status).toBe("streaming");
  });

  it("reports missing when the message is not in the snapshot", () => {
    const base = state();
    const result = applyMessageDelta(base, event({ kind: "message_delta", messageId: "missing", delta: "x" }));
    expect(result.missing).toBe(true);
    expect(result.state).toBe(base);
  });

  it("ignores events for another group without a matching run", () => {
    const base = state();
    const result = applyMessageDelta(base, event({ kind: "message_delta", groupId: "other", messageId: "m1", delta: "x" }));
    expect(result.state).toBe(base);
    expect(result.missing).toBe(false);
  });
});

describe("applyRunStatus", () => {
  it("updates the run fields", () => {
    const result = applyRunStatus(
      state(),
      event({ kind: "run_status", runId: "r1", status: "running", phase: "develop" }),
    );
    expect(result.state?.runs[0].phase).toBe("develop");
    expect(result.state?.runs[0].status).toBe("running");
  });

  it("terminal status also completes the streaming output message", () => {
    const result = applyRunStatus(
      state(),
      event({ kind: "run_status", runId: "r1", status: "completed", messageId: "m1" }),
    );
    expect(result.state?.runs[0].status).toBe("completed");
    expect(result.state?.messages[0].status).toBe("completed");
  });

  it("reports missing when the run is unknown", () => {
    const result = applyRunStatus(state(), event({ kind: "run_status", runId: "nope", status: "failed" }));
    expect(result.missing).toBe(true);
  });

  it("classifies terminal statuses", () => {
    expect(isTerminalRunStatus("completed")).toBe(true);
    expect(isTerminalRunStatus("failed")).toBe(true);
    expect(isTerminalRunStatus("running")).toBe(false);
    expect(isTerminalRunStatus(null)).toBe(false);
  });
});
