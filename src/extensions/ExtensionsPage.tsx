//! 扩展页（设计 P09）：列出当前群可用的扩展页签，选中后在主栏渲染同源 iframe。

import { useMemo, useState } from "react";

import type { ExtensionStatus, ExtensionTab, Group, Member, Message } from "../types";
import { ExtensionPanel } from "./ExtensionPanel";

export interface ExtensionViewEntry {
  viewKey: string;
  ext: ExtensionStatus;
  tab: ExtensionTab;
}

interface ExtensionsPageProps {
  group: Group | null;
  members: Member[];
  messages: Message[];
  senderMemberId: string | null;
  views: ExtensionViewEntry[];
  onOpenSettings: () => void;
  onError: (message: string) => void;
}

export function ExtensionsPage({
  group,
  members,
  messages,
  senderMemberId,
  views,
  onOpenSettings,
  onError,
}: ExtensionsPageProps) {
  const [selected, setSelected] = useState<string | null>(views[0]?.viewKey ?? null);
  const current = useMemo(
    () => views.find((view) => view.viewKey === selected) ?? views[0] ?? null,
    [views, selected],
  );

  return (
    <div className="extensions-page">
      <aside className="extensions-list" aria-label="扩展列表">
        <h2>扩展</h2>
        {views.length === 0 ? (
          <p className="extensions-hint">
            未发现扩展。检查 <code>OHMYWORKPANEL_EXTENSION_ROOTS</code> 与清单文件。
          </p>
        ) : (
          <ul>
            {views.map((view) => (
              <li key={view.viewKey}>
                <button
                  type="button"
                  className={`extensions-item${view.viewKey === current?.viewKey ? " is-active" : ""}`}
                  aria-current={view.viewKey === current?.viewKey}
                  onClick={() => setSelected(view.viewKey)}
                >
                  <span>{view.tab.title}</span>
                  <small>
                    {view.ext.name} · {view.ext.version}
                    {view.ext.enabled ? "" : " · 未启用"}
                  </small>
                </button>
              </li>
            ))}
          </ul>
        )}
        {views.length > 0 && (
          <button type="button" className="extensions-settings" onClick={onOpenSettings}>
            运行设置 · Extend
          </button>
        )}
      </aside>

      <section className="extensions-detail" aria-label="扩展内容">
        {current ? (
          <ExtensionPanel
            extension={current.ext}
            tab={current.tab}
            group={group}
            members={members}
            messages={messages}
            senderMemberId={senderMemberId}
            onOpenSettings={onOpenSettings}
            onError={onError}
          />
        ) : (
          <div className="extensions-empty">
            <p>扩展 UI 走平台同源代理，不直连本机端口。</p>
            <p className="extensions-hint">启用扩展后可在这里使用 PanelLive 等页签。</p>
          </div>
        )}
      </section>
    </div>
  );
}
