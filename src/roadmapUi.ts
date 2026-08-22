import type { Feature, FeatureTask, RoadmapItem, RoadmapOrchestration } from "./types";

export const ORCH_STATUS_LABEL: Record<string, string> = {
  running: "执行中",
  paused: "已暂停",
  failed: "失败",
  completed: "已完成",
  cancelled: "已取消",
};

export function roadmapProgress(items: RoadmapItem[]): { done: number; total: number; pct: number } {
  const total = items.length;
  const done = items.filter((i) => i.status === "done").length;
  const pct = total === 0 ? 0 : Math.round((done / total) * 100);
  return { done, total, pct };
}

export function checklistPct(done: number, total: number): number {
  if (total <= 0) return 0;
  return Math.round((done / total) * 100);
}

export function boundWorkForItem(
  itemId: string,
  features: Feature[],
  tasks: FeatureTask[],
): { features: Feature[]; tasks: FeatureTask[]; doneTasks: number; totalTasks: number } {
  const bound = features
    .filter((f) => f.targetRoadmapItemId === itemId)
    .sort((a, b) => a.sortOrder - b.sortOrder);
  const ids = new Set(bound.map((f) => f.id));
  const boundTasks = tasks
    .filter((t) => ids.has(t.featureId))
    .sort((a, b) => a.sortOrder - b.sortOrder || a.createdAt - b.createdAt);
  const doneTasks = boundTasks.filter((t) => t.done).length;
  return { features: bound, tasks: boundTasks, doneTasks, totalTasks: boundTasks.length };
}

export function orchCursorLabel(
  orch: RoadmapOrchestration | undefined,
  features: Feature[],
  tasks: FeatureTask[],
): string | null {
  if (!orch?.cursorFeatureId && !orch?.cursorTaskId) return null;
  const feature = features.find((f) => f.id === orch.cursorFeatureId);
  const task = tasks.find((t) => t.id === orch.cursorTaskId);
  if (feature && task) return `${feature.title} · ${task.title}`;
  if (task) return task.title;
  if (feature) return feature.title;
  return null;
}

/** Why「启动」编排可能失败 / 应禁用 — 供 UI 提示。 */
export function orchStartBlockers(
  itemId: string,
  features: Feature[],
  tasks: FeatureTask[],
  hasAdminAgent: boolean,
): string[] {
  const work = boundWorkForItem(itemId, features, tasks);
  const blockers: string[] = [];
  if (work.features.length === 0) {
    blockers.push("没有功能绑定到该路线图项");
    return blockers;
  }
  if (work.totalTasks === 0) {
    blockers.push("已绑定功能但还没有 checklist 子任务");
    return blockers;
  }
  if (work.doneTasks === work.totalTasks) {
    blockers.push("checklist 已全部完成");
    return blockers;
  }
  const openFeatures = work.features.filter((f) =>
    work.tasks.some((t) => t.featureId === f.id && !t.done),
  );
  const missingAssignee = openFeatures.filter((f) => !f.assigneeMemberId);
  if (missingAssignee.length > 0 && !hasAdminAgent) {
    blockers.push("未指派 Agent，且群未设置管理员 Agent");
  }
  return blockers;
}

/** Human label + tone for orchestration chip / banner. */
export function orchDisplayLabel(orch: RoadmapOrchestration): {
  label: string;
  tone: "running" | "paused" | "failed" | "done" | "idle";
} {
  if (orch.status === "running") return { label: "执行中", tone: "running" };
  if (orch.status === "failed") return { label: "失败暂停", tone: "failed" };
  if (orch.status === "completed") return { label: "已完成", tone: "done" };
  if (orch.status === "cancelled") return { label: "已取消", tone: "idle" };
  if (orch.status === "paused") {
    const msg = orch.errorMessage ?? "";
    if (msg && msg !== "已手动暂停。" && msg !== "已手动暂停") {
      return { label: "失败暂停", tone: "failed" };
    }
    return { label: "已暂停", tone: "paused" };
  }
  return { label: ORCH_STATUS_LABEL[orch.status] ?? orch.status, tone: "idle" };
}

export function activeOrchAlerts(
  items: RoadmapItem[],
  orchByItem: Map<string, RoadmapOrchestration>,
): { itemTitle: string; message: string }[] {
  const alerts: { itemTitle: string; message: string }[] = [];
  for (const item of items) {
    const orch = orchByItem.get(item.id);
    if (!orch) continue;
    const { tone, label } = orchDisplayLabel(orch);
    if (tone !== "failed") continue;
    alerts.push({
      itemTitle: item.title,
      message: orch.errorMessage ? `${label}：${orch.errorMessage}` : label,
    });
  }
  return alerts;
}

/** Default checklist titles for one-click seed under a roadmap item. */
export function seedChecklistTitles(roadmapTitle: string): { featureTitle: string; tasks: string[] } {
  const name = roadmapTitle.trim() || "路线图项";
  return {
    featureTitle: `${name} · 实施`,
    tasks: [
      `梳理现状与验收标准（${name}）`,
      `实现核心改动并跑通测试门禁`,
      `灰度验证并记录结果`,
    ],
  };
}

export function pickDefaultAssigneeId(
  members: { id: string; kind: string; isActive: boolean }[],
  adminMemberId: string | null | undefined,
): string | undefined {
  if (adminMemberId) {
    const admin = members.find((m) => m.id === adminMemberId && m.isActive);
    if (admin) return admin.id;
  }
  const first = members.find((m) => m.isActive && (m.kind === "agent" || m.kind === "chatbot"));
  return first?.id;
}

export type AgentOrchLane = {
  roadmapTitle: string;
  cursor: string | null;
  statusLabel: string;
  tone: "running" | "paused" | "failed" | "done" | "idle";
};

/** Active orchestration work currently attributed to an agent (by run or feature assignee). */
export function agentOrchLane(
  agentId: string,
  orchestrations: RoadmapOrchestration[],
  roadmap: RoadmapItem[],
  features: Feature[],
  tasks: FeatureTask[],
  runs: { id: string; agentMemberId: string }[],
): AgentOrchLane | null {
  const active = orchestrations.filter((o) =>
    o.status === "running" || o.status === "paused" || o.status === "failed",
  );
  for (const orch of active) {
    const runHit = orch.currentRunId
      ? runs.some((r) => r.id === orch.currentRunId && r.agentMemberId === agentId)
      : false;
    const feature = features.find((f) => f.id === orch.cursorFeatureId);
    const assigneeHit = feature?.assigneeMemberId === agentId;
    if (!runHit && !assigneeHit) continue;
    const item = roadmap.find((r) => r.id === orch.roadmapItemId);
    const { label, tone } = orchDisplayLabel(orch);
    return {
      roadmapTitle: item?.title ?? "路线图",
      cursor: orchCursorLabel(orch, features, tasks),
      statusLabel: label,
      tone,
    };
  }
  return null;
}
