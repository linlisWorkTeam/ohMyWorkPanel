import { LivePanel } from "./LivePanel";
import { extensionEntryUrl } from "./extensions";
import type { ExtensionStatus, ExtensionTab, Group, Member, Message } from "./types";

interface Props {
  extension: ExtensionStatus;
  tab: ExtensionTab;
  group: Group | null;
  members: Member[];
  messages: Message[];
  senderMemberId: string | null;
  onOpenSettings?: () => void;
  onError?: (msg: string) => void;
}

/** Generic Extend iframe shell. PanelLive keeps its postMessage bridge via LivePanel. */
export function ExtensionPanel({
  extension,
  tab,
  group,
  members,
  messages,
  senderMemberId,
  onOpenSettings,
  onError,
}: Props) {
  if (extension.id === "panellive" && (tab.id === "live" || tab.route === "tab://live")) {
    return (
      <LivePanel
        extension={extension}
        group={group}
        members={members}
        messages={messages}
        senderMemberId={senderMemberId}
        onOpenSettings={onOpenSettings}
        onError={onError}
      />
    );
  }

  if (!extension.enabled) {
    return (
      <div className="live-panel empty-chat">
        <strong>{extension.name} 未启用</strong>
        <span>在运行设置 → Extend 中开启后可使用「{tab.title}」页签。</span>
        {onOpenSettings && (
          <button type="button" className="pm-btn primary sm" onClick={onOpenSettings}>
            打开运行设置
          </button>
        )}
      </div>
    );
  }

  const base = extensionEntryUrl(extension, tab);
  const url =
    base && group?.id
      ? `${base}${base.includes("?") ? "&" : "?"}groupId=${encodeURIComponent(group.id)}`
      : base;

  if (!extension.healthy || !url) {
    return (
      <div className="live-panel empty-chat">
        <strong>{extension.name} 未就绪</strong>
        <span>
          {extension.healthDetail
            || `扩展服务未响应（经同源代理 /api/extensions/${extension.id}/…，勿直连端口）。`}
        </span>
      </div>
    );
  }

  return (
    <div className="live-panel">
      <iframe
        title={`${extension.name} · ${tab.title}`}
        className="live-frame"
        src={url}
        allow="microphone; autoplay"
      />
    </div>
  );
}
