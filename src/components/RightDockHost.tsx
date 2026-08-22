import type { ReactNode } from "react";
import { DOCK_MIN_WIDTH } from "../contrib/dockGeom";
import type { UiContribution } from "../contrib/types";

export function RightDockHost({
  tabs,
  activeId,
  onSelect,
  dockedId,
  onDock,
  onUndock,
  dockWidth,
  onDockWidth,
  pane,
  dockPane,
  onClose,
}: {
  tabs: UiContribution[];
  activeId: string;
  onSelect: (id: string) => void;
  dockedId: string | null;
  onDock: (id: string) => void;
  onUndock: () => void;
  dockWidth: number;
  onDockWidth: (width: number) => void;
  pane: ReactNode;
  dockPane: ReactNode;
  onClose: () => void;
}) {
  const docked = Boolean(dockedId && dockPane);
  const visibleTabs = tabs.filter((tab) => tab.slot === "right-tab" || tab.slot === "right-dock");

  return (
    <aside className={`member-panel${docked ? " is-docked" : ""}`}>
      <header>
        <div className="tabs" role="tablist" aria-label="右栏">
          {visibleTabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              className={`tab ${activeId === tab.id || dockedId === tab.id ? "active" : ""}`}
              aria-selected={activeId === tab.id}
              onClick={() => onSelect(tab.id)}
            >
              {tab.title}
            </button>
          ))}
        </div>
        {visibleTabs.some((tab) => tab.dockable && tab.id === activeId) && !docked && (
          <button type="button" className="icon-btn dock-btn" title="拆出第二列" onClick={() => onDock(activeId)}>
            拆出
          </button>
        )}
        {docked && (
          <button type="button" className="icon-btn dock-btn" title="收回页签" onClick={onUndock}>
            收回
          </button>
        )}
        <button className="icon-btn" onClick={onClose} aria-label="关闭右栏">×</button>
      </header>
      <div className="right-dock-body" style={docked ? { gridTemplateColumns: `minmax(${DOCK_MIN_WIDTH}px, 1fr) 4px ${dockWidth}px` } : undefined}>
        <div className="right-dock-pane">{pane}</div>
        {docked && (
          <>
            <div
              className="right-dock-split"
              onPointerDown={(event) => {
                const startX = event.clientX;
                const startW = dockWidth;
                const move = (ev: PointerEvent) => onDockWidth(Math.max(DOCK_MIN_WIDTH, startW - (ev.clientX - startX)));
                const up = () => {
                  window.removeEventListener("pointermove", move);
                  window.removeEventListener("pointerup", up);
                };
                window.addEventListener("pointermove", move);
                window.addEventListener("pointerup", up);
              }}
            />
            <div className="right-dock-extra">{dockPane}</div>
          </>
        )}
      </div>
    </aside>
  );
}
