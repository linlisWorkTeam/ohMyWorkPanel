import type { Group, Member } from "../types";

/** Project groups: at most one active chatbot. Chat groups: unlimited. */
export function groupHasActiveChatbot(members: Member[]): boolean {
  return members.some((m) => m.isActive && m.kind === "chatbot");
}

export function chatbotSlotTaken(group: Group | null | undefined, members: Member[]): boolean {
  if (!group || group.groupKind === "chat") return false;
  return groupHasActiveChatbot(members);
}

export type UserAddMode = "create" | "link" | "invite";

/** Whether add-user form can submit for the given mode. */
export function canSubmitUserMember(
  mode: UserAddMode,
  opts: { loginUsername: string; loginPassword: string; existingAuthUserId: string },
): boolean {
  if (mode === "invite") return true;
  if (mode === "link") return Boolean(opts.existingAuthUserId.trim());
  return Boolean(opts.loginUsername.trim() && opts.loginPassword);
}

/** Active → 移除；inactive / pending invite → 删除（永久）。 */
export function memberRosterAction(member: Member): "remove" | "delete" {
  if (member.invitePending || !member.isActive) return "delete";
  return "remove";
}
