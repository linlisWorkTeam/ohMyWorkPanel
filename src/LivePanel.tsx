import type { ExtensionStatus } from "./types";
import { liveEntryUrl } from "./extensions";

interface Props {
  extension: ExtensionStatus | null;
  onOpenSettings?: () => void;
}

/** Host shell for PanelLive tab://live — UI owned by PanelLive entry URL. */
export function LivePanel({ extension, onOpenSettings }: Props) {
  const url = liveEntryUrl(extension);
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
        <span>{extension.healthDetail || "请在服务器上启动 /AI/WorkPanelLive（npm start :8790）"}</span>
      </div>
    );
  }
  return (
    <div className="live-panel">
      <iframe title="PanelLive" className="live-frame" src={url} allow="microphone" />
    </div>
  );
}
