import { memo, useEffect, useState } from "react";
import { api } from "../api";
import { hasRenderableContent, parseMessageContent } from "../messageContent";
import { markdownToHtml } from "../markdownLite";
import { modelsForAdapter } from "../agentModels";
import { agentBusyLabel, queueCounts, runsForAgentActive } from "../queueCounts";
import { memberRosterAction } from "../memberForm";
import { PHASE_LABEL, time, dayLabel, readError } from "./uiShared";
import { applySelfDetailsToggle } from "../detailsToggle";
import type { Group, GroupState, Member, RunPhaseEntry, TaskRun } from "../types";
import { ContextActionMenu, useLongPress, type ActionItem } from "./ContextActionMenu";

/* ============================================================
   WorkPanel UI furniture: ? App.tsx ?????/??/?????P1 ????
   ???/????????????????? --lp-* ?? token?
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
  onQuote,
}: {
  message: GroupState["messages"][number];
  members: Member[];
  runs: TaskRun[];
  viewerMemberId: string | null;
  onRun: (run: TaskRun, operation: "cancel" | "retry") => void;
  voiceUxEnabled?: boolean;
  playingMessageId?: string | null;
  onPlayVoice?: (messageId: string, content: string) => void;
  onQuote?: (message: GroupState["messages"][number], senderName: string) => void;
}) {
  const sender = members.find((member) => member.id === message.senderMemberId);
  const run = runs.find((candidate) => candidate.outputMessageId === message.id);
  const own = Boolean(viewerMemberId) && message.senderMemberId === viewerMemberId;
  const responding = message.status === "streaming" || run?.status === "queued" || run?.status === "running";
  const canStop = run?.status === "running" || run?.status === "queued";
  const canRetry = Boolean(run && ["failed", "cancelled", "interrupted", "changes_requested"].includes(run.status));
  const hasContent = hasRenderableContent(message.content);
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
        : run?.status === "queued" ? "???" : "?"
    } />
  );
  const showSpeak = Boolean(voiceUxEnabled && hasContent && !responding && onPlayVoice);
  const playing = playingMessageId === message.id;
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
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
  };
  const senderName = sender?.displayName ?? "?????";
  const items: ActionItem[] = [
    { id: "copy", label: "??", onSelect: () => void copyMessage() },
    { id: "quote", label: "??", onSelect: () => onQuote?.(message, senderName) },
  ];
  if (showSpeak) {
    items.push({
      id: "speak",
      label: playing ? "????" : "??",
      disabled: playing,
      onSelect: () => onPlayVoice?.(message.id, message.content),
    });
  }
  if (canRetry && run) {
    items.push({ id: "retry", label: "??", onSelect: () => onRun(run, "retry") });
  }
  const openMenu = (x: number, y: number) => setMenu({ x, y });
  const hold = useLongPress(openMenu);
  return (
    <article className={`message-row ${own ? "own" : ""} ${responding ? "is-responding" : ""}`}>
      <Avatar member={sender} responding={responding && !own} />
      <div className="message-content">
        <div className="message-meta">
          <strong>{senderName}</strong>
          <span>{time(message.createdAt)}</span>
          {run && <Status status={run.status} />}
          {run?.phase && (run.status === "queued" || run.status === "running") && (
            <em className="phase-badge">{PHASE_LABEL[run.phase] ?? run.phase}</em>
          )}
          {run?.reviewStatus && <ReviewBadge reviewStatus={run.reviewStatus} />}
        </div>
        <div
          className={`bubble ${message.status}${responding ? " streaming" : ""}`}
          onContextMenu={(event) => {
            event.preventDefault();
            openMenu(event.clientX, event.clientY);
          }}
          {...hold}
        >
          {body}
          {canStop && run && (
            <button
              type="button"
              className="bubble-stop"
              onClick={(event) => {
                event.stopPropagation();
                onRun(run, "cancel");
              }}
              onPointerDown={(event) => event.stopPropagation()}
            >
              ??
            </button>
          )}
        </div>
        {run?.errorMessage && <p className="run-error">{run.errorMessage}</p>}
      </div>
      {menu && <ContextActionMenu items={items} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />}
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
                label="????"
                streaming={streaming}
              />
            )}
            {hasArtifact && (
              <LazyChannelPart
                groupId={groupId}
                messageId={messageId}
                channel="artifact"
                label="????"
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
          label="????"
          streaming={streaming}
        />
      )}
      {hasArtifact && (
        <LazyChannelPart
          groupId={groupId}
          messageId={messageId}
          channel="artifact"
          label="????"
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
        {streaming ? "?????" : ""}
        {!open ? " ? ????" : ""}
      </summary>
      {loading && text == null && <pre className="part-loading">????</pre>}
      {error && <pre className="part-error">{error}</pre>}
      {text != null && <pre>{text || "???"}</pre>}
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

export /** run ?????P2????????????? */
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
        {open ? "? ????" : "? ????"}
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
          {entries === null && <li className="phase-empty">????</li>}
          {entries !== null && entries.length === 0 && <li className="phase-empty">??????</li>}
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
  const nameOf = (id: string) => members.find((m) => m.id === id)?.displayName ?? "?????";
  if (active.length === 0 && review.length === 0) {
    return <div className="queue-empty">?????????????</div>;
  }
  return (
    <>
      <div className="sec-title"><span>??? ? ??</span><span>{active.length}</span></div>
      {active.map((run) => (
        <div key={run.id} className="queue-card">
          <div className="qc-top">
            <span className="qc-name">{nameOf(run.agentMemberId)}</span>
            <span className="qc-id">{run.id.slice(0, 8)}</span>
            <span className={`st ${run.status}`}>{run.status === "running" ? "???" : "???"}</span>
          </div>
          <div className="qc-phase">{run.phase ? (PHASE_LABEL[run.phase] ?? run.phase) : run.status === "queued" ? "????" : "????"}</div>
          <div className="qc-bar"><i /></div>
          <div className="qc-sub">
            <span>{time(run.createdAt)}</span>
            <button type="button" className="mini-btn qc-cancel" onClick={() => onCancel(run)}>??</button>
          </div>
          <PhaseTrail runId={run.id} />
        </div>
      ))}
      {review.length > 0 && (
        <>
          <div className="sec-title"><span>???</span><span>{review.length}</span></div>
          {review.map((run) => (
            <div key={run.id} className="queue-card review">
              <div className="qc-top">
                <span className="qc-name">{nameOf(run.agentMemberId)}</span>
                <span className="st review">???</span>
              </div>
              <div className="qc-sub">?? {run.reviewerMemberId ? nameOf(run.reviewerMemberId) : "???"} ? ??????????</div>
              {run.reviewStatus === "pending" && (
                <div className="qc-actions">
                  <button type="button" className="mini-btn qc-approve" onClick={() => onReview(run, "approved")}>? ??</button>
                  <button type="button" className="mini-btn qc-reject" onClick={() => onReview(run, "rejected")}>? ?????????</button>
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
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  useEffect(() => {
    if (!responding) setQueueOpen(false);
  }, [responding]);
  const activeRuns = queueOpen ? runsForAgentActive(runs, member.id) : [];
  const idleRuntime =
    member.runtimeStatus === "ready" ? "???" : member.runtimeStatus === "unavailable" ? "???" : "???";
  const busyOrIdle = detecting ? "????" : busy ?? idleRuntime;
  const statusText = member.kind === "agent"
    ? `${member.adapter}${member.model ? ` ? ${member.model}` : ""}`
    : member.kind === "chatbot"
      ? `${member.adapter ?? "chatbot"} ? ${member.model || "deepseek-v4-flash"}`
      : member.invitePending
        ? "?????"
        : member.roleDescription || "????";
  const stateLabel = member.kind === "user"
    ? (online ? "??" : member.invitePending ? "??" : "??")
    : detecting ? "???" : busy ? "???" : idleRuntime;
  const stateKind = detecting ? "" : busy ? "busy" : member.runtimeStatus === "unavailable" ? "bad" : online || member.runtimeStatus === "ready" ? "ok" : "";
  const rosterAction = memberRosterAction(member);
  const items: ActionItem[] = [];
  if (member.kind === "agent") {
    items.push({
      id: "detect",
      label: detecting ? "???" : "??",
      disabled: Boolean(detecting),
      onSelect: () => onDetect(member),
    });
  }
  if ((member.kind === "agent" || member.kind === "chatbot") && !member.systemLocked) {
    items.push({
      id: "admin",
      label: isAdmin
        ? (group.groupKind === "chat" ? "??????" : "????")
        : (group.groupKind === "chat" ? "??????" : "???"),
      onSelect: () => onAdmin(isAdmin ? null : member.id),
    });
  }
  if (modelOptions.length > 0 && (member.kind === "agent" || member.kind === "chatbot") && !member.systemLocked) {
    for (const model of modelOptions) {
      items.push({
        id: `model:${model}`,
        label: member.model === model ? `?? ? ${model} ?` : `??? ${model}`,
        onSelect: () => onModel(member, model),
      });
    }
  }
  if (member.kind === "agent" && member.adapter === "dsh") {
    items.push({ id: "dsh", label: "?? DSH Web", onSelect: () => onOpenDsh?.(member) });
  }
  if (member.invitePending) {
    items.push({
      id: "revoke-invite",
      label: "????",
      danger: true,
      onSelect: () => onRemove(member),
    });
  } else if (!member.systemLocked && member.id !== group.ownerMemberId) {
    items.push({
      id: "remove",
      label: rosterAction === "delete" ? "??" : "??",
      danger: true,
      onSelect: () => onRemove(member),
    });
  }
  if (items.length === 0) {
    items.push({
      id: "copy-name",
      label: "????",
      onSelect: () => void navigator.clipboard.writeText(member.displayName),
    });
  }
  const openMenu = (x: number, y: number) => setMenu({ x, y });
  const hold = useLongPress(openMenu);
  return (
    <div
      className={`member-row roster-row ${member.isActive ? "" : "inactive"} ${member.invitePending ? "invite-pending" : ""} ${responding ? "is-responding" : ""}`}
      onContextMenu={(event) => {
        event.preventDefault();
        openMenu(event.clientX, event.clientY);
      }}
      {...hold}
    >
      <Avatar member={member} responding={responding} online={online} />
      <div className="member-details">
        <strong>
          {member.displayName}
          {member.id === group.ownerMemberId && <em>??</em>}
          {isAdmin && (
            <em className="admin-badge">{group.groupKind === "chat" ? "????" : "???"}</em>
          )}
          {askMode && <em className="ask-badge">Ask</em>}
          {member.kind === "chatbot" && <em className="admin-badge">???</em>}
            {member.systemLocked && <em className="admin-badge" title="??????? Agent?????/??">??</em>}
          {member.invitePending && <em className="invite-badge">???</em>}
        </strong>
        <span>
          {member.kind === "agent" && busy ? (
            <button
              type="button"
              className="member-queue-toggle"
              onClick={() => setQueueOpen((open) => !open)}
              aria-expanded={queueOpen}
              title={queueOpen ? "??????" : "??????"}
            >
              {statusText}
            </button>
          ) : (
            statusText
          )}
          {member.tags ? ` ? ?? ${member.tags}` : ""}
        </span>
        {queueOpen && activeRuns.length > 0 && (
          <ul className="member-queue-list">
            {activeRuns.map((run) => (
              <li key={run.id}>
                <span>{run.status === "running" ? "???" : "???"} ? {run.id.slice(0, 8)}</span>
                <button type="button" className="danger" onClick={() => onCancelRun(run)}>??</button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className={`m-state ${stateKind}`}>{stateLabel}</div>
      {menu && <ContextActionMenu items={items} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />}
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
export function Status({ status }: { status: string }) { return <span className={`status ${status}`}>{({ queued: "???", running: "???", awaiting_review: "???", changes_requested: "???", completed: "??", failed: "??", cancelled: "???", interrupted: "???" } as Record<string, string>)[status] ?? status}</span>; }
export function ReviewBadge({ reviewStatus }: { reviewStatus: string }) { return <span className={`review-badge ${reviewStatus}`}>{({ pending: "???", approved: "???", rejected: "???" } as Record<string, string>)[reviewStatus] ?? reviewStatus}</span>; }

/** ??????????"?????????"????? */
export function EmptyHome({ canCreate, onCreate }: { canCreate?: boolean; onCreate?: () => void }) {
  return (
    <div className="empty-home">
      <div className="eh-mark">L</div>
      <strong className="eh-title">???? WorkPanel</strong>
      <p className="eh-desc">????????????????????? @Agent ??????????????????</p>
      <div className="eh-actions">
        {canCreate && (
          <button type="button" className="eh-primary" onClick={onCreate}>? ????</button>
        )}
      </div>
      <p className="eh-hints"><kbd>Ctrl/?+1</kbd> ????? ? <kbd>Ctrl/?+2</kbd> ????</p>
    </div>
  );
}
