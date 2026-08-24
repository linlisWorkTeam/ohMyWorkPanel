import { Fragment, memo, useState, type JSX } from "react";
import { ContextActionMenu, useLongPress, type ActionItem } from "../components/ContextActionMenu";
import { MessagePartsBody, TypingIndicator } from "../components/furniture";
import { PHASE_LABEL, dayLabel, time } from "../components/uiShared";
import { hasRenderableContent } from "../messageContent";
import type { GroupState, Member, TaskRun } from "../types";

export type ChatTranscriptProps = {
  messages: GroupState["messages"];
  members: Member[];
  runs: TaskRun[];
  viewerMemberId: string | null;
  onRun: (run: TaskRun, operation: "cancel" | "retry") => void;
  voiceUxEnabled?: boolean;
  playingMessageId?: string | null;
  onPlayVoice?: (messageId: string, content: string) => void;
  onQuote?: (message: GroupState["messages"][number], senderName: string) => void;
};

/** `[语音]`, `[语音 3″]`, `3″`, `🎤 3`. */
function looksLikeVoicePlaceholder(content: string): boolean {
  const t = content.trim();
  if (!t) return false;
  return /^\[语音[^\]]*\]$/.test(t) || /^\d+\s*[″"]$/.test(t) || /^🎤\s*\d/.test(t);
}

function voiceSeconds(content: string): number {
  const match = content.match(/(\d+)\s*[″"'s秒]?/);
  const n = match ? Number(match[1]) : 1;
  return Number.isFinite(n) && n > 0 ? n : 1;
}

async function copyText(text: string) {
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
}

export function ChatTranscript(props: ChatTranscriptProps): JSX.Element {
  const { messages } = props;
  return (
    <div className="wp-chat">
      {messages.map((message, index) => {
        const prev = messages[index - 1];
        const showDay = !prev || dayLabel(prev.createdAt) !== dayLabel(message.createdAt);
        return (
          <Fragment key={message.id}>
            {showDay && (
              <div className="wp-day">
                <span>{dayLabel(message.createdAt)}</span>
              </div>
            )}
            <TranscriptRow {...props} message={message} />
          </Fragment>
        );
      })}
    </div>
  );
}

const TranscriptRow = memo(function TranscriptRow({
  message,
  members,
  runs,
  viewerMemberId,
  onRun,
  voiceUxEnabled,
  playingMessageId,
  onPlayVoice,
  onQuote,
}: ChatTranscriptProps & { message: GroupState["messages"][number] }) {
  const sender = members.find((member) => member.id === message.senderMemberId);
  const run = runs.find((candidate) => candidate.outputMessageId === message.id);
  const own = Boolean(viewerMemberId) && message.senderMemberId === viewerMemberId;
  const responding = message.status === "streaming" || run?.status === "queued" || run?.status === "running";
  const canStop = run?.status === "running" || run?.status === "queued";
  const canRetry = Boolean(run && ["failed", "cancelled", "interrupted", "changes_requested"].includes(run.status));
  const hasContent = hasRenderableContent(message.content);
  const showParts = hasContent || message.hasThinking || message.hasArtifact;
  const isVoice = Boolean(onPlayVoice) && looksLikeVoicePlaceholder(message.content);
  const playing = playingMessageId === message.id;
  const failed = run?.status === "failed";
  const senderName = sender?.displayName ?? "已移除成员";
  const showSpeak = Boolean(voiceUxEnabled && hasContent && !responding && onPlayVoice);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const openMenu = (x: number, y: number) => setMenu({ x, y });
  const hold = useLongPress(openMenu);

  const copySource = message.content || run?.errorMessage || "";
  const items: ActionItem[] = [
    { id: "copy", label: "复制", onSelect: () => void copyText(copySource) },
    { id: "quote", label: "引用", onSelect: () => onQuote?.(message, senderName) },
  ];
  if (showSpeak) {
    items.push({
      id: "speak",
      label: playing ? "播放中…" : "朗读",
      disabled: playing,
      onSelect: () => onPlayVoice?.(message.id, message.content),
    });
  }
  if (canRetry && run) {
    items.push({ id: "retry", label: "重试", onSelect: () => onRun(run, "retry") });
  }

  const whoExtra = responding ? " · 流式" : failed ? " · 失败" : "";
  const bubClass = [
    "wp-bub",
    isVoice ? "voice" : "",
    failed ? "failed" : "",
    isVoice && playing ? "playing" : "",
  ].filter(Boolean).join(" ");

  const body = showParts ? (
    <>
      {(message.hasThinking || message.hasArtifact) && (
        <div className="wp-think">
          {message.hasThinking ? "思考过程" : "中间产物"}
          {message.hasThinking && message.hasArtifact ? " · 中间产物" : ""}
          {responding ? "（生成中）" : ""}
        </div>
      )}
      <MessagePartsBody
        groupId={message.groupId}
        messageId={message.id}
        content={message.content}
        hasThinking={Boolean(message.hasThinking)}
        hasArtifact={Boolean(message.hasArtifact)}
        streaming={responding}
      />
      {responding && <span className="wp-caret" aria-hidden />}
    </>
  ) : (
    <TypingIndicator
      label={
        run?.phase ? (PHASE_LABEL[run.phase] ?? run.phase)
          : run?.status === "queued" ? "排队中" : "…"
      }
    />
  );

  return (
    <div className={own ? "wp-row me" : "wp-row"} data-msg-id={message.id}>
      <div
        className={own ? "wp-av me" : "wp-av"}
        style={{ background: sender?.avatarColor ?? "var(--lp-text-tertiary)" }}
      >
        {sender?.displayName.slice(0, 1) ?? "?"}
      </div>
      <div className="wp-col">
        <div className="wp-who">{senderName}{whoExtra}</div>
        <div
          className={bubClass}
          title={isVoice ? "点击播放" : undefined}
          onClick={isVoice ? () => onPlayVoice?.(message.id, message.content) : undefined}
          onContextMenu={(event) => {
            event.preventDefault();
            openMenu(event.clientX, event.clientY);
          }}
          {...hold}
        >
          {isVoice ? (
            <>
              <span className="tri" />
              <span className="bars" aria-hidden="true"><i /><i /><i /><i /><i /></span>
              <span className="sec">{voiceSeconds(message.content)}″</span>
            </>
          ) : (
            body
          )}
          {failed && run?.errorMessage && <p className="run-error">{run.errorMessage}</p>}
          {canStop && run && (
            <button
              type="button"
              className="wp-stop"
              onClick={(event) => {
                event.stopPropagation();
                onRun(run, "cancel");
              }}
              onPointerDown={(event) => event.stopPropagation()}
            >
              停止
            </button>
          )}
        </div>
        <div className="wp-time">{time(message.createdAt)}</div>
      </div>
      {menu && <ContextActionMenu items={items} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />}
    </div>
  );
});
