import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";

export type ActionItem = {
  id: string;
  label: string;
  danger?: boolean;
  disabled?: boolean;
  onSelect?: () => void;
  children?: ActionItem[];
};

export type ContextMenuProps = {
  items: ActionItem[];
  x: number;
  y: number;
  onClose: () => void;
};

export function ContextMenu({ items, x, y, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement | null>(null);
  const [openSub, setOpenSub] = useState<string | null>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) return;
    const rect = node.getBoundingClientRect();
    node.style.left = `${Math.min(Math.max(8, x), window.innerWidth - rect.width - 8)}px`;
    node.style.top = `${Math.min(Math.max(8, y - rect.height - 8), window.innerHeight - rect.height - 8)}px`;
    node.focus();
  }, [x, y]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCloseRef.current();
    };
    const onPointer = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) onCloseRef.current();
    };
    window.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer);
    };
  }, []);

  if (items.length === 0) return null;

  const renderLeaf = (item: ActionItem) => (
    <button
      key={item.id}
      type="button"
      role="menuitem"
      className={item.danger ? "danger" : undefined}
      disabled={item.disabled}
      onClick={() => {
        if (item.disabled) return;
        item.onSelect?.();
        onClose();
      }}
    >
      {item.label}
    </button>
  );

  return (
    <div ref={ref} className="ctx-menu ui-context-menu" role="menu" tabIndex={-1} style={{ left: x, top: y }}>
      {items.map((item) => {
        if (!item.children?.length) return renderLeaf(item);
        const expanded = openSub === item.id;
        return (
          <div key={item.id} className="ctx-group">
            <button
              type="button"
              role="menuitem"
              aria-expanded={expanded}
              className={expanded ? "open" : undefined}
              onClick={(event) => {
                event.stopPropagation();
                setOpenSub(expanded ? null : item.id);
              }}
            >
              <span>{item.label}</span>
              <i className="ctx-arrow" aria-hidden>{expanded ? "▾" : "▸"}</i>
            </button>
            {expanded && <div className="ctx-sub" role="menu">{item.children.map(renderLeaf)}</div>}
          </div>
        );
      })}
    </div>
  );
}

export function useLongPress(onOpen: (x: number, y: number) => void, ms = 420) {
  const timer = useRef<number | undefined>(undefined);
  const clear = () => {
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = undefined;
  };
  useEffect(() => clear, []);
  return {
    onPointerDown: (event: ReactPointerEvent) => {
      if (event.button !== 0) return;
      const { clientX, clientY } = event;
      clear();
      timer.current = window.setTimeout(() => onOpen(clientX, clientY), ms);
    },
    onPointerUp: clear,
    onPointerLeave: clear,
    onPointerCancel: clear,
  };
}
