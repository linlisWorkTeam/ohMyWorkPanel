//! 成员详情（设计 P04）：右栏详情分区，按成员类型展示事实与允许的操作。
//!
//! 移除与永久删除分开：本面板只做「移出群」，永久删除仍在成员菜单且需二次确认。

import { useState } from "react";

import { modelsForAdapter } from "../agents/agentModels";
import { queueCounts, runsForAgentActive } from "./queueCounts";
import type { Group, Member, TaskRun } from "../types";

const KIND_LABEL: Record<string, string> = {
  user: "用户",
  agent: "Agent",
  chatbot: "聊天机器人",
};

const RUNTIME_LABEL: Record<string, string> = {
  ready: "就绪",
  unavailable: "不可用",
  unknown: "待检测",
};

function formatTime(ts: number): string {
  return new Date(ts).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

interface MemberDetailProps {
  member: Member;
  group: Group;
  runs: TaskRun[];
  detecting: boolean;
  onClose: () => void;
  onDetect: (member: Member) => void;
  onAdmin: (memberId: string | null) => void;
  onRemove: (member: Member) => void;
  onModel: (member: Member, model: string) => void;
  onApiKey: (member: Member, apiKey: string) => void;
  onError: (message: string) => void;
}

export function MemberDetail({
  member,
  group,
  runs,
  detecting,
  onClose,
  onDetect,
  onAdmin,
  onRemove,
  onModel,
  onApiKey,
  onError,
}: MemberDetailProps) {
  const [keyDraft, setKeyDraft] = useState("");
  const [keyBusy, setKeyBusy] = useState(false);
  const activeRuns = runsForAgentActive(runs, member.id);
  const queued = queueCounts(runs, member.id);
  const isOwner = group.ownerMemberId === member.id;
  const isAdminAgent = group.adminMemberId === member.id;
  const models = modelsForAdapter(member.adapter ?? "") ?? [];
  const locked = Boolean(member.systemLocked);
  const rawWorkspace = member.workspacePath ?? "";
  const workspaceLabel = rawWorkspace.startsWith("\\\\?\\") ? rawWorkspace.slice(4) : rawWorkspace;

  const saveKey = async () => {
    if (!keyDraft.trim() || keyBusy) return;
    setKeyBusy(true);
    try {
      await onApiKey(member, keyDraft.trim());
      setKeyDraft("");
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    } finally {
      setKeyBusy(false);
    }
  };

  return (
    <div className="member-detail">
      <header className="member-detail-head">
        <div className="member-detail-id">
          <span className="member-detail-avatar" style={{ background: member.avatarColor ?? "var(--lp-text-dim)" }}>
            {member.displayName.slice(0, 1)}
          </span>
          <div>
            <h3>{member.displayName}</h3>
            <p>
              {KIND_LABEL[member.kind] ?? member.kind}
              {member.adapter ? ` · ${member.adapter}` : ""}
              {member.runtimeStatus ? ` · ${RUNTIME_LABEL[member.runtimeStatus] ?? member.runtimeStatus}` : ""}
            </p>
          </div>
        </div>
        <button type="button" className="pm-btn sm quiet" onClick={onClose}>关闭</button>
      </header>

      <div className="member-detail-badges">
        {isOwner && <span className="member-badge">群主</span>}
        {isAdminAgent && <span className="member-badge">管理员</span>}
        {locked && <span className="member-badge warn">系统锁定</span>}
        {member.invitePending && <span className="member-badge">邀请未接受</span>}
        {(activeRuns.length > 0 || queued.queued > 0) && (
          <span className="member-badge active">
            运行中 {activeRuns.length} · 排队 {queued.queued}
          </span>
        )}
      </div>

      <dl className="member-detail-grid">
        <div><dt>角色</dt><dd>{member.roleDescription || "—"}</dd></div>
        <div><dt>模型</dt><dd>{member.model || "—"}</dd></div>
        <div><dt>可执行文件</dt><dd className="mono">{member.executablePath || "—"}</dd></div>
        <div><dt>工作区</dt><dd className="mono">{workspaceLabel || "（默认沙箱）"}</dd></div>
        <div><dt>API Key</dt><dd>{member.apiKeySet ? "已设置" : "未设置"}</dd></div>
        <div><dt>API URL</dt><dd className="mono">{member.apiUrl || "—"}</dd></div>
        <div><dt>保活</dt><dd>{member.keepAlive ? "开" : "关"}{member.warmStatus ? ` · ${member.warmStatus}` : ""}</dd></div>
        <div><dt>加入时间</dt><dd>{formatTime(member.createdAt)}</dd></div>
      </dl>

      {member.kind === "agent" && (
        <div className="member-detail-actions">
          <button type="button" className="pm-btn sm" disabled={detecting} onClick={() => onDetect(member)}>
            {detecting ? "检测中…" : "检测"}
          </button>
          {models.length > 0 && (
            <label className="member-detail-model">
              模型
              <select
                value={member.model ?? ""}
                onChange={(event) => onModel(member, event.target.value)}
              >
                <option value="">默认</option>
                {models.map((model) => (
                  <option key={model} value={model}>{model}</option>
                ))}
              </select>
            </label>
          )}
        </div>
      )}

      {(member.kind === "agent" || member.kind === "chatbot") && (
        <div className="member-detail-key">
          <label>
            更新 API Key
            <input
              type="password"
              value={keyDraft}
              placeholder={member.apiKeySet ? "已设置，输入新值可覆盖" : "粘贴 Key"}
              onChange={(event) => setKeyDraft(event.target.value)}
            />
          </label>
          <button type="button" className="pm-btn sm" disabled={keyBusy || !keyDraft.trim()} onClick={() => void saveKey()}>
            {keyBusy ? "保存中…" : "保存"}
          </button>
        </div>
      )}

      <div className="member-detail-actions">
        {isOwner && (
          <button
            type="button"
            className="pm-btn sm"
            disabled={locked}
            onClick={() => onAdmin(isAdminAgent ? null : member.id)}
          >
            {isAdminAgent ? "取消管理员" : "设为管理员"}
          </button>
        )}
        <button
          type="button"
          className="pm-btn sm danger"
          disabled={locked || isOwner}
          title={isOwner ? "群主不能被移除" : locked ? "系统成员不可移除" : undefined}
          onClick={() => {
            if (window.confirm(`将 ${member.displayName} 移出群？历史消息会保留。`)) onRemove(member);
          }}
        >
          移出群
        </button>
      </div>

      <p className="member-detail-hint">
        永久删除仍在成员菜单中执行（需二次确认）；此处只移出当前群。
      </p>
    </div>
  );
}
