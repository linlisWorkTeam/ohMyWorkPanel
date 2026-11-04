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
const LEFT_MIN = 200, LEFT_MAX = 340, LEFT_START = 232, LEFT_SNAP = 208;
const RIGHT_MIN = 280, RIGHT_MAX = 360, RIGHT_START = 320, RIGHT_SNAP = 280;
export const CONCEDE_RIGHT = 1280;
export const CONCEDE_LEFT = 1280;

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
  const [leftMode, setLeftMode] = useState<"open" | "rail">(() =>
    typeof window !== "undefined" && window.matchMedia("(max-width: 1279px)").matches ? "rail" : "open",
  );
  const [dragging, setDragging] = useState(false);
  const leftWRef = useRef(leftW);
  leftWRef.current = leftW;
  const rightWRef = useRef(rightW);
  rightWRef.current = rightW;
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

  // 窄屏让步：按当前窗口宽度收敛（1280 以下侧栏折叠、右栏关闭）。
  useEffect(() => {
    const check = () => {
      const width = typeof window !== "undefined" ? window.innerWidth : 1440;
      if (width < CONCEDE_RIGHT && options.rightOpen) options.onRightClose?.();
      if (width < CONCEDE_LEFT) setLeftMode((mode) => (mode === "open" ? "rail" : mode));
    };
    check();
    window.addEventListener("resize", check);
    return () => window.removeEventListener("resize", check);
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
