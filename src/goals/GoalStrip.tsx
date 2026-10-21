import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { Goal } from "../types";

const STATUS_LABEL: Record<Goal["status"], string> = {
  draft: "草稿",
  active: "运行中",
  paused: "已暂停",
  awaiting_approval: "待批准",
  completed: "已完成",
  aborted: "已停止",
};

const STOP_LABEL: Record<NonNullable<Goal["stopReason"]>, string> = {
  goal_met: "目标达成",
  rounds_exhausted: "轮次用尽",
  wall_time_exhausted: "超时停止",
  tokens_exhausted: "token 用尽",
  no_progress: "连续无进展",
  consecutive_failures: "连续失败",
  aborted_by_human: "人工停止",
};

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function totalTokens(goal: Goal): number {
  return goal.rounds.reduce((sum, round) => sum + round.tokensUsed, 0);
}

/**
 * 群聊顶部的 Goal 条：展示当前目标、轮次与预算，并提供启动/暂停/批准/停止。
 * 真正的循环驱动器后续接入；这里只操作系统状态。
 */
export function GoalStrip({ group }: { group: { id: string } }) {
  const [goal, setGoal] = useState<Goal | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      const goals = await api.listGoals(group.id);
      const running = goals.find(
        (item) =>
          item.status === "active" ||
          item.status === "paused" ||
          item.status === "awaiting_approval",
      );
      setGoal(running ?? goals[0] ?? null);
      setError(null);
    } catch (cause) {
      setError(errorText(cause));
    }
  }, [group.id]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await action();
      await reload();
    } catch (cause) {
      setError(errorText(cause));
    } finally {
      setBusy(false);
    }
  };

  if (!goal) {
    return (
      <div className="goal-strip is-empty">
        <span className="gs-title">GOAL</span>
        <input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="给这个群一个目标，Agent 会自己推进"
          aria-label="新目标"
        />
        <button
          type="button"
          disabled={busy || !draft.trim()}
          onClick={() =>
            void run(async () => {
              await api.createGoal({ groupId: group.id, objective: draft.trim() });
              setDraft("");
            })
          }
        >
          新建
        </button>
        {error && <span className="gs-error">{error}</span>}
      </div>
    );
  }

  const budgetRounds = goal.budget.maxRounds > 0 ? goal.budget.maxRounds : "∞";
  const terminal = goal.status === "completed" || goal.status === "aborted";

  return (
    <div className="goal-strip" data-status={goal.status}>
      <span className="gs-title">GOAL</span>
      <span className="gs-objective" title={goal.objective}>
        {goal.objective}
      </span>
      <span className="gs-status">{STATUS_LABEL[goal.status]}</span>
      <span className="gs-meta">
        {goal.rounds.length}/{budgetRounds} 轮 · {totalTokens(goal)} tok
      </span>
      {goal.rounds.length > 0 && goal.rounds[goal.rounds.length - 1].summary && (
        <span className="gs-last" title={goal.rounds[goal.rounds.length - 1].summary}>
          {goal.rounds[goal.rounds.length - 1].summary}
        </span>
      )}
      {!terminal && goal.status === "draft" && (
        <button type="button" disabled={busy} onClick={() => void run(() => api.startGoal(goal.id))}>
          启动
        </button>
      )}
      {!terminal && goal.status === "active" && (
        <button type="button" disabled={busy} onClick={() => void run(() => api.pauseGoal(goal.id))}>
          暂停
        </button>
      )}
      {!terminal && goal.status === "paused" && (
        <button type="button" disabled={busy} onClick={() => void run(() => api.startGoal(goal.id))}>
          继续
        </button>
      )}
      {!terminal && goal.status === "awaiting_approval" && (
        <button type="button" disabled={busy} onClick={() => void run(() => api.approveGoal(goal.id))}>
          批准
        </button>
      )}
      {!terminal && (
        <button type="button" disabled={busy} onClick={() => void run(() => api.abortGoal(goal.id))}>
          停止
        </button>
      )}
      {goal.stopReason && <span className="gs-stop">{STOP_LABEL[goal.stopReason]}</span>}
      {error && <span className="gs-error">{error}</span>}
    </div>
  );
}
