import { describe, expect, it } from "vitest";
import { boundWorkForItem, orchCursorLabel, orchStartBlockers, orchDisplayLabel, activeOrchAlerts, roadmapProgress, seedChecklistTitles, pickDefaultAssigneeId, agentOrchLane, checklistPct } from "./roadmapUi";
import type { Feature, FeatureTask, RoadmapItem, RoadmapOrchestration } from "./types";

const items: RoadmapItem[] = [
  { id: "a", groupId: "g", title: "A", description: "", status: "done", priority: "p1", targetDate: null, sortOrder: 0, createdAt: 1 },
  { id: "b", groupId: "g", title: "B", description: "", status: "in_progress", priority: "p1", targetDate: null, sortOrder: 1, createdAt: 2 },
];

const features: Feature[] = [
  {
    id: "f1", groupId: "g", title: "Feat", description: "", status: "todo", priority: "p1", area: "",
    assigneeMemberId: null, targetRoadmapItemId: "b", sortOrder: 0, createdAt: 1, updatedAt: 1,
  },
];

const tasks: FeatureTask[] = [
  { id: "t1", featureId: "f1", title: "Task one", done: false, sortOrder: 0, createdAt: 1 },
  { id: "t2", featureId: "f1", title: "Task two", done: true, sortOrder: 1, createdAt: 2 },
];

describe("roadmapUi", () => {
  it("computes progress", () => {
    expect(roadmapProgress(items)).toEqual({ done: 1, total: 2, pct: 50 });
    expect(roadmapProgress([])).toEqual({ done: 0, total: 0, pct: 0 });
    expect(checklistPct(1, 4)).toBe(25);
    expect(checklistPct(0, 0)).toBe(0);
  });

  it("binds checklist work to roadmap item", () => {
    const w = boundWorkForItem("b", features, tasks);
    expect(w.totalTasks).toBe(2);
    expect(w.doneTasks).toBe(1);
    expect(boundWorkForItem("a", features, tasks).totalTasks).toBe(0);
  });

  it("formats orchestration cursor", () => {
    const orch: RoadmapOrchestration = {
      id: "o", groupId: "g", roadmapItemId: "b", status: "running",
      cursorFeatureId: "f1", cursorTaskId: "t1", currentRunId: "r",
      errorMessage: null, createdAt: 1, updatedAt: 1,
    };
    expect(orchCursorLabel(orch, features, tasks)).toBe("Feat · Task one");
    expect(orchCursorLabel(undefined, features, tasks)).toBeNull();
  });

  it("lists start blockers for unbound or unassigned work", () => {
    expect(orchStartBlockers("a", features, tasks, true)).toEqual(["没有功能绑定到该路线图项"]);
    expect(orchStartBlockers("b", features, tasks, false)).toEqual([
      "未指派 Agent，且群未设置管理员 Agent",
    ]);
    expect(orchStartBlockers("b", features, tasks, true)).toEqual([]);
  });

  it("labels failed vs manual pause", () => {
    const failed: RoadmapOrchestration = {
      id: "o", groupId: "g", roadmapItemId: "b", status: "failed",
      cursorFeatureId: "f1", cursorTaskId: "t1", currentRunId: null,
      errorMessage: "exit 1", createdAt: 1, updatedAt: 1,
    };
    const manual: RoadmapOrchestration = {
      ...failed, status: "paused", errorMessage: "已手动暂停。",
    };
    expect(orchDisplayLabel(failed)).toEqual({ label: "失败暂停", tone: "failed" });
    expect(orchDisplayLabel(manual)).toEqual({ label: "已暂停", tone: "paused" });
    expect(activeOrchAlerts(items, new Map([["b", failed]]))).toHaveLength(1);
  });

  it("seeds checklist titles and picks assignee", () => {
    const seed = seedChecklistTitles("灰度发布");
    expect(seed.featureTitle).toContain("灰度发布");
    expect(seed.tasks).toHaveLength(3);
    expect(pickDefaultAssigneeId(
      [{ id: "a1", kind: "agent", isActive: true }, { id: "u", kind: "user", isActive: true }],
      "a1",
    )).toBe("a1");
    expect(pickDefaultAssigneeId(
      [{ id: "a2", kind: "agent", isActive: true }],
      null,
    )).toBe("a2");
  });

  it("attributes orch lane to agent via run or assignee", () => {
    const orch: RoadmapOrchestration = {
      id: "o", groupId: "g", roadmapItemId: "b", status: "running",
      cursorFeatureId: "f1", cursorTaskId: "t1", currentRunId: "r1",
      errorMessage: null, createdAt: 1, updatedAt: 1,
    };
    const withAssignee = [{ ...features[0], assigneeMemberId: "agent-x" }];
    expect(agentOrchLane("agent-x", [orch], items, withAssignee, tasks, [{ id: "r1", agentMemberId: "other" }])?.roadmapTitle).toBe("B");
    expect(agentOrchLane("runner", [orch], items, features, tasks, [{ id: "r1", agentMemberId: "runner" }])?.statusLabel).toBe("执行中");
    expect(agentOrchLane("nobody", [orch], items, features, tasks, [])).toBeNull();
  });
});
