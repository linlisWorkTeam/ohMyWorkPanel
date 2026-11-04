//! 目标页（P07）：左列表 + 右详情，含新建、推进、审批、账本与预算。
//!
//! 设计要点：
//! - 事实以服务端为准：每个写操作后都重新拉取列表，不乐观伪造状态。
//! - advance 可能阻塞数分钟：超时提示「后台结果待确认」，不显示「已停止」。
//! - 终态不提供「继续」；自动推进开关只在非终态出现。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { api } from "../api";
import type { Goal, GoalBudget, GoalRound } from "../types";
import {
  formatDuration,
  formatTime,
  GOAL_STATUS_LABEL,
  GOAL_STOP_LABEL,
  VERDICT_LABEL,
} from "./labels";
import { summarizeRounds } from "./ledger";

const DEFAULT_BUDGET: GoalBudget = {
  maxRounds: 10,
  maxWallTimeMs: 60 * 60 * 1000,
  maxTokens: 200_000,
  maxNoProgressRounds: 3,
  maxConsecutiveFailures: 2,
};

interface GoalsPageProps {
  groupId: string;
  canManage: boolean;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function selectDefault(goals: Goal[]): string | null {
  const running = goals.find(
    (goal) => goal.status === "active" || goal.status === "awaiting_approval",
  );
  return (running ?? goals[0])?.id ?? null;
}

export function GoalsPage({ groupId, canManage }: GoalsPageProps) {
  const [goals, setGoals] = useState<Goal[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const generation = useRef(0);

  const load = useCallback(
    async (preferredId?: string | null) => {
      const current = ++generation.current;
      setLoading(true);
      try {
        const list = await api.listGoals(groupId);
        if (current !== generation.current) return;
        setGoals(list);
        setSelectedId((previous) => {
          const wanted = preferredId ?? previous;
          if (wanted && list.some((goal) => goal.id === wanted)) return wanted;
          return selectDefault(list);
        });
        setError(null);
      } catch (cause) {
        if (current === generation.current) setError(errorText(cause));
      } finally {
        if (current === generation.current) setLoading(false);
      }
    },
    [groupId],
  );

  useEffect(() => {
    setGoals([]);
    setSelectedId(null);
    void load(null);
    return () => {
      generation.current += 1;
    };
  }, [load]);

  const selected = useMemo(
    () => goals.find((goal) => goal.id === selectedId) ?? null,
    [goals, selectedId],
  );

  const act = async (action: () => Promise<unknown>, preferredId?: string | null) => {
    setBusy("action");
    try {
      await action();
      await load(preferredId ?? selectedId);
    } catch (cause) {
      setError(errorText(cause));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="goals-page">
      <aside className="goals-list" aria-label="目标列表">
        <div className="goals-list-head">
          <h2>目标</h2>
          {canManage && (
            <button type="button" className="pm-btn primary sm" onClick={() => setShowCreate(true)}>
              新建
            </button>
          )}
        </div>
        {loading && goals.length === 0 ? (
          <p className="goals-hint">加载中…</p>
        ) : goals.length === 0 ? (
          <div className="goals-empty">
            <p>这个群还没有目标。</p>
            {canManage ? (
              <button type="button" className="pm-btn sm" onClick={() => setShowCreate(true)}>
                创建第一个目标
              </button>
            ) : (
              <p className="goals-hint">请联系群主或管理员创建。</p>
            )}
          </div>
        ) : (
          <ul className="goals-items">
            {goals.map((goal) => {
              const stats = summarizeRounds(goal.rounds);
              return (
                <li key={goal.id}>
                  <button
                    type="button"
                    className={`goal-row${goal.id === selectedId ? " is-active" : ""}`}
                    aria-current={goal.id === selectedId}
                    onClick={() => setSelectedId(goal.id)}
                  >
                    <span className="goal-row-title" title={goal.objective}>
                      {goal.objective}
                    </span>
                    <span className="goal-row-meta">
                      <span className="goal-status" data-status={goal.status}>
                        {GOAL_STATUS_LABEL[goal.status]}
                      </span>
                      <span>
                        {goal.rounds.length}/{goal.budget.maxRounds > 0 ? goal.budget.maxRounds : "∞"} 轮
                      </span>
                      <span>{stats.tokens} tok</span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </aside>

      <section className="goals-detail" aria-label="目标详情">
        {error && (
          <div className="goals-error" role="alert">
            {error}
          </div>
        )}
        {showCreate ? (
          <CreateGoalPanel
            groupId={groupId}
            onCancel={() => setShowCreate(false)}
            onCreated={async (goal) => {
              setShowCreate(false);
              await load(goal.id);
            }}
            onError={setError}
          />
        ) : selected ? (
          <GoalDetail
            goal={selected}
            canManage={canManage}
            busy={busy !== null}
            onAction={act}
            onError={setError}
          />
        ) : (
          <div className="goals-empty">
            <p>选择左侧目标查看详情，或新建一个目标。</p>
          </div>
        )}
      </section>
    </div>
  );
}

function GoalDetail({
  goal,
  canManage,
  busy,
  onAction,
  onError,
}: {
  goal: Goal;
  canManage: boolean;
  busy: boolean;
  onAction: (action: () => Promise<unknown>, preferredId?: string | null) => Promise<void>;
  onError: (message: string) => void;
}) {
  const [expanded, setExpanded] = useState<number | null>(null);
  const terminal = goal.status === "completed" || goal.status === "aborted";
  const stats = summarizeRounds(goal.rounds);
  const elapsed = goal.startedAt ? Date.now() - goal.startedAt : 0;

  const advance = async () => {
    try {
      await onAction(() => api.advanceGoal(goal.id), goal.id);
    } catch {
      // onAction 已展示错误；这里补一条更准确的超时语义。
    }
  };

  return (
    <div className="goal-detail">
      <header className="goal-detail-head">
        <div>
          <h2>{goal.objective}</h2>
          <div className="goal-detail-meta">
            <span className="goal-status" data-status={goal.status}>
              {GOAL_STATUS_LABEL[goal.status]}
            </span>
            {goal.stopReason && (
              <span className="goal-stop">{GOAL_STOP_LABEL[goal.stopReason]}</span>
            )}
            <span>{goal.rounds.length} 轮</span>
            <span>{stats.tokens} tok</span>
            {goal.budget.maxTokens > 0 && <span>预算 {goal.budget.maxTokens} tok</span>}
            {goal.startedAt ? <span>已运行 {formatDuration(elapsed)}</span> : null}
          </div>
        </div>
        {canManage && !terminal && (
          <div className="goal-actions">
            {goal.status === "draft" && (
              <button type="button" className="pm-btn primary sm" disabled={busy}
                onClick={() => void onAction(() => api.startGoal(goal.id), goal.id)}>
                启动
              </button>
            )}
            {goal.status === "active" && (
              <>
                <button type="button" className="pm-btn primary sm" disabled={busy} onClick={() => void advance()}>
                  {busy ? "本轮推进中…" : "推进一步"}
                </button>
                <button type="button" className="pm-btn sm" disabled={busy}
                  onClick={() => void onAction(() => api.pauseGoal(goal.id), goal.id)}>
                  暂停
                </button>
              </>
            )}
            {goal.status === "paused" && (
              <button type="button" className="pm-btn primary sm" disabled={busy}
                onClick={() => void onAction(() => api.startGoal(goal.id), goal.id)}>
                继续
              </button>
            )}
            {goal.status === "awaiting_approval" && (
              <button type="button" className="pm-btn primary sm" disabled={busy}
                onClick={() => void onAction(() => api.approveGoal(goal.id), goal.id)}>
                批准本轮
              </button>
            )}
            <button type="button" className="pm-btn sm danger" disabled={busy}
              onClick={() => void onAction(() => api.abortGoal(goal.id), goal.id)}>
              停止
            </button>
            <label className="goal-auto" title="允许后台每 10 秒自动推进一轮">
              <input
                type="checkbox"
                checked={goal.autoAdvance}
                disabled={busy}
                onChange={(event) => {
                  void onAction(
                    () => api.setGoalAutoAdvance(goal.id, event.target.checked),
                    goal.id,
                  ).catch((cause) => onError(errorText(cause)));
                }}
              />
              自动
            </label>
          </div>
        )}
      </header>

      {goal.status === "active" && (
        <p className="goals-hint">
          推进可能等待数分钟；离开此页不会停止目标。等待超时只代表结果待确认，请返回后查看最新轮次。
        </p>
      )}

      <section className="goal-budget" aria-label="预算">
        <span>轮次 {goal.rounds.length}/{goal.budget.maxRounds > 0 ? goal.budget.maxRounds : "∞"}</span>
        <span>无进展上限 {goal.budget.maxNoProgressRounds}</span>
        <span>连续失败上限 {goal.budget.maxConsecutiveFailures}</span>
        <span>每轮审批 {goal.requireApprovalEachRound ? "开" : "关"}</span>
      </section>

      <section className="goal-ledger" aria-label="证据账本">
        <h3>证据账本</h3>
        {goal.rounds.length === 0 ? (
          <p className="goals-hint">还没有轮次。</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th>#</th>
                <th>判定</th>
                <th>token</th>
                <th>完成时间</th>
                <th>摘要</th>
              </tr>
            </thead>
            <tbody>
              {[...goal.rounds].reverse().map((round) => (
                <LedgerRow
                  key={round.index}
                  round={round}
                  expanded={expanded === round.index}
                  onToggle={() => setExpanded(expanded === round.index ? null : round.index)}
                />
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}

function LedgerRow({
  round,
  expanded,
  onToggle,
}: {
  round: GoalRound;
  expanded: boolean;
  onToggle: () => void;
}) {
  const summary = round.summary.trim();
  const clipped = summary.length > 80 && !expanded ? `${summary.slice(0, 80)}…` : summary;
  return (
    <tr className="goal-ledger-row" data-verdict={round.verdict}>
      <td>#{round.index}</td>
      <td>{VERDICT_LABEL[round.verdict]}</td>
      <td>{round.tokensUsed}</td>
      <td>{formatTime(round.finishedAt)}</td>
      <td>
        {clipped ? (
          <button type="button" className="ledger-summary" onClick={onToggle} title={summary}>
            {clipped}
          </button>
        ) : (
          "—"
        )}
      </td>
    </tr>
  );
}

function CreateGoalPanel({
  groupId,
  onCancel,
  onCreated,
  onError,
}: {
  groupId: string;
  onCancel: () => void;
  onCreated: (goal: Goal) => Promise<void> | void;
  onError: (message: string) => void;
}) {
  const [objective, setObjective] = useState("");
  const [budget, setBudget] = useState<GoalBudget>(DEFAULT_BUDGET);
  const [requireApproval, setRequireApproval] = useState(false);
  const [autoAdvance, setAutoAdvance] = useState(false);
  const [saving, setSaving] = useState(false);

  const submit = async () => {
    if (!objective.trim() || saving) return;
    setSaving(true);
    try {
      const goal = await api.createGoal({
        groupId,
        objective: objective.trim(),
        budget,
        requireApprovalEachRound: requireApproval,
        autoAdvance,
      });
      await onCreated(goal);
    } catch (cause) {
      onError(errorText(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="goal-create">
      <h2>新建目标</h2>
      <label className="goal-field">
        <span>目标</span>
        <textarea
          rows={3}
          value={objective}
          placeholder="例如：完成登录页并接入真实接口"
          onChange={(event) => setObjective(event.target.value)}
        />
      </label>
      <div className="goal-budget-form">
        <label className="goal-field">
          <span>最大轮次（0=不限）</span>
          <input type="number" min={0} value={budget.maxRounds}
            onChange={(e) => setBudget({ ...budget, maxRounds: Number(e.target.value) })} />
        </label>
        <label className="goal-field">
          <span>最大时长（分钟）</span>
          <input type="number" min={1} value={Math.round(budget.maxWallTimeMs / 60000)}
            onChange={(e) => setBudget({ ...budget, maxWallTimeMs: Number(e.target.value) * 60000 })} />
        </label>
        <label className="goal-field">
          <span>token 上限（0=不限）</span>
          <input type="number" min={0} value={budget.maxTokens}
            onChange={(e) => setBudget({ ...budget, maxTokens: Number(e.target.value) })} />
        </label>
        <label className="goal-field">
          <span>连续无进展上限</span>
          <input type="number" min={1} value={budget.maxNoProgressRounds}
            onChange={(e) => setBudget({ ...budget, maxNoProgressRounds: Number(e.target.value) })} />
        </label>
        <label className="goal-field">
          <span>连续失败上限</span>
          <input type="number" min={1} value={budget.maxConsecutiveFailures}
            onChange={(e) => setBudget({ ...budget, maxConsecutiveFailures: Number(e.target.value) })} />
        </label>
      </div>
      <label className="goal-check">
        <input type="checkbox" checked={requireApproval} onChange={(e) => setRequireApproval(e.target.checked)} />
        每轮需要人工批准
      </label>
      <label className="goal-check">
        <input type="checkbox" checked={autoAdvance} onChange={(e) => setAutoAdvance(e.target.checked)} />
        创建后自动推进
      </label>
      <div className="goal-create-actions">
        <button type="button" className="pm-btn primary sm" disabled={saving || !objective.trim()} onClick={() => void submit()}>
          {saving ? "创建中…" : "创建"}
        </button>
        <button type="button" className="pm-btn sm" disabled={saving} onClick={onCancel}>
          取消
        </button>
      </div>
    </div>
  );
}
