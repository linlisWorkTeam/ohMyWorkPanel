//! Run 详情（设计 P04）：右栏详情分区展示一次执行的完整事实。
//!
//! 只读展示 + 允许的操作（取消 / 重试 / 审阅）；不伪造调度图。

import { PHASE_LABEL } from "../components/uiShared";
import type { Member, TaskRun } from "../types";

const STATUS_LABEL: Record<string, string> = {
  queued: "排队中",
  running: "运行中",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
  interrupted: "已中断",
  changes_requested: "需修改",
};



const REVIEW_LABEL: Record<string, string> = {
  pending: "待审阅",
  approved: "已通过",
  rejected: "已驳回",
};

function time(ts: number | null): string {
  if (!ts) return "—";
  return new Date(ts).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

interface RunDetailsProps {
  run: TaskRun;
  member: Member | undefined;
  parent: TaskRun | null;
  onClose: () => void;
  onCancel: (run: TaskRun) => void;
  onRetry: (run: TaskRun) => void;
  onReview: (run: TaskRun, decision: "approved" | "rejected") => void;
}

export function RunDetails({ run, member, parent, onClose, onCancel, onRetry, onReview }: RunDetailsProps) {
  const canCancel = run.status === "running" || run.status === "queued";
  const canRetry = ["failed", "cancelled", "interrupted", "changes_requested"].includes(run.status);
  const elapsed = run.startedAt
    ? (run.completedAt ?? Date.now()) - run.startedAt
    : 0;

  return (
    <div className="run-details">
      <header className="run-details-head">
        <div>
          <h3>运行详情</h3>
          <p className="run-details-sub">{member?.displayName ?? run.agentMemberId}</p>
        </div>
        <button type="button" className="pm-btn sm quiet" onClick={onClose}>关闭</button>
      </header>

      <dl className="run-details-grid">
        <div><dt>状态</dt><dd data-status={run.status}>{STATUS_LABEL[run.status] ?? run.status}</dd></div>
        <div><dt>阶段</dt><dd>{run.phase ? (PHASE_LABEL[run.phase] ?? run.phase) : "—"}</dd></div>
        <div><dt>审阅</dt><dd>{run.reviewStatus ? (REVIEW_LABEL[run.reviewStatus] ?? run.reviewStatus) : "—"}</dd></div>
        <div><dt>耗时</dt><dd>{elapsed > 0 ? `${Math.round(elapsed / 1000)}s` : "—"}</dd></div>
        <div><dt>深度</dt><dd>{run.depth}</dd></div>
        <div><dt>run id</dt><dd className="run-details-id">{run.id}</dd></div>
        <div><dt>开始</dt><dd>{time(run.startedAt)}</dd></div>
        <div><dt>结束</dt><dd>{time(run.completedAt)}</dd></div>
      </dl>

      {parent && (
        <p className="run-details-hint">
          父运行：{parent.id.slice(0, 12)}…（{STATUS_LABEL[parent.status] ?? parent.status}）
        </p>
      )}
      {run.errorMessage && (
        <p className="run-details-error" role="alert">{run.errorMessage}</p>
      )}

      <div className="run-details-actions">
        {canCancel && (
          <button type="button" className="pm-btn sm" onClick={() => onCancel(run)}>请求停止</button>
        )}
        {canRetry && (
          <button type="button" className="pm-btn sm" onClick={() => onRetry(run)}>重试（新建执行）</button>
        )}
        {run.reviewStatus === "pending" && (
          <>
            <button type="button" className="pm-btn primary sm" onClick={() => onReview(run, "approved")}>通过</button>
            <button type="button" className="pm-btn sm" onClick={() => onReview(run, "rejected")}>驳回</button>
          </>
        )}
      </div>

      {canRetry && (
        <p className="run-details-hint">重试会创建新的执行，可能再次写入工作区。</p>
      )}
    </div>
  );
}
