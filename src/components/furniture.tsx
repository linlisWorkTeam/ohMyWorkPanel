import { memo, useEffect, useState } from "react";
import { api } from "../api";
import { agentReplyDefaultOpen, extractReplyPreview } from "../chatUi";
import { hasRenderableContent, parseMessageContent } from "../messageContent";
import { markdownToHtml } from "../markdownLite";
import { modelsForAdapter } from "../agentModels";
import { agentBusyLabel, queueCounts, runsForAgentActive } from "../queueCounts";
import { memberRosterAction } from "../memberForm";
import { PHASE_LABEL, time, dayLabel, readError } from "./uiShared";
import { applySelfDetailsToggle } from "../detailsToggle";
import type { Group, GroupState, Member, RunPhaseEntry, TaskRun } from "../types";

/* ============================================================
   WorkPanel UI furniture: 从 App.tsx 抽取的消息/成员/状态组件（P1 组件化）
   仅呈现/交互，不持有业务状态；所有颜色只走 --lp-* 语义 token。
   ============================================================ */

export const MessageBubble = memo(function MessageBubble({
  message,
  members,
  runs,
  viewerMemberId,
  onRun,
  voiceUxEnabled,
  playingMessageId,
  onPlayVoice,
}: {
  message: GroupState["messages"][number];
  members: Member[];
  runs: TaskRun[];
  viewerMemberId: string | null;
  onRun: (run: TaskRun, operation: "cancel" | "retry") => void;
  voiceUxEnabled?: boolean;
  playingMessageId?: string | null;
  onPlayVoice?: (messageId: string, content: string) => void;
}) {
  const sender = members.find((member) => member.id === message.senderMemberId);
  const run = runs.find((candidate) => candidate.outputMessageId === message.id);
  const own = Boolean(viewerMemberId) && message.senderMemberId === viewerMemberId;
  const responding = message.status === "streaming" || run?.status === "queued" || run?.status === "running";
  const hasContent = hasRenderableContent(message.content);
  const foldAgent = !own && hasContent;
  const [expanded, setExpanded] = useState(() => agentReplyDefaultOpen(responding));
  useEffect(() => {
    if (responding) setExpanded(true);
  }, [responding]);
  const showParts = hasContent || message.hasThinking || message.hasArtifact;
  const body = showParts ? (
    <>
      <MessagePartsBody
        groupId={message.groupId}
        messageId={message.id}
        content={message.content}
        hasThinking={Boolean(message.hasThinking)}
        hasArtifact={Boolean(message.hasArtifact)}
        streaming={responding}
      />
      {responding && <span className="stream-caret" aria-hidden />}
    </>
  ) : (
    <TypingIndicator label={
      run?.phase ? (PHASE_LABEL[run.phase] ?? run.phase)
        : run?.status === "queued" ? "排队中" : "…"
    } />
  );
  const showPlay = Boolean(voiceUxEnabled && hasContent && !responding && onPlayVoice);
  const playing = playingMessageId === message.id;
  const [copied, setCopied] = useState(false);
  const copyMessage = async () => {
    const text = message.content || "";
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      try {
        document.execCommand("copy");
      } catch {
        /* ignore */
      }
      document.body.removeChild(ta);
    }
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  };
  return (
    <article className={`message-row ${own ? "own" : ""} ${responding ? "is-responding" : ""}`}>
      <Avatar member={sender} responding={responding && !own} />
      <div className="message-content">
        <div className="message-meta">
          <strong>{sender?.displayName ?? "已移除成员"}</strong>
          <span>{time(message.createdAt)}</span>
          {run && <Status status={run.status} />}
          {run?.phase && (run.status === "queued" || run.status === "running") && (
            <em className="phase-badge">{PHASE_LABEL[run.phase] ?? run.phase}</em>
          )}
          {run?.reviewStatus && <ReviewBadge reviewStatus={run.reviewStatus} />}
        </div>
        <div className={`bubble ${message.status}${responding ? " streaming" : ""}${showPlay ? " has-play" : ""}`}>
          {foldAgent ? (
            <details
              className="agent-reply-fold"
              open={expanded}
              onToggle={(event) => applySelfDetailsToggle(event, setExpanded)}
            >
              <summary>{responding ? "流式输出中…" : extractReplyPreview(message.content)}</summary>
              {body}
            </details>
          ) : (
            body
          )}
          {showPlay && (
            <button
              type="button"
              className={`bubble-play-btn${playing ? " playing" : ""}`}
              title={playing ? "播放中…" : "朗读消息"}
              disabled={playing}
              onClick={() => onPlayVoice?.(message.id, message.content)}
            >
              {playing ? "…" : "▶"}
            </button>
          )}
        </div>
        <div className="m-actions">
          <button type="button" className="mini-btn" onClick={() => void copyMessage()}>{copied ? "已复制 ✓" : "复制"}</button>
          {run && (
            <span className="mini-sep" />
          )}
          {(run?.status === "running" || run?.status === "queued") && (
            <button type="button" className="mini-btn" onClick={() => onRun(run, "cancel")}>停止</button>
          )}
          {run && ["failed", "cancelled", "interrupted", "changes_requested"].includes(run.status) && (
            <button type="button" className="mini-btn" onClick={() => onRun(run, "retry")}>重试</button>
          )}
        </div>
        {run?.errorMessage && <p className="run-error">{run.errorMessage}</p>}
      </div>
    </article>
  );
});

export function MessagePartsBody({
  groupId,
  messageId,
  content,
  hasThinking,
  hasArtifact,
  streaming,
}: {
  groupId: string;
  messageId: string;
  content: string;
  hasThinking: boolean;
  hasArtifact: boolean;
  streaming: boolean;
}) {
  const doc = parseMessageContent(content);
  if (!doc) {
    return (
      <div className="message-parts">
        {(hasThinking || hasArtifact) && (
          <>
            {hasThinking && (
              <LazyChannelPart
                groupId={groupId}
                messageId={messageId}
                channel="thinking"
                label="思考过程"
                streaming={streaming}
              />
            )}
            {hasArtifact && (
              <LazyChannelPart
                groupId={groupId}
                messageId={messageId}
                channel="artifact"
                label="中间产物"
                streaming={streaming}
              />
            )}
          </>
        )}
        <MarkdownBlock text={content} />
      </div>
    );
  }
  const finals = doc.parts.filter((part) => part.channel === "final" && part.text.trim());
  return (
    <div className="message-parts">
      {hasThinking && (
        <LazyChannelPart
          groupId={groupId}
          messageId={messageId}
          channel="thinking"
          label="思考过程"
          streaming={streaming}
        />
      )}
      {hasArtifact && (
        <LazyChannelPart
          groupId={groupId}
          messageId={messageId}
          channel="artifact"
          label="中间产物"
          streaming={streaming}
        />
      )}
      {finals.map((part, index) => (
        <MarkdownBlock key={`final-${index}`} text={part.text} />
      ))}
    </div>
  );
}

export function LazyChannelPart({
  groupId,
  messageId,
  channel,
  label,
  streaming,
}: {
  groupId: string;
  messageId: string;
  channel: "thinking" | "artifact";
  label: string;
  streaming: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    const load = () => {
      setLoading(true);
      setError(null);
      void api
        .getMessageChannelPart(groupId, messageId, channel)
        .then((part) => {
          if (!cancelled) setText(part.text);
        })
        .catch((reason) => {
          if (!cancelled) setError(readError(reason));
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    };
    load();
    if (!streaming) return () => { cancelled = true; };
    const timer = window.setInterval(load, 1500);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [open, groupId, messageId, channel, streaming]);

  return (
    <details
      className={`part-${channel}`}
      onClick={(event) => event.stopPropagation()}
      onToggle={(event) => applySelfDetailsToggle(event, setOpen)}
    >
      <summary>
        {label}
        {streaming ? "（生成中）" : ""}
        {!open ? " · 点击加载" : ""}
      </summary>
      {loading && text == null && <pre className="part-loading">加载中…</pre>}
      {error && <pre className="part-error">{error}</pre>}
      {text != null && <pre>{text || "（空）"}</pre>}
    </details>
  );
}

export function MarkdownBlock({ text }: { text: string }) {
  return (
    <div
      className="part-final md-body"
      dangerouslySetInnerHTML={{ __html: markdownToHtml(text) }}
    />
  );
}

export function TypingIndicator({ label }: { label: string }) {
  return (
    <span className="typing" role="status" aria-live="polite">
      <span className="typing-label">{label}</span>
      <span className="typing-dots" aria-hidden><i /><i /><i /></span>
    </span>
  );
}

export /** run 阶段轨迹（P2）：懒加载展开阶段时间线。 */
function PhaseTrail({ runId }: { runId: string }) {
  const [open, setOpen] = useState(false);
  const [entries, setEntries] = useState<RunPhaseEntry[] | null>(null);
  const toggle = async () => {
    const next = !open;
    setOpen(next);
    if (next && entries === null) {
      try {
        setEntries(await api.getRunPhases(runId));
      } catch {
        setEntries([]);
      }
    }
  };
  return (
    <div className="qc-trail">
      <button type="button" className="mini-btn qc-trail-toggle" onClick={() => void toggle()}>
        {open ? "▲ 收起轨迹" : "▼ 展开轨迹"}
      </button>
      {open && (
        <ol className="phase-trail">
          {(entries ?? []).map((entry, i) => (
            <li key={i}>
              <code>{entry.phase}</code>
              {entry.note ? <span className="pt-note">{entry.note}</span> : null}
              <time>{time(entry.createdAt)}</time>
            </li>
          ))}
          {entries === null && <li className="phase-empty">加载中…</li>}
          {entries !== null && entries.length === 0 && <li className="phase-empty">暂无阶段记录</li>}
        </ol>
      )}
    </div>
  );
}

export function RunQueuePane({ runs, members, onCancel, onReview }: {
  runs: TaskRun[];
  members: Member[];
  onCancel: (run: TaskRun) => void;
  onReview: (run: TaskRun, decision: "approved" | "rejected") => void;
}) {
  const active = runs
    .filter((r) => r.status === "running" || r.status === "queued")
    .sort((a, b) => (a.startedAt ?? a.createdAt) - (b.startedAt ?? b.createdAt));
  const review = runs.filter((r) => r.status === "awaiting_review");
  const nameOf = (id: string) => members.find((m) => m.id === id)?.displayName ?? "已移除成员";
  if (active.length === 0 && review.length === 0) {
    return <div className="queue-empty">当前没有排队或待审批的任务</div>;
  }
  return (
    <>
      <div className="sec-title"><span>执行中 · 排队</span><span>{active.length}</span></div>
      {active.map((run) => (
        <div key={run.id} className="queue-card">
          <div className="qc-top">
            <span className="qc-name">{nameOf(run.agentMemberId)}</span>
            <span className="qc-id">{run.id.slice(0, 8)}</span>
            <span className={`st ${run.status}`}>{run.status === "running" ? "执行中" : "排队中"}</span>
          </div>
          <div className="qc-phase">{run.phase ? (PHASE_LABEL[run.phase] ?? run.phase) : run.status === "queued" ? "等待调度" : "执行中…"}</div>
          <div className="qc-bar"><i /></div>
          <div className="qc-sub">
            <span>{time(run.createdAt)}</span>
            <button type="button" className="mini-btn qc-cancel" onClick={() => onCancel(run)}>取消</button>
          </div>
          <PhaseTrail runId={run.id} />
        </div>
      ))}
      {review.length > 0 && (
        <>
          <div className="sec-title"><span>待审批</span><span>{review.length}</span></div>
          {review.map((run) => (
            <div key={run.id} className="queue-card review">
              <div className="qc-top">
                <span className="qc-name">{nameOf(run.agentMemberId)}</span>
                <span className="st review">待审批</span>
              </div>
              <div className="qc-sub">交由 {run.reviewerMemberId ? nameOf(run.reviewerMemberId) : "审批人"} · 批准后由调度自动继续</div>
              {run.reviewStatus === "pending" && (
                <div className="qc-actions">
                  <button type="button" className="mini-btn qc-approve" onClick={() => onReview(run, "approved")}>✓ 批准</button>
                  <button type="button" className="mini-btn qc-reject" onClick={() => onReview(run, "rejected")}>✕ 拒绝（返回待修改）</button>
                </div>
              )}
            </div>
          ))}
        </>
      )}
    </>
  );
}

export function MemberRow({ member, group, runs, detecting, online, askMode, onAdmin, onRemove, onDetect, onModel, onCancelRun, onOpenDsh }: {
  member: Member; group: Group; runs: TaskRun[]; detecting?: boolean; online?: boolean; askMode?: boolean;
  onAdmin: (id: string | null) => void; onRemove: (member: Member) => void; onDetect: (member: Member) => void;
  onModel: (member: Member, model: string) => void;
  onCancelRun: (run: TaskRun) => void;
    onOpenDsh?: (member: Member) => void;
}) {
    
  const isAdmin = group.adminMemberId === member.id;
  const modelOptions = modelsForAdapter(member.adapter);
  const counts = queueCounts(runs, member.id);
  const busy = agentBusyLabel(counts);
  const responding = busy != null;
  const [queueOpen, setQueueOpen] = useState(false);
  useEffect(() => {
    if (!responding) setQueueOpen(false);
  }, [responding]);
  const activeRuns = queueOpen ? runsForAgentActive(runs, member.id) : [];
  const idleRuntime =
    member.runtimeStatus === "ready" ? "已就绪" : member.runtimeStatus === "unavailable" ? "不可用" : "待检测";
  const busyOrIdle = detecting ? "检测中…" : busy ?? idleRuntime;
  const statusText = member.kind === "agent"
    ? `${member.adapter} · ${busyOrIdle}${member.keepAlive ? ` · 保活${member.warmStatus ? `(${member.warmStatus})` : ""}` : ""}`
    : member.kind === "chatbot"
      ? `${member.adapter ?? "chatbot"} · ${member.model || "deepseek-v4-flash"} · ${member.apiKeySet ? "已配置 Key" : "缺 Key"}`
      : member.invitePending
        ? "链接中 · 等待接受邀请"
        : member.roleDescription || "本地成员";
  const rosterAction = memberRosterAction(member);
  return (
    <div className={`member-row ${member.isActive ? "" : "inactive"} ${member.invitePending ? "invite-pending" : ""} ${responding ? "is-responding" : ""}`}>
      <Avatar member={member} responding={responding} online={online} />
      <div className="member-details">
        <strong>
          {member.displayName}
          {member.id === group.ownerMemberId && <em>群主</em>}
          {isAdmin && (
            <em className="admin-badge">{group.groupKind === "chat" ? "默认响应" : "管理员"}</em>
          )}
          {askMode && <em className="ask-badge">Ask</em>}
          {member.kind === "chatbot" && <em className="admin-badge">机器人</em>}
            {member.systemLocked && <em className="admin-badge" title="平台锁定的自举 Agent，不可修改/移除">系统</em>}
          {member.invitePending && <em className="invite-badge">链接中</em>}
          {online && <em className="online-badge">在线</em>}
          {busy && <em className="responding-badge">{busy}</em>}
        </strong>
        <span>
          {member.kind === "agent" && busy ? (
            <button
              type="button"
              className="member-queue-toggle"
              onClick={() => setQueueOpen((open) => !open)}
              aria-expanded={queueOpen}
              title={queueOpen ? "收起排队任务" : "展开排队任务"}
            >
              {statusText}
            </button>
          ) : (
            statusText
          )}
          {member.tags ? ` · 🏷 ${member.tags}` : ""}
        </span>
        {queueOpen && activeRuns.length > 0 && (
          <ul className="member-queue-list">
            {activeRuns.map((run) => (
              <li key={run.id}>
                <span>{run.status === "running" ? "执行中" : "排队中"} · {run.id.slice(0, 8)}</span>
                <button type="button" className="danger" onClick={() => onCancelRun(run)}>取消</button>
              </li>
            ))}
          </ul>
        )}
        {modelOptions.length > 0 && (member.kind === "agent" || member.kind === "chatbot") && (
          <select
            className="member-model-select"
            value={member.model || modelOptions[0]}
            onChange={(e) => onModel(member, e.target.value)}
            title="切换模型"
          >
            {modelOptions.map((m) => <option key={m} value={m}>{m}</option>)}
          </select>
        )}
      </div>
      <div className="member-actions">
          {!member.systemLocked && (<>
        {member.kind === "agent" && (
          <button disabled={!!detecting} onClick={() => onDetect(member)}>{detecting ? "检测中" : "检测"}</button>
        )}
          {member.kind === "agent" && member.adapter === "dsh" && (
            <button
              onClick={() => onOpenDsh?.(member)}
              title="在群聊内打开 DeepSeek Harness Web 界面（需在服务器启动 dsh web，默认 :3080）"
            >
              跳转 DSH Web
            </button>
          )}
        {(member.kind === "agent" || member.kind === "chatbot") && (
          <button onClick={() => onAdmin(isAdmin ? null : member.id)} title={group.groupKind === "chat" ? "未设置时无人默认回复；设置后无 @ 时由该成员兜底" : "群管理员（Agent 可保活）"}>
            {isAdmin
              ? (group.groupKind === "chat" ? "撤销默认响应" : "撤销")
              : (group.groupKind === "chat" ? "设为默认响应" : "设管理")}
          </button>
        )}
        {member.id !== group.ownerMemberId && (
          <button className="danger" onClick={() => onRemove(member)}>
            {rosterAction === "delete"
              ? (member.invitePending ? "撤销邀请" : "删除")
              : "移除"}
          </button>
        )}
          </>)}
      </div>
    </div>
  );
}
export function Avatar({ member, responding, online }: { member?: Member; responding?: boolean; online?: boolean }) {
  return (
    <span className={`avatar ${responding ? "responding" : ""} ${online ? "is-online" : ""}`} style={{ background: member?.avatarColor ?? "#8792a5" }}>
      {member?.displayName.slice(0, 1) ?? "?"}
    </span>
  );
}
export function Status({ status }: { status: string }) { return <span className={`status ${status}`}>{({ queued: "排队中", running: "运行中", awaiting_review: "待审阅", changes_requested: "待修改", completed: "完成", failed: "失败", cancelled: "已停止", interrupted: "已中断" } as Record<string, string>)[status] ?? status}</span>; }
export function ReviewBadge({ reviewStatus }: { reviewStatus: string }) { return <span className={`review-badge ${reviewStatus}`}>{({ pending: "审阅中", approved: "已通过", rejected: "被退回" } as Record<string, string>)[reviewStatus] ?? reviewStatus}</span>; }

/** 无群欢迎页（替代永远"正在打开本地群聊…"的空态）。 */
export function EmptyHome({ canCreate, onCreate }: { canCreate?: boolean; onCreate?: () => void }) {
  return (
    <div className="empty-home">
      <div className="eh-mark">L</div>
      <strong className="eh-title">欢迎来到 WorkPanel</strong>
      <p className="eh-desc">这里还没有群聊。项目群绑定服务器工作区、用 @Agent 协作；聊天群则是一组机器人的轻对话。</p>
      <div className="eh-actions">
        {canCreate && (
          <button type="button" className="eh-primary" onClick={onCreate}>＋ 新建群聊</button>
        )}
      </div>
      <p className="eh-hints"><kbd>Ctrl/⌘+1</kbd> 左栏控制轨 · <kbd>Ctrl/⌘+2</kbd> 成员面板</p>
    </div>
  );
}
