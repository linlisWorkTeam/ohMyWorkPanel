// Web API layer - replaces api.ts when running in browser (non-Tauri mode).
// Uses fetch() + WebSocket instead of Tauri invoke().
import type {
  GroupState, Member, MessageChannelPart, MessagePage, MetricsSample, PresetRole, RuntimeSettings, Message, TaskRun,
  RoadmapItem, Feature, FeatureTask, RoadmapState, RoadmapOrchestration,
  CreateRoadmapItemInput, UpdateRoadmapItemInput,
  CreateFeatureInput, UpdateFeatureInput,
  CreateFeatureTaskInput, UpdateFeatureTaskInput,
  Experience, SaveExperienceInput, LogEntry, LogLevel, LogQueryFilter,
  DirListing, Group, ReleaseStatus, OpsJobState, ExtensionStatus, A2aDispatchResult,
} from "./types";

const API_BASE = "";
const WS_BASE = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}`;
const TOKEN_KEY = "linlis_auth_token";

export const requiresAuth = true;

let authToken: string | null = (() => {
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
})();

export function setAuthToken(token: string | null) {
  authToken = token;
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token);
    else localStorage.removeItem(TOKEN_KEY);
  } catch {
    /* ignore quota / private mode */
  }
  // Keep WS stub in sync (web build aliases @tauri-apps/api/event → stubs)
  void import("./stubs/tauri-event")
    .then((m) => m._setWebAuthToken?.(token))
    .catch(() => {});
}
export function getAuthToken(): string | null {
  return authToken;
}

type UnauthorizedListener = () => void;
const unauthorizedListeners = new Set<UnauthorizedListener>();

/** Subscribe to global 401 / missing-token events (web only). */
export function onUnauthorized(listener: UnauthorizedListener): () => void {
  unauthorizedListeners.add(listener);
  return () => {
    unauthorizedListeners.delete(listener);
  };
}

function emitUnauthorized() {
  setAuthToken(null);
  unauthorizedListeners.forEach((listener) => listener());
}

function isAuthPath(path: string): boolean {
  return path.startsWith("/api/auth/login") || path.startsWith("/api/auth/register");
}

const STATUS_HINT: Record<number, string> = {
  400: "请求无效",
  401: "登录已失效，请重新登录",
  403: "没有权限执行此操作",
  404: "请求的资源不存在",
  409: "数据冲突",
  422: "数据格式不正确",
  500: "服务器出错了",
  502: "服务暂时不可用",
  503: "服务暂时不可用",
};

function friendlyError(status: number, body: string): string {
  const detail = body.trim().slice(0, 120).replace(/\s+/g, " ");
  const hint = STATUS_HINT[status] ?? "请求失败";
  return detail ? `${hint}（${detail}）` : hint;
}

async function apiFetch<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string>),
  };
  if (authToken) headers["Authorization"] = `Bearer ${authToken}`;
  const res = await fetch(`${API_BASE}${path}`, { ...options, headers });
  if (!res.ok) {
    const text = await res.text();
    if (res.status === 401 && !isAuthPath(path)) {
      emitUnauthorized();
    }
    throw new Error(friendlyError(res.status, text));
  }
  return res.json();
}

export const api = {
  // Auth
  register: (username: string, password: string) =>
    apiFetch<{ token: string; user_id: string; username: string; isAdmin?: boolean; is_admin?: boolean }>(
      "/api/auth/register",
      {
        method: "POST",
        body: JSON.stringify({ username, password }),
      },
    ),
  login: (username: string, password: string) =>
    apiFetch<{ token: string; user_id: string; username: string; isAdmin?: boolean; is_admin?: boolean }>(
      "/api/auth/login",
      {
        method: "POST",
        body: JSON.stringify({ username, password }),
      },
    ),
  verify: () =>
    apiFetch<{ sub: string; username: string; isAdmin?: boolean; is_admin?: boolean }>("/api/auth/verify"),

  // Bootstrap - shape matches Tauri invoke({ groups })
  bootstrap: async () => {
    const groups = await apiFetch<GroupState["group"][]>("/api/groups");
    return { groups };
  },

  // Groups
  getGroupState: (groupId: string) =>
    apiFetch<GroupState>(`/api/groups/${groupId}`),

  listMessagesBefore: (groupId: string, beforeCreatedAt: number, beforeId: string, limit = 50) => {
    const q = new URLSearchParams({
      beforeCreatedAt: String(beforeCreatedAt),
      beforeId,
      limit: String(limit),
    });
    return apiFetch<MessagePage>(`/api/groups/${groupId}/messages?${q}`);
  },

  getMessageChannelPart: (groupId: string, messageId: string, channel: "thinking" | "artifact") =>
    apiFetch<MessageChannelPart>(
      `/api/groups/${groupId}/messages/${encodeURIComponent(messageId)}/parts/${encodeURIComponent(channel)}`,
    ),

  createGroup: (input: {
    name: string;
    workspacePath: string;
    ownerName: string;
    presetRoles?: string[];
    groupKind?: "project" | "chat";
  }) =>
    apiFetch<GroupState>("/api/groups", {
      method: "POST",
      body: JSON.stringify(input),
    }),

  addMember: (input: {
    groupId: string;
    kind: "user" | "agent" | "chatbot";
    displayName: string;
    roleDescription: string;
    avatarColor?: string;
    adapter?: string;
    executablePath?: string;
    chatbotProvider?: "opencode-go" | "deepseek";
    apiKey?: string;
    model?: string;
    loginUsername?: string;
    loginPassword?: string;
    existingAuthUserId?: string;
  }) =>
    apiFetch<Member>(`/api/groups/${input.groupId}/members`, {
      method: "POST",
      body: JSON.stringify(input),
    }),

  listJoinableUsers: (groupId: string) =>
    apiFetch<{ id: string; username: string }[]>(
      `/api/users/joinable?groupId=${encodeURIComponent(groupId)}`,
    ),

  setGroupArchived: (groupId: string, archived: boolean) =>
    apiFetch<Group>(`/api/groups/${groupId}/archive`, {
      method: "PUT",
      body: JSON.stringify({ archived }),
    }),

  updateMemberModel: (memberId: string, model: string | null) =>
    apiFetch<Member>(`/api/members/${memberId}/model`, {
      method: "PUT",
      body: JSON.stringify({ model }),
    }),

  removeMember: (groupId: string, memberId: string) =>
    apiFetch<void>(`/api/groups/${groupId}/members/${memberId}`, {
      method: "DELETE",
    }),

  setAdmin: (groupId: string, memberId: string | null) =>
    apiFetch<GroupState>(`/api/groups/${groupId}/admin`, {
      method: "PUT",
      body: JSON.stringify({ memberId }),
    }),

  sendMessage: (
    groupId: string,
    senderMemberId: string,
    content: string,
    mentionMemberIds: string[],
  ) =>
    apiFetch<{ message: Message; runIds: string[] }>("/api/messages", {
      method: "POST",
      body: JSON.stringify({ groupId, senderMemberId, content, mentionMemberIds }),
    }),

  cancelRun: (runId: string) =>
    apiFetch<void>(`/api/runs/${runId}/cancel`, { method: "POST" }),

  retryRun: (runId: string) =>
    apiFetch<string>(`/api/runs/${runId}/retry`, { method: "POST" }),

  getSettings: () => apiFetch<RuntimeSettings>("/api/settings"),
  getMetricsLatest: () => apiFetch<MetricsSample>("/api/metrics/latest"),
  listActiveRuns: (groupId: string) =>
    apiFetch<TaskRun[]>(`/api/groups/${groupId}/runs/active`),

  updateSettings: (settings: RuntimeSettings) =>
    apiFetch<RuntimeSettings>("/api/settings", {
      method: "PUT",
      body: JSON.stringify(settings),
    }),

  ocrImage: (imagePath: string) =>
    apiFetch<string>("/api/ocr", {
      method: "POST",
      body: JSON.stringify({ imagePath }),
    }),

  ocrImageBase64: (base64Data: string) =>
    apiFetch<string>("/api/ocr/base64", {
      method: "POST",
      body: JSON.stringify({ base64Data }),
    }),

  getPresetRoles: () => apiFetch<PresetRole[]>("/api/preset-roles"),

  // PM: Roadmap Items
  listRoadmapItems: (groupId: string) =>
    apiFetch<RoadmapItem[]>(`/api/groups/${groupId}/roadmap`),
  createRoadmapItem: (input: CreateRoadmapItemInput) =>
    apiFetch<RoadmapItem>("/api/roadmap-items", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateRoadmapItem: (id: string, input: UpdateRoadmapItemInput) =>
    apiFetch<RoadmapItem>(`/api/roadmap-items/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  deleteRoadmapItem: (id: string) =>
    apiFetch<void>(`/api/roadmap-items/${id}`, { method: "DELETE" }),

  // PM: Features
  listFeatures: (groupId: string) =>
    apiFetch<Feature[]>(`/api/groups/${groupId}/features`),
  createFeature: (input: CreateFeatureInput) =>
    apiFetch<Feature>("/api/features", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateFeature: (id: string, input: UpdateFeatureInput) =>
    apiFetch<Feature>(`/api/features/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  deleteFeature: (id: string) =>
    apiFetch<void>(`/api/features/${id}`, { method: "DELETE" }),

  // PM: Feature Tasks
  listFeatureTasks: (featureId: string) =>
    apiFetch<FeatureTask[]>(`/api/features/${featureId}/tasks`),
  createFeatureTask: (input: CreateFeatureTaskInput) =>
    apiFetch<FeatureTask>("/api/feature-tasks", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateFeatureTask: (id: string, input: UpdateFeatureTaskInput) =>
    apiFetch<FeatureTask>(`/api/feature-tasks/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  deleteFeatureTask: (id: string) =>
    apiFetch<void>(`/api/feature-tasks/${id}`, { method: "DELETE" }),

  // PM: Aggregated State
  getRoadmapState: (groupId: string) =>
    apiFetch<RoadmapState>(`/api/groups/${groupId}/roadmap-state`),

  listRoadmapOrchestrations: (groupId: string) =>
    apiFetch<RoadmapOrchestration[]>(`/api/groups/${groupId}/roadmap-orchestrations`),
  startRoadmapItem: (id: string) =>
    apiFetch<RoadmapOrchestration>(`/api/roadmap-items/${id}/start`, { method: "POST" }),
  pauseRoadmapOrchestration: (id: string) =>
    apiFetch<RoadmapOrchestration>(`/api/roadmap-orchestrations/${id}/pause`, { method: "POST" }),
  resumeRoadmapOrchestration: (id: string) =>
    apiFetch<RoadmapOrchestration>(`/api/roadmap-orchestrations/${id}/resume`, { method: "POST" }),
  cancelRoadmapOrchestration: (id: string) =>
    apiFetch<RoadmapOrchestration>(`/api/roadmap-orchestrations/${id}/cancel`, { method: "POST" }),

  // Shared Memory: Experiences (sourceMemberId 可指定群内成员；缺省由服务端回落到登录用户)
  saveExperience: (input: SaveExperienceInput) =>
    apiFetch<string>("/api/experiences", {
      method: "POST",
      body: JSON.stringify({
        groupId: input.groupId, title: input.title,
        content: input.content, tags: input.tags ?? "",
        sourceMemberId: input.sourceMemberId ?? undefined,
      }),
    }),
  queryExperiences: (groupId: string, query?: string, limit?: number) =>
    apiFetch<Experience[]>(
      `/api/experiences?groupId=${encodeURIComponent(groupId)}&query=${encodeURIComponent(query ?? "")}&limit=${limit ?? 20}`,
    ),
  deleteExperience: (id: string) =>
    apiFetch<boolean>(`/api/experiences/${id}`, { method: "DELETE" }),

  // Logs
  listLogs: (filter: LogQueryFilter = {}) => {
    const params = new URLSearchParams();
    if (filter.limit != null) params.set("limit", String(filter.limit));
    if (filter.offset != null) params.set("offset", String(filter.offset));
    if (filter.level) params.set("level", filter.level);
    if (filter.source) params.set("source", filter.source);
    if (filter.since != null) params.set("since", String(filter.since));
    const qs = params.toString();
    return apiFetch<LogEntry[]>(`/api/logs${qs ? `?${qs}` : ""}`);
  },
  countLogs: (level?: LogLevel, source?: string) => {
    const params = new URLSearchParams();
    if (level) params.set("level", level);
    if (source) params.set("source", source);
    const qs = params.toString();
    return apiFetch<{ count: number }>(`/api/logs/count${qs ? `?${qs}` : ""}`);
  },
  clearLogs: () => apiFetch<void>("/api/logs", { method: "DELETE" }),

  listServerDir: (path: string) =>
    apiFetch<DirListing>(`/api/fs/list?path=${encodeURIComponent(path || "/")}`),
  createServerDir: (parent: string, name: string) =>
    apiFetch<{ path: string }>("/api/fs/mkdir", {
      method: "POST",
      body: JSON.stringify({ parent, name }),
    }),

  listGroupExtensions: (groupId: string) =>
    apiFetch<ExtensionStatus[]>(`/api/groups/${groupId}/extensions`),
  setPanelliveEnabled: (groupId: string, enabled: boolean) =>
    apiFetch<ExtensionStatus>(`/api/groups/${groupId}/extensions/panellive`, {
      method: "PUT",
      body: JSON.stringify({ enabled }),
    }),
  dispatchA2a: (envelope: {
    skill: string;
    groupId: string;
    sessionId?: string;
    payload?: unknown;
    source?: string;
    target?: string;
  }) =>
    apiFetch<A2aDispatchResult>("/api/a2a/dispatch", {
      method: "POST",
      body: JSON.stringify(envelope),
    }),
  updateGroupWorkspace: (groupId: string, workspacePath: string) =>
    apiFetch<Group>(`/api/groups/${groupId}/workspace`, {
      method: "PUT",
      body: JSON.stringify({ workspacePath }),
    }),
  updateMemberWorkspace: (memberId: string, workspacePath: string) =>
    apiFetch<Member>(`/api/members/${memberId}/workspace`, {
      method: "PUT",
      body: JSON.stringify({ workspacePath }),
    }),
  getGroupAnnouncement: (groupId: string) =>
    apiFetch<Group>(`/api/groups/${groupId}/announcement`).then((r) => ({
      ...(r as unknown as Group),
      id: groupId,
      announcement: (r as { announcement?: string }).announcement ?? "",
    })),
  setGroupAnnouncement: (groupId: string, announcement: string) =>
    apiFetch<Group>(`/api/groups/${groupId}/announcement`, {
      method: "PUT",
      body: JSON.stringify({ announcement }),
    }),
  opsReleaseStatus: () => apiFetch<ReleaseStatus>("/api/ops/release-status"),
  opsJob: () => apiFetch<OpsJobState>("/api/ops/job"),
  opsRunTestGate: () => apiFetch<void>("/api/ops/test-gate", { method: "POST" }),
  opsDeployCanary: () => apiFetch<void>("/api/ops/deploy-canary", { method: "POST" }),

  // WebSocket - replaces Tauri listen()
  connectWS: (onMessage: (data: string) => void) => {
    const ws = new WebSocket(`${WS_BASE}/ws?token=${authToken ?? ""}`);
    ws.onmessage = (e) => onMessage(e.data);
    ws.onerror = () => console.error("WebSocket error");
    return ws;
  },
};
