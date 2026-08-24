import { useEffect, useState, type JSX } from "react";
import { modelsForAdapter } from "../agentModels";
import { ContextActionMenu, useLongPress, type ActionItem } from "../components/ContextActionMenu";
import { memberRosterAction } from "../memberForm";
import { agentBusyLabel, queueCounts, runsForAgentActive } from "../queueCounts";
import type { Group, Member, TaskRun } from "../types";

export type RosterProps = {
  members: Member[];
  group: Group;
  runs: TaskRun[];
  detectingId?: string | null;
  onlineUserIds?: ReadonlySet<string>;
  askMode?: boolean;
  onAdmin: (id: string | null) => void;
  onRemove: (member: Member) => void;
  onDetect: (member: Member) => void;
  onModel: (member: Member, model: string) => void;
  onCancelRun: (run: TaskRun) => void;
  onOpenDsh?: (member: Member) => void;
  onEditApiKey?: (member: Member, apiKey: string) => void;
  onEditApiUrl?: (member: Member, apiUrl: string) => void;
};

export function Roster(props: RosterProps): JSX.Element {
  const { members } = props;
  return (
    <div className="wp-roster">
      <p className="wp-roster-hint">右键或长按成员：检测 / 设管理 / 移除（行上不摊按钮）</p>
      {members.map((member) => (
        <RosterRow key={member.id} {...props} member={member} />
      ))}
    </div>
  );
}

function RosterRow({
  member,
  group,
  runs,
  detectingId,
  onlineUserIds,
  askMode,
  onAdmin,
  onRemove,
  onDetect,
  onModel,
  onCancelRun,
  onOpenDsh,
  onEditApiKey,
  onEditApiUrl,
}: RosterProps & { member: Member }): JSX.Element {
  const detecting = detectingId === member.id;
  const online =
    member.kind === "user" && !!member.authUserId && !!onlineUserIds?.has(member.authUserId);
  const rowAskMode = Boolean(askMode) && member.id === group.adminMemberId;
  const isAdmin = group.adminMemberId === member.id;
  const modelOptions = modelsForAdapter(member.adapter);
  const counts = queueCounts(runs, member.id);
  const busy = agentBusyLabel(counts);
  const responding = busy != null;
  const [queueOpen, setQueueOpen] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  useEffect(() => {
    if (!responding) setQueueOpen(false);
  }, [responding]);
  const activeRuns = queueOpen ? runsForAgentActive(runs, member.id) : [];
  const idleRuntime =
    member.runtimeStatus === "ready" ? "已就绪" : member.runtimeStatus === "unavailable" ? "不可用" : "待检测";
  const statusText = member.kind === "agent"
    ? `${member.adapter}${member.model ? ` · ${member.model}` : ""}`
    : member.kind === "chatbot"
      ? `${member.adapter ?? "chatbot"} · ${member.model || "deepseek-v4-flash"}`
      : member.invitePending
        ? "邀请未接受"
        : member.roleDescription || "本地成员";
  const stateLabel = member.kind === "user"
    ? (online ? "在线" : member.invitePending ? "等待" : "离线")
    : detecting ? "检测中" : busy ? "执行中" : idleRuntime;
  const stateKind = detecting ? "" : busy ? "busy" : member.runtimeStatus === "unavailable" ? "bad" : online || member.runtimeStatus === "ready" ? "ok" : "";
  const rosterAction = memberRosterAction(member);
  const items: ActionItem[] = [];
  if (member.kind === "agent") {
    items.push({
      id: "detect",
      label: detecting ? "检测中" : "检测",
      disabled: Boolean(detecting),
      onSelect: () => onDetect(member),
    });
  }
  if ((member.kind === "agent" || member.kind === "chatbot") && !member.systemLocked) {
    items.push({
      id: "admin",
      label: isAdmin
        ? (group.groupKind === "chat" ? "撤销默认响应" : "撤销管理")
        : (group.groupKind === "chat" ? "设为默认响应" : "设管理"),
      onSelect: () => onAdmin(isAdmin ? null : member.id),
    });
  }
  if ((member.kind === "agent" || member.kind === "chatbot") && !member.systemLocked) {
    items.push({
      id: "edit-api-key",
      label: "编辑 API Key…",
      onSelect: () => {
        const value = window.prompt(
          member.kind === "chatbot" ? "粘贴新的 API Key（留空则清除）：" : "粘贴新的 API Key（留空则清除）：",
          "",
        );
        if (value !== null) onEditApiKey?.(member, value.trim());
      },
    });
  }
  if (member.kind === "chatbot" && !member.systemLocked) {
    items.push({
      id: "edit-api-url",
      label: "编辑 API 地址…",
      onSelect: () => {
        const value = window.prompt(
          "OpenAI 兼容端点，如 https://api.xxx.com/v1（留空则清除）：",
          member.apiUrl ?? "",
        );
        if (value !== null) onEditApiUrl?.(member, value.trim());
      },
    });
  }
  if (modelOptions.length > 0 && (member.kind === "agent" || member.kind === "chatbot") && !member.systemLocked) {
    const children: ActionItem[] = modelOptions.map((model) => ({
      id: `model:${model}`,
      label: member.model === model ? `${model} ✓` : model,
      onSelect: () => onModel(member, model),
    }));
    children.push({
      id: "model:custom",
      label: "自定义模型…",
      onSelect: () => {
        const custom = window.prompt("输入模型 ID（例如 my-model-7b）：", member.model ?? "");
        const value = custom?.trim();
        if (value) onModel(member, value);
      },
    });
    items.push({
      id: "models",
      label: member.model ? `切换模型 · ${member.model}` : "切换模型",
      children,
    });
  }
  if (member.kind === "agent" && member.adapter === "dsh") {
    items.push({ id: "dsh", label: "跳转 DSH Web", onSelect: () => onOpenDsh?.(member) });
  }
  if (member.invitePending) {
    items.push({
      id: "revoke-invite",
      label: "撤销邀请",
      danger: true,
      onSelect: () => onRemove(member),
    });
  } else if (!member.systemLocked && member.id !== group.ownerMemberId) {
    items.push({
      id: "remove",
      label: rosterAction === "delete" ? "删除" : "移除",
      danger: true,
      onSelect: () => onRemove(member),
    });
  }
  if (items.length === 0) {
    items.push({
      id: "copy-name",
      label: "复制名称",
      onSelect: () => void navigator.clipboard.writeText(member.displayName),
    });
  }
  const openMenu = (x: number, y: number) => setMenu({ x, y });
  const hold = useLongPress(openMenu);
  const dotClass = detecting || responding
    ? "busy"
    : (member.kind === "user" ? (online ? "" : "off") : member.runtimeStatus === "ready" ? "" : "off");
  const rowClass = [
    "wp-m-row",
    member.isActive ? "" : "inactive",
    member.invitePending ? "invite-pending" : "",
    responding ? "on" : "",
  ].filter(Boolean).join(" ");

  return (
    <div
      className={rowClass}
      onContextMenu={(event) => {
        event.preventDefault();
        openMenu(event.clientX, event.clientY);
      }}
      {...hold}
    >
      <div
        className="wp-m-av"
        style={{ background: member.avatarColor ?? "var(--lp-text-tertiary)" }}
      >
        {member.displayName.slice(0, 1)}
        <i className={`dot ${dotClass}`.trim()} />
      </div>
      <div className="wp-m-meta">
        <div className="wp-m-name">
          {member.displayName}
          {member.id === group.ownerMemberId && <em className="lead">群主</em>}
          {isAdmin && (
            <em className="lead">{group.groupKind === "chat" ? "默认响应" : "管理员"}</em>
          )}
          {rowAskMode && <em>Ask</em>}
          {member.kind === "chatbot" && <em>机器人</em>}
          {member.systemLocked && <em title="平台锁定的自举 Agent，不可修改/移除">系统</em>}
          {member.invitePending && <em>链接中</em>}
        </div>
        <div className="wp-m-sub">
          {member.kind === "agent" && busy ? (
            <button
              type="button"
              onClick={() => setQueueOpen((open) => !open)}
              aria-expanded={queueOpen}
              title={queueOpen ? "收起排队任务" : "展开排队任务"}
            >
              {statusText}
            </button>
          ) : (
            statusText
          )}
          {member.tags ? ` · ${member.tags}` : ""}
        </div>
        {queueOpen && activeRuns.length > 0 && (
          <ul>
            {activeRuns.map((run) => (
              <li key={run.id}>
                <span>{run.status === "running" ? "执行中" : "排队中"} · {run.id.slice(0, 8)}</span>
                <button type="button" onClick={() => onCancelRun(run)}>取消</button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className={`wp-m-state ${stateKind}`.trim()}>{stateLabel}</div>
      {menu && <ContextActionMenu items={items} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />}
    </div>
  );
}
