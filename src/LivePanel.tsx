import { useEffect, useRef } from "react";
import { api } from "./api";
import { liveEntryUrl } from "./extensions";
import {
  LIVE_MSG,
  appendGroupIdToLiveUrl,
  buildLiveMentionMessage,
  projectLiveChatLines,
  resolveLiveResponder,
} from "./liveBridge";
import type { ExtensionStatus, Group, Member, Message } from "./types";

interface Props {
  extension: ExtensionStatus | null;
  group: Group | null;
  members: Member[];
  messages: Message[];
  senderMemberId: string | null;
  onOpenSettings?: () => void;
  onError?: (msg: string) => void;
}

/** Host shell for PanelLive — bridges session state + chat history into the iframe. */
export function LivePanel({
  extension,
  group,
  members,
  messages,
  senderMemberId,
  onOpenSettings,
  onError,
}: Props) {
  const frameRef = useRef<HTMLIFrameElement | null>(null);
  const lastSpokenId = useRef<string | null>(null);
  const awaitingReply = useRef(false);
  const openedAt = useRef(Date.now());
  const baseUrl = liveEntryUrl(extension);
  const url =
    baseUrl && group?.id ? appendGroupIdToLiveUrl(baseUrl, group.id) : baseUrl;

  const postChatSync = () => {
    const win = frameRef.current?.contentWindow;
    if (!win || !group) return;
    win.postMessage(
      {
        type: LIVE_MSG.chatSync,
        groupId: group.id,
        lines: projectLiveChatLines(messages, members),
      },
      "*",
    );
  };

  useEffect(() => {
    postChatSync();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- sync whenever chat changes
  }, [messages, members, group?.id]);

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const data = event.data;
      if (!data || typeof data !== "object") return;
      const type = (data as { type?: string }).type;
      if (type === LIVE_MSG.ready) {
        postChatSync();
        return;
      }
      if (type !== LIVE_MSG.userText) return;
      if (!group || !senderMemberId) {
        onError?.("Live：当前账号未绑定本群用户成员，无法写入聊天");
        return;
      }
      const text = String((data as { text?: string }).text ?? "").trim();
      if (!text) return;
      const responder = resolveLiveResponder(group, members);
      const { content, mentionIds } = buildLiveMentionMessage(text, responder);
      awaitingReply.current = true;
      void api
        .sendMessage(group.id, senderMemberId, content, mentionIds)
        .catch((reason) => {
          awaitingReply.current = false;
          onError?.(typeof reason === "string" ? reason : reason instanceof Error ? reason.message : "Live 发送失败");
        });
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [group, members, senderMemberId]);

  // TTS only for replies after a Live utterance (avoid speaking old history on open).
  useEffect(() => {
    if (!group || messages.length === 0 || !awaitingReply.current) return;
    const last = messages[messages.length - 1];
    if (last.createdAt < openedAt.current) return;
    if (lastSpokenId.current === last.id) return;
    const sender = members.find((m) => m.id === last.senderMemberId);
    if (!sender || (sender.kind !== "agent" && sender.kind !== "chatbot")) return;
    lastSpokenId.current = last.id;
    awaitingReply.current = false;
    const plain = projectLiveChatLines([last], members)[0]?.text ?? "";
    if (!plain.trim()) return;
    frameRef.current?.contentWindow?.postMessage(
      { type: LIVE_MSG.speak, text: plain, messageId: last.id },
      "*",
    );
  }, [messages, members, group]);

  if (!extension?.enabled) {
    return (
      <div className="live-panel empty-chat">
        <strong>Live 扩展未启用</strong>
        <span>在运行设置中开启 PanelLive（Extend）后可使用语音 Live。</span>
        {onOpenSettings && (
          <button type="button" className="pm-btn primary sm" onClick={onOpenSettings}>
            打开运行设置
          </button>
        )}
      </div>
    );
  }
  if (!extension.healthy || !url) {
    return (
      <div className="live-panel empty-chat">
        <strong>PanelLive 未就绪</strong>
        <span>
          {extension.healthDetail
            || "请在服务器上启动 /AI/WorkPanelLive（npm start :8790）。Live UI 经同源代理加载，勿直连 127.0.0.1。"}
        </span>
      </div>
    );
  }
  return (
    <div className="live-panel">
      <p className="live-panel-hint">
        Live 与聊天共用同一会话记录；语音转写会写入群聊并 @ 默认响应者。麦克风需要 HTTPS 或 localhost。
      </p>
      <iframe
        ref={frameRef}
        title="PanelLive"
        className="live-frame"
        src={url}
        allow="microphone; autoplay"
        onLoad={() => postChatSync()}
      />
    </div>
  );
}
