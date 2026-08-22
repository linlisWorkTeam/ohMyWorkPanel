import type { ReactNode } from "react";

export function Shell({
  leftOpen,
  onToggleLeft,
  onOpenSettings,
  showAgentConfig,
  onOpenAgentConfig,
  brand,
  groups,
  footer,
  header,
  wave,
  chat,
  composer,
  right,
}: {
  leftOpen: boolean;
  onToggleLeft: () => void;
  onOpenSettings: () => void;
  showAgentConfig: boolean;
  onOpenAgentConfig: () => void;
  brand: ReactNode;
  groups: ReactNode;
  footer: ReactNode;
  header: ReactNode;
  wave: ReactNode;
  chat: ReactNode;
  composer: ReactNode;
  right: ReactNode;
}) {
  return (
    <div className={`wp-shell${leftOpen ? " is-left-open" : ""}`}>
      <nav className="wp-rail" aria-label="???">
        <span className="wp-rail-logo">L</span>
        <button type="button" className="wp-rail-btn" title="?????" onClick={onToggleLeft}>?</button>
        <button type="button" className="wp-rail-btn" title="??" onClick={onOpenSettings}>?</button>
        {showAgentConfig && (
          <button type="button" className="wp-rail-btn" title="Agent ??" onClick={onOpenAgentConfig}>?</button>
        )}
      </nav>
      <aside className="wp-left">
        {brand}
        {groups}
        {footer}
      </aside>
      <section className="wp-mid">
        <header className="wp-head">{header}</header>
        {wave && <div className="wp-wave">{wave}</div>}
        {chat}
        {composer}
      </section>
      <aside className="wp-right">{right}</aside>
    </div>
  );
}
