import { useEffect, useLayoutEffect, useRef, type PointerEvent as ReactPointerEvent } from "react";

export type ActionItem = {
  id: string;
  label: string;
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
};

/** Absolutely positioned action sheet. Must not change layout height of the list. */
export function ContextActionMenu({
  items,
  x,
  y,
  onClose,
}: {
  items: ActionItem[];
  x: number;
  y: number;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement | null>(null);

  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) return;
    const rect = node.getBoundingClientRect();
    const left = Math.min(Math.max(8, x), window.innerWidth - rect.width - 8);
    const top = Math.min(Math.max(8, y - rect.height - 8), window.innerHeight - rect.height - 8);
    node.style.left = `${left}px`;
    node.style.top = `${top}px`;
  }, [x, y]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    const onPointer = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) onClose();
    };
    window.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer);
    };
  }, [onClose]);

  if (items.length === 0) return null;

  return (
    <div ref={ref} className="ctx-menu" role="menu" style={{ left: x, top: y }}>
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          role="menuitem"
          className={item.danger ? "danger" : undefined}
          disabled={item.disabled}
          onClick={() => {
            if (item.disabled) return;
            item.onSelect();
            onClose();
          }}
        >
          {item.label}
        </button>
      ))}
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
