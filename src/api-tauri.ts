import { invoke } from "@tauri-apps/api/core";
import type {
  GroupState, Member, MessagePage, PresetRole, RuntimeSettings,
  RoadmapItem, Feature, FeatureTask, RoadmapState,
  CreateRoadmapItemInput, UpdateRoadmapItemInput,
  CreateFeatureInput, UpdateFeatureInput,
  CreateFeatureTaskInput, UpdateFeatureTaskInput,
  Experience, SaveExperienceInput, LogEntry, LogLevel, LogQueryFilter,
  DirListing, Group, ReleaseStatus, OpsJobState,
} from "./types";

export const api = {
  bootstrap: () => invoke<{ groups: GroupState["group"][] }>("bootstrap"),
  getGroupState: (groupId: string) => invoke<GroupState>("get_group_state", { groupId }),
  listMessagesBefore: (groupId: string, beforeCreatedAt: number, beforeId: string, limit?: number) =>
    invoke<MessagePage>("list_messages_before", { groupId, beforeCreatedAt, beforeId, limit }),
  createGroup: (input: {
    name: string; workspacePath: string; ownerName: string; presetRoles?: string[];
    groupKind?: "project" | "chat";
  }) =>
    invoke<GroupState>("create_group", { input }),
  addMember: (input: {
    groupId: string; kind: "user" | "agent" | "chatbot"; displayName: string; roleDescription: string;
    avatarColor?: string; adapter?: string; executablePath?: string;
    chatbotProvider?: "opencode-go" | "deepseek"; apiKey?: string; model?: string;
  }) => invoke<Member>("add_member", { input }),
  setGroupArchived: (groupId: string, archived: boolean) =>
    invoke<Group>("set_group_archived_cmd", { groupId, archived }),
  updateMemberModel: (memberId: string, model: string | null) =>
    invoke<Member>("update_member_model_cmd", { memberId, model }),
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

  listRoadmapOrchestrations: async () => [] as import("./types").RoadmapOrchestration[],
  startRoadmapItem: async (_id: string) => { throw new Error("路线图编排请在 Web 灰度环境使用"); },
  pauseRoadmapOrchestration: async (_id: string) => { throw new Error("路线图编排请在 Web 灰度环境使用"); },
  resumeRoadmapOrchestration: async (_id: string) => { throw new Error("路线图编排请在 Web 灰度环境使用"); },
  cancelRoadmapOrchestration: async (_id: string) => { throw new Error("路线图编排请在 Web 灰度环境使用"); },

  // Shared Memory: Experiences
  saveExperience: (input: SaveExperienceInput) =>
    invoke<string>("save_experience", {
      groupId: input.groupId, sourceMemberId: input.sourceMemberId,
      title: input.title, content: input.content, tags: input.tags ?? null,
    }),
  queryExperiences: (groupId: string, query?: string, limit?: number) =>
    invoke<Experience[]>("query_experiences", { groupId, query: query ?? null, limit: limit ?? null }),
  deleteExperience: (id: string) => invoke<boolean>("delete_experience", { id }),

  // Logs
  listLogs: (filter: LogQueryFilter = {}) =>
    invoke<LogEntry[]>("list_logs", {
      limit: filter.limit ?? null, offset: filter.offset ?? null,
      level: filter.level ?? null, source: filter.source ?? null, since: filter.since ?? null,
    }),
  countLogs: (level?: LogLevel) =>
    invoke<number>("count_logs", { level: level ?? null }).then((count) => ({ count })),
  clearLogs: () => invoke<void>("clear_logs"),

  listServerDir: (path: string) => invoke<DirListing>("list_server_dir", { path }),
  updateGroupWorkspace: (groupId: string, workspacePath: string) =>
    invoke<Group>("update_group_workspace_cmd", { groupId, workspacePath }),
  updateMemberWorkspace: (memberId: string, workspacePath: string) =>
    invoke<Member>("update_member_workspace_cmd", { memberId, workspacePath }),
  getGroupAnnouncement: (groupId: string) =>
    invoke<Group>("get_group_announcement", { groupId }),
  setGroupAnnouncement: (groupId: string, announcement: string) =>
    invoke<Group>("set_group_announcement_cmd", { groupId, announcement }),
  opsReleaseStatus: () => invoke<ReleaseStatus>("ops_release_status"),
  opsJob: () => invoke<OpsJobState>("ops_job_status"),
  opsRunTestGate: () => invoke<void>("ops_run_test_gate"),
  opsDeployCanary: () => invoke<void>("ops_deploy_canary"),
};
