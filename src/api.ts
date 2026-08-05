import { invoke } from "@tauri-apps/api/core";
import type {
  AddMemberResult, GroupState, Member, MessageChannelPart, MessagePage, PresetRole, RuntimeSettings,
  RoadmapItem, Feature, FeatureTask, RoadmapState,
  CreateRoadmapItemInput, UpdateRoadmapItemInput,
  CreateFeatureInput, UpdateFeatureInput,
  CreateFeatureTaskInput, UpdateFeatureTaskInput,
  Experience, SaveExperienceInput, LogEntry, LogLevel, LogQueryFilter,
  DirListing, Group, ReleaseStatus, OpsJobState, InvitePreview,
} from "./types";

/** Desktop/Tauri builds do not require JWT login. */
export const requiresAuth = false;
export function setAuthToken(_token: string | null) {}
export function getAuthToken(): string | null {
  return null;
}
export function onUnauthorized(_listener: () => void): () => void {
  return () => {};
}

export const api = {
  login: (_username: string, _password: string) =>
    Promise.reject(new Error("Desktop mode does not use web login")) as Promise<{ token: string; user_id: string; username: string }>,
  register: (_username: string, _password: string) =>
    Promise.reject(new Error("Desktop mode does not use web register")) as Promise<{ token: string; user_id: string; username: string }>,
  bootstrap: () => invoke<{ groups: GroupState["group"][] }>("bootstrap"),
  getGroupState: (groupId: string) => invoke<GroupState>("get_group_state", { groupId }),
  markGroupRead: (_groupId: string) => Promise.resolve({ ok: true as const }),
  listPresence: () => Promise.resolve({ onlineUserIds: [] as string[] }),
  listMessagesBefore: (groupId: string, beforeCreatedAt: number, beforeId: string, limit?: number) =>
    invoke<MessagePage>("list_messages_before", { groupId, beforeCreatedAt, beforeId, limit }),
  getMessageChannelPart: (groupId: string, messageId: string, channel: "thinking" | "artifact") =>
    invoke<MessageChannelPart>("get_message_channel_part", { groupId, messageId, channel }),
  createGroup: (input: {
    name: string; workspacePath: string; ownerName: string; presetRoles?: string[];
    groupKind?: "project" | "chat";
  }) =>
    invoke<GroupState>("create_group", { input }),
  addMember: (input: {
    groupId: string; kind: "user" | "agent" | "chatbot"; displayName: string; roleDescription: string;
    avatarColor?: string; adapter?: string; executablePath?: string;
    chatbotProvider?: "opencode-go" | "deepseek"; apiKey?: string; model?: string;
    loginUsername?: string; loginPassword?: string; existingAuthUserId?: string;
    invite?: boolean;
  }) => invoke<AddMemberResult>("add_member", { input }),
  listJoinableUsers: (groupId: string) =>
    invoke<{ id: string; username: string }[]>("list_joinable_users", { groupId }),
  getInvitePreview: (_token: string) =>
    Promise.reject(new Error("Invite links are web-only")) as Promise<InvitePreview>,
  acceptInvite: (_token: string) =>
    Promise.reject(new Error("Invite links are web-only")) as Promise<Member>,
  verify: async () => ({ sub: "desktop", username: "desktop", isAdmin: true }),
  setGroupArchived: (groupId: string, archived: boolean) =>
    invoke<Group>("set_group_archived_cmd", { groupId, archived }),
  updateMemberModel: (memberId: string, model: string | null) =>
    invoke<Member>("update_member_model_cmd", { memberId, model }),
  removeMember: (groupId: string, memberId: string) => invoke<void>("remove_member", { groupId, memberId }),
  purgeMember: (groupId: string, memberId: string) => invoke<void>("remove_member", { groupId, memberId }),
  setAdmin: (groupId: string, memberId: string | null) => invoke<GroupState>("set_admin", { groupId, memberId }),
  sendMessage: (groupId: string, senderMemberId: string, content: string, mentionMemberIds: string[]) =>
    invoke("send_message", { groupId, senderMemberId, content, mentionMemberIds }),
  cancelRun: (runId: string) => invoke<void>("cancel_run", { runId }),
  retryRun: (runId: string) => invoke<string>("retry_run", { runId }),
  detectAgent: (memberId: string) => invoke<string>("detect_agent", { memberId }),
  getSettings: () => invoke<RuntimeSettings>("get_runtime_settings"),
  getMetricsLatest: () => Promise.reject(new Error("metrics API is web-only")) as Promise<import("./types").MetricsSample>,
  listActiveRuns: (_groupId: string) => Promise.resolve([] as import("./types").TaskRun[]),
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
  countLogs: (level?: LogLevel, source?: string) =>
    invoke<number>("count_logs", { level: level ?? null, source: source ?? null }).then((count) => ({ count })),
  clearLogs: () => invoke<void>("clear_logs"),

  listServerDir: (path: string) => invoke<DirListing>("list_server_dir", { path }),
  createServerDir: (parent: string, name: string) =>
    invoke<string>("create_server_dir", { parent, name }).then((path) => ({ path })),
  listGroupExtensions: async (_groupId: string) => [] as import("./types").ExtensionStatus[],
  setPanelliveEnabled: async (_groupId: string, _enabled: boolean) => {
    throw new Error("Desktop mode: PanelLive Extension Host 仅 Web 服务可用");
  },
  dispatchA2a: async () => {
    throw new Error("Desktop mode: A2A dispatch 仅 Web 服务可用");
  },
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
