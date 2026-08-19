import { useRef, type PointerEvent as ReactPointerEvent } from "react";

/**
 * 三栏分界条（DSH ui-layout ResizablePane 借鉴）。
 * 职责只做「指针拖拽 + 钳制」：起始宽度由 getWidth 在按下瞬间取一次，
 * 拖拽中把钳制后的新宽度交给 onResize；松手时把最终宽度交给 onRelease（供折拢）。
 */
export type DividerProps = {
  side: "left" | "right";
  min?: number;
  max?: number;
  /** 拖拽开始时解析当前面板宽度（只调用一次） */
  getWidth: () => number;
  /** 拖拽中持续回调（已钳制到 [min,max]） */
  onResize: (width: number) => void;
  /** 松手时回调最终宽度（已钳制） */
  onRelease?: (width: number) => void;
  /** 是否正在拖拽（用于高亮） */
  dragging?: boolean;
  onDraggingChange?: (active: boolean) => void;
};

export function Divider({
  side,
  min = 160,
  max = 420,
  getWidth,
  onResize,
  onRelease,
  dragging,
  onDraggingChange,
}: DividerProps) {
  const baseWidth = useRef(0);
  const startX = useRef(0);
  const clamp = (w: number) => Math.max(min, Math.min(max, w));

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    baseWidth.current = getWidth();
    startX.current = e.clientX;
    onDraggingChange?.(true);

    const onMove = (ev: PointerEvent) => {
      const delta = ev.clientX - startX.current;
      const next = side === "left" ? baseWidth.current + delta : baseWidth.current - delta;
      onResize(clamp(next));
    };
    const onUp = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      onDraggingChange?.(false);
      const delta = ev.clientX - startX.current;
      onRelease?.(clamp(side === "left" ? baseWidth.current + delta : baseWidth.current - delta));
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  return (
    <div
      className={`divider divider-${side}${dragging ? " dragging" : ""}`}
      role="separator"
      aria-orientation="vertical"
      title="拖动调整宽度"
      onPointerDown={onPointerDown}
    />
  );
}
