import type { Group, Member, Message } from "./types";
import { partsToPlainText } from "./messageContent";

export const LIVE_MSG = {
  ready: "linlis.live.ready",
  chatSync: "linlis.live.chat_sync",
  userText: "linlis.live.user_text",
  speak: "linlis.live.speak",
} as const;

export type LiveChatLine = {
  id: string;
  name: string;
  text: string;
  role: "user" | "agent" | "bot" | "meta";
};

export function messageToPlainText(content: string): string {
  return partsToPlainText(content).trim();
}

/** Project recent group messages for Live panel (same source as chat). */
export function projectLiveChatLines(
  messages: Message[],
  members: Member[],
  limit = 40,
): LiveChatLine[] {
  const byId = new Map(members.map((m) => [m.id, m]));
  return messages.slice(-limit).map((m) => {
    const mem = byId.get(m.senderMemberId);
    const kind = mem?.kind ?? "user";
    const role: LiveChatLine["role"] =
      kind === "chatbot" ? "bot" : kind === "agent" ? "agent" : "user";
    return {
      id: m.id,
      name: mem?.displayName ?? "已移除成员",
      text: messageToPlainText(m.content),
      role,
    };
  });
}

/** Prefer admin responder, else first active chatbot. */
export function resolveLiveResponder(group: Group, members: Member[]): Member | null {
  if (group.adminMemberId) {
    const admin = members.find((m) => m.id === group.adminMemberId && m.isActive);
    if (admin && (admin.kind === "agent" || admin.kind === "chatbot")) return admin;
  }
  return members.find((m) => m.isActive && m.kind === "chatbot") ?? null;
}

export function buildLiveMentionMessage(text: string, responder: Member | null): {
  content: string;
  mentionIds: string[];
} {
  const trimmed = text.trim();
  if (!responder) return { content: trimmed, mentionIds: [] };
  return {
    content: `@${responder.displayName} ${trimmed}`,
    mentionIds: [responder.id],
  };
}

export function appendGroupIdToLiveUrl(url: string, groupId: string): string {
  const u = new URL(url, "http://local.invalid");
  u.searchParams.set("groupId", groupId);
  return `${u.pathname}${u.search}`;
}
