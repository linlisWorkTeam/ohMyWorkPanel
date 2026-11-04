//! Goal 领域的中文标签与格式化（页面与 Goal 条共用）。

import type { GoalRoundVerdict, GoalStatus, GoalStopReason } from "../types";

export const GOAL_STATUS_LABEL: Record<GoalStatus, string> = {
  draft: "草稿",
  active: "运行中",
  paused: "已暂停",
  awaiting_approval: "待批准",
  completed: "已完成",
  aborted: "已停止",
};

export const GOAL_STOP_LABEL: Record<GoalStopReason, string> = {
  goal_met: "目标达成",
  rounds_exhausted: "轮次用尽",
  wall_time_exhausted: "超时停止",
  tokens_exhausted: "token 用尽",
  no_progress: "连续无进展",
  consecutive_failures: "连续失败",
  aborted_by_human: "人工停止",
};

export const VERDICT_LABEL: Record<GoalRoundVerdict, string> = {
  goal_met: "达成",
  progress: "进展",
  no_progress: "无进展",
  failed: "失败",
};

/** 时间戳 → 本地短时间；非法值返回 `—`。 */
export function formatTime(ts: number | null | undefined): string {
  if (!ts || !Number.isFinite(ts)) return "—";
  return new Date(ts).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** 毫秒 → 人类可读时长。 */
export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "0s";
  const totalSeconds = Math.round(ms / 1000);
  if (totalSeconds < 60) return `${totalSeconds}s`;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes < 60) return seconds > 0 ? `${minutes}m${seconds}s` : `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h${minutes % 60}m`;
}
