import { describe, expect, it } from "vitest";
import { agentBusyLabel, queueCounts, runsForAgentActive } from "./queueCounts";
import type { TaskRun } from "./types";

function run(partial: Partial<TaskRun> & Pick<TaskRun, "id" | "agentMemberId" | "status">): TaskRun {
  return {
    groupId: "g",
    rootMessageId: "m",
    parentRunId: null,
    depth: 0,
    outputMessageId: null,
    errorMessage: null,
    reviewStatus: "pending",
    reviewerMemberId: null,
    createdAt: 1,
    startedAt: null,
    completedAt: null,
    phase: null,
    phaseUpdatedAt: null,
    ...partial,
  };
}

describe("queueCounts", () => {
  it("counts running and queued for one agent", () => {
    const runs = [
      run({ id: "1", agentMemberId: "a", status: "running" }),
      run({ id: "2", agentMemberId: "a", status: "queued" }),
      run({ id: "3", agentMemberId: "a", status: "queued" }),
      run({ id: "4", agentMemberId: "a", status: "completed" }),
    ];
    expect(queueCounts(runs, "a")).toEqual({ running: 1, queued: 2 });
  });

  it("ignores other agents", () => {
    const runs = [
      run({ id: "1", agentMemberId: "a", status: "running" }),
      run({ id: "2", agentMemberId: "b", status: "queued" }),
    ];
    expect(queueCounts(runs, "a")).toEqual({ running: 1, queued: 0 });
    expect(queueCounts(runs, "b")).toEqual({ running: 0, queued: 1 });
  });

  it("empty list is idle", () => {
    expect(queueCounts([], "a")).toEqual({ running: 0, queued: 0 });
  });
});

describe("agentBusyLabel", () => {
  it("formats running with queue", () => {
    expect(agentBusyLabel({ running: 1, queued: 2 })).toBe("执行中 · 排队 2");
  });

  it("formats running only", () => {
    expect(agentBusyLabel({ running: 1, queued: 0 })).toBe("执行中");
  });

  it("formats queued only", () => {
    expect(agentBusyLabel({ running: 0, queued: 3 })).toBe("排队 3");
  });

  it("returns null when idle", () => {
    expect(agentBusyLabel({ running: 0, queued: 0 })).toBeNull();
  });
});

describe("runsForAgentActive", () => {
  it("returns only queued/running for agent, oldest first", () => {
    const runs = [
      run({ id: "new", agentMemberId: "a", status: "queued", createdAt: 30 }),
      run({ id: "done", agentMemberId: "a", status: "completed", createdAt: 5 }),
      run({ id: "run", agentMemberId: "a", status: "running", createdAt: 10 }),
      run({ id: "other", agentMemberId: "b", status: "queued", createdAt: 1 }),
      run({ id: "old", agentMemberId: "a", status: "queued", createdAt: 20 }),
    ];
    expect(runsForAgentActive(runs, "a").map((r) => r.id)).toEqual(["run", "old", "new"]);
  });
});
