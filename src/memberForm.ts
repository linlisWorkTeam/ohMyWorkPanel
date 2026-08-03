import type { Group, Member } from "./types";

/** Project groups: at most one active chatbot. Chat groups: unlimited. */
export function groupHasActiveChatbot(members: Member[]): boolean {
  return members.some((m) => m.isActive && m.kind === "chatbot");
}

export function chatbotSlotTaken(group: Group | null | undefined, members: Member[]): boolean {
  if (!group || group.groupKind === "chat") return false;
  return groupHasActiveChatbot(members);
}
