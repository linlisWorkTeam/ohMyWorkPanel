//! 在线状态归并（M1：presence 事实归 accounts 领域）。
//!
//! 纯函数：快照替换、单用户上线/下线；未知事件保持原样。

export type PresenceEvent = {
  kind: string;
  userId?: string | null;
  online?: boolean | null;
  onlineUserIds?: string[] | null;
};

/** 返回新的在线集合；无变化时返回原引用，避免多余渲染。 */
export function applyPresence(
  current: ReadonlySet<string>,
  event: PresenceEvent,
): Set<string> {
  if (event.kind === "presence_snapshot") {
    const ids = (event.onlineUserIds ?? []).filter((id): id is string => Boolean(id));
    return new Set(ids);
  }
  if (event.kind === "presence" && event.userId) {
    const next = new Set(current);
    if (event.online) next.add(event.userId);
    else next.delete(event.userId);
    return next;
  }
  return current as Set<string>;
}

/** 事件是否值得触发未读/列表刷新（不含 presence）。 */
export function shouldBumpUnread(event: { kind: string; groupId?: string | null; status?: string | null }): boolean {
  if (!event.groupId) return false;
  if (event.kind === "message_created") return true;
  return event.kind === "run_status" && event.status === "completed";
}
