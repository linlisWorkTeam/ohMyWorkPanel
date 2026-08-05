import type { Group, Member } from "./types";

/** Project groups: at most one active chatbot. Chat groups: unlimited. */
export function groupHasActiveChatbot(members: Member[]): boolean {
  return members.some((m) => m.isActive && m.kind === "chatbot");
}

export function chatbotSlotTaken(group: Group | null | undefined, members: Member[]): boolean {
  if (!group || group.groupKind === "chat") return false;
  return groupHasActiveChatbot(members);
}

export type UserAddMode = "create" | "link";

/** Whether add-user form can submit for the given mode. */
export function canSubmitUserMember(
  mode: UserAddMode,
  opts: { loginUsername: string; loginPassword: string; existingAuthUserId: string },
): boolean {
  if (mode === "link") return Boolean(opts.existingAuthUserId.trim());
  return Boolean(opts.loginUsername.trim() && opts.loginPassword);
}
