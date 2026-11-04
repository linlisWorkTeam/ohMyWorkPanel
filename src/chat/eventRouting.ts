//! WS 事件路由（M1：事件 → 动作的判定归 chat 领域）。
//!
//! 只做分类，不产生副作用；App 按分类执行刷新/合并。
//! 顺序与既有行为一致：presence → orchestration → delta → run → resync → 兜底刷新。

import { isIgnorableWsKind, shouldResyncAfterWsEvent } from "../observability/realtimeWs";

export type ChatEventRoute =
  | "ignore"
  | "presence"
  | "orchestration"
  | "delta"
  | "run"
  | "resync"
  | "refresh";

export type RoutableEvent = {
  kind: string;
  userId?: string | null;
  online?: boolean | null;
  messageId?: string | null;
  runId?: string | null;
  onlineUserIds?: string[] | null;
};

export function routeChatEvent(event: RoutableEvent): ChatEventRoute {
  if (isIgnorableWsKind(event.kind)) return "ignore";
  if (event.kind === "presence_snapshot" && Array.isArray(event.onlineUserIds)) return "presence";
  if (event.kind === "presence" && event.userId) return "presence";
  if (event.kind === "orchestration_status") return "orchestration";
  if (event.kind === "message_delta" && event.messageId) return "delta";
  if (event.kind === "run_status" && event.runId) return "run";
  if (shouldResyncAfterWsEvent(event.kind)) return "resync";
  return "refresh";
}
