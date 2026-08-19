import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import type { DividerProps } from "./Divider";

/**
 * 三栏 AppFrame 几何（DSH ui-layout 借鉴）：
 * - 左栏/右栏宽度可拖拽（约束见常量），几何只进 localStorage（不入 DB）；
 * - 左栏可折叠为 56px 控制轨（data-left="rail"）；
 * - 右栏关闭由业务方持有（App 的 showMembers），本 hook 仅感知 rightOpen；
 * - 窄屏让步：仅在「宽度穿越阈值」发生时自动收敛，避免跟手动操作打架。
 */
const LS = { leftW: "lp.frame.leftW", rightW: "lp.frame.rightW" };
const RAIL_W = 56;
const LEFT_MIN = 160, LEFT_MAX = 340, LEFT_START = 248, LEFT_SNAP = 176;
const RIGHT_MIN = 240, RIGHT_MAX = 420, RIGHT_START = 310, RIGHT_SNAP = 250;
export const CONCEDE_RIGHT = 1100;
export const CONCEDE_LEFT = 860;

function readNum(key: string, fallback: number): number {
  try {
    const raw = localStorage.getItem(key);
    if (raw) {
      const n = Number(raw);
      if (Number.isFinite(n)) return n;
    }
  } catch {
    /* ignore */
  }
  return fallback;
}
function writeNum(key: string, value: number): void {
  try {
    localStorage.setItem(key, String(Math.round(value)));
  } catch {
    /* ignore */
  }
}

export type UseAppFrameOptions = {
  rightOpen?: boolean;
  onRightClose?: () => void;
};

export function useAppFrame(options: UseAppFrameOptions = {}) {
  const rootRef = useRef<HTMLElement | null>(null);
  const [leftW, setLeftWState] = useState(() => readNum(LS.leftW, LEFT_START));
  const [rightW, setRightWState] = useState(() => readNum(LS.rightW, RIGHT_START));
  const [leftMode, setLeftMode] = useState<"open" | "rail">("open");
  const [dragging, setDragging] = useState(false);
  const leftWRef = useRef(leftW);
  leftWRef.current = leftW;
  const rightWRef = useRef(rightW);
  rightWRef.current = rightW;
  const prevWidth = useRef(0);
  const rightOpen = options.rightOpen !== false;

  const setLeftW = useCallback((w: number) => {
    setLeftWState(w);
    writeNum(LS.leftW, w);
  }, []);
  const setRightW = useCallback((w: number) => {
    setRightWState(w);
    writeNum(LS.rightW, w);
  }, []);

  const toggleLeft = useCallback(() => {
    setLeftMode((mode) => (mode === "open" ? "rail" : "open"));
  }, []);

  // 窄屏让步（仅在穿越阈值时自动收敛一次，避免与手动操作反复打架）
  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const check = () => {
      const w = el.clientWidth;
      if (prevWidth.current === 0) prevWidth.current = w;
      const narrowed = prevWidth.current > w;
      if (narrowed && w < CONCEDE_RIGHT && options.rightOpen && options.onRightClose) {
        options.onRightClose();
      }
      if (narrowed && w < CONCEDE_LEFT) {
        setLeftMode((mode) => (mode === "open" ? "rail" : mode));
      }
      prevWidth.current = w;
    };
    check();
    const ro = new ResizeObserver(check);
    ro.observe(el);
    return () => ro.disconnect();
  }, [options.rightOpen, options.onRightClose]);

  const leftDivider: DividerProps = {
    side: "left",
    min: LEFT_MIN,
    max: LEFT_MAX,
    getWidth: () => leftWRef.current,
    onResize: setLeftW,
    onRelease: (w: number) => {
      if (w <= LEFT_SNAP) setLeftMode("rail");
    },
    dragging,
    onDraggingChange: setDragging,
  };
  const rightDivider: DividerProps = {
    side: "right",
    min: RIGHT_MIN,
    max: RIGHT_MAX,
    getWidth: () => rightWRef.current,
    onResize: setRightW,
    onRelease: (w: number) => {
      if (w <= RIGHT_SNAP) options.onRightClose?.();
    },
    dragging,
    onDraggingChange: setDragging,
  };

  const rootStyle = {
    "--left-w": `${leftW}px`,
    "--right-w": `${rightW}px`,
    "--rail-w": `${RAIL_W}px`,
  } as CSSProperties;

  return {
    rootRef,
    rootStyle,
    leftMode,
    rightOpen,
    dragging,
    leftDivider,
    rightDivider,
    toggleLeft,
  };
}
