import { invoke } from "@tauri-apps/api/core";
import type { GroupState, Member, PresetRole, RuntimeSettings } from "./types";

export const api = {
  bootstrap: () => invoke<{ groups: GroupState["group"][] }>("bootstrap"),
  getGroupState: (groupId: string) => invoke<GroupState>("get_group_state", { groupId }),
  createGroup: (input: { name: string; workspacePath: string; ownerName: string; presetRoles?: string[] }) =>
    invoke<GroupState>("create_group", { input }),
  addMember: (input: {
    groupId: string; kind: "user" | "agent"; displayName: string; roleDescription: string;
    avatarColor?: string; adapter?: string; executablePath?: string;
  }) => invoke<Member>("add_member", { input }),
  removeMember: (groupId: string, memberId: string) => invoke<void>("remove_member", { groupId, memberId }),
  setAdmin: (groupId: string, memberId: string | null) => invoke<GroupState>("set_admin", { groupId, memberId }),
  sendMessage: (groupId: string, senderMemberId: string, content: string, mentionMemberIds: string[]) =>
    invoke("send_message", { groupId, senderMemberId, content, mentionMemberIds }),
  cancelRun: (runId: string) => invoke<void>("cancel_run", { runId }),
  retryRun: (runId: string) => invoke<string>("retry_run", { runId }),
  detectAgent: (memberId: string) => invoke<string>("detect_agent", { memberId }),
  getSettings: () => invoke<RuntimeSettings>("get_runtime_settings"),
   updateSettings: (settings: RuntimeSettings) => invoke<RuntimeSettings>("update_runtime_settings", { settings }),
   ocrImage: (imagePath: string) => invoke<string>("ocr_image", { imagePath }),
   getPresetRoles: () => invoke<PresetRole[]>("get_preset_roles_command"),
  ocrImageBase64: (base64Data: string) => invoke<string>("ocr_image_base64", { base64Data }),
};
