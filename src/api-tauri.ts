import { invoke } from "@tauri-apps/api/core";
import type {
  GroupState, Member, PresetRole, RuntimeSettings,
  RoadmapItem, Feature, FeatureTask, RoadmapState,
  CreateRoadmapItemInput, UpdateRoadmapItemInput,
  CreateFeatureInput, UpdateFeatureInput,
  CreateFeatureTaskInput, UpdateFeatureTaskInput,
} from "./types";

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

  // PM: Roadmap Items
  listRoadmapItems: (groupId: string) => invoke<RoadmapItem[]>("list_roadmap_items", { groupId }),
  createRoadmapItem: (input: CreateRoadmapItemInput) => invoke<RoadmapItem>("create_roadmap_item", { input }),
  updateRoadmapItem: (id: string, input: UpdateRoadmapItemInput) => invoke<RoadmapItem>("update_roadmap_item", { id, input }),
  deleteRoadmapItem: (id: string) => invoke<void>("delete_roadmap_item", { id }),

  // PM: Features
  listFeatures: (groupId: string) => invoke<Feature[]>("list_features", { groupId }),
  createFeature: (input: CreateFeatureInput) => invoke<Feature>("create_feature", { input }),
  updateFeature: (id: string, input: UpdateFeatureInput) => invoke<Feature>("update_feature", { id, input }),
  deleteFeature: (id: string) => invoke<void>("delete_feature", { id }),

  // PM: Feature Tasks
  listFeatureTasks: (featureId: string) => invoke<FeatureTask[]>("list_feature_tasks", { featureId }),
  createFeatureTask: (input: CreateFeatureTaskInput) => invoke<FeatureTask>("create_feature_task", { input }),
  updateFeatureTask: (id: string, input: UpdateFeatureTaskInput) => invoke<FeatureTask>("update_feature_task", { id, input }),
  deleteFeatureTask: (id: string) => invoke<void>("delete_feature_task", { id }),

  // PM: Aggregated State
  getRoadmapState: (groupId: string) => invoke<RoadmapState>("get_roadmap_state", { groupId }),
};
