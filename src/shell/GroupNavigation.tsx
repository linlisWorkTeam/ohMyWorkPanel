//! 群内导航：协作 / 交付 / 目标 / 场景（后续扩展 内容、扩展）。
//!
//! 只负责展示与回调，不持有数据；active 由壳层状态决定。

export interface GroupNavItem {
  id: string;
  label: string;
  disabled?: boolean;
  /** 不可用原因，作为 title 与 aria-description。 */
  reason?: string;
}

export function GroupNavigation({
  items,
  active,
  onSelect,
}: {
  items: GroupNavItem[];
  active: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <nav className="group-nav" aria-label="群内导航">
      {items.map((item) => {
        const isActive = item.id === active;
        return (
          <button
            key={item.id}
            type="button"
            className={`group-nav-item${isActive ? " is-active" : ""}`}
            aria-current={isActive ? "page" : undefined}
            title={item.disabled ? item.reason : undefined}
            disabled={item.disabled}
            onClick={() => onSelect(item.id)}
          >
            {item.label}
          </button>
        );
      })}
    </nav>
  );
}
