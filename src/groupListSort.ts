import type { Group } from "./types";

/** Unread groups first, then newer createdAt. */
export function sortGroupsForSidebar(groups: Group[]): Group[] {
  return [...groups].sort((a, b) => {
    const au = (a.unreadCount ?? 0) > 0 ? 1 : 0;
    const bu = (b.unreadCount ?? 0) > 0 ? 1 : 0;
    if (au !== bu) return bu - au;
    return (b.createdAt ?? 0) - (a.createdAt ?? 0);
  });
}

export function formatUnreadBadge(count: number): string {
  if (count <= 0) return "";
  if (count > 99) return "99+";
  return String(count);
}

/** Bump unread for a group that is not currently open. */
export function bumpUnread(groups: Group[], groupId: string, activeGroupId: string | null | undefined): Group[] {
  if (!groupId || groupId === activeGroupId) return groups;
  return sortGroupsForSidebar(
    groups.map((g) =>
      g.id === groupId ? { ...g, unreadCount: (g.unreadCount ?? 0) + 1 } : g,
    ),
  );
}

export function clearUnread(groups: Group[], groupId: string): Group[] {
  return sortGroupsForSidebar(
    groups.map((g) => (g.id === groupId ? { ...g, unreadCount: 0 } : g)),
  );
}
