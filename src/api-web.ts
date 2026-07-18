// Web API layer — replaces api.ts when running in browser (non-Tauri mode).
// Uses fetch() + WebSocket instead of Tauri invoke().
import type { GroupState, Member, PresetRole, RuntimeSettings } from "./types";

const API_BASE = "";
const WS_BASE = `ws://${location.host}`;

let authToken: string | null = null;

export function setAuthToken(token: string | null) {
  authToken = token;
}
export function getAuthToken(): string | null {
  return authToken;
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
    throw new Error(`${res.status}: ${text.slice(0, 200)}`);
  }
  return res.json();
}

export const api = {
  // Auth
  register: (username: string, password: string) =>
    apiFetch<{ token: string; user_id: string; username: string }>(
      "/api/auth/register",
      {
        method: "POST",
        body: JSON.stringify({ username, password }),
      },
    ),
  login: (username: string, password: string) =>
    apiFetch<{ token: string; user_id: string; username: string }>(
      "/api/auth/login",
      {
        method: "POST",
        body: JSON.stringify({ username, password }),
      },
    ),

  // Bootstrap — returns groups the user belongs to
  bootstrap: () =>
    apiFetch<{ groups: GroupState["group"][] }>("/api/bootstrap"),

  // Groups
  getGroupState: (groupId: string) =>
    apiFetch<GroupState>(`/api/groups/${groupId}`),

  createGroup: (input: {
    name: string;
    workspacePath: string;
    ownerName: string;
    presetRoles?: string[];
  }) =>
    apiFetch<GroupState>("/api/groups", {
      method: "POST",
      body: JSON.stringify(input),
    }),

  addMember: (input: {
    groupId: string;
    kind: "user" | "agent";
    displayName: string;
    roleDescription: string;
    avatarColor?: string;
    adapter?: string;
    executablePath?: string;
  }) =>
    apiFetch<Member>("/api/members", {
      method: "POST",
      body: JSON.stringify(input),
    }),

  removeMember: (groupId: string, memberId: string) =>
    apiFetch<void>(`/api/groups/${groupId}/members/${memberId}`, {
      method: "DELETE",
    }),

  setAdmin: (groupId: string, memberId: string | null) =>
    apiFetch<GroupState>(
      `/api/groups/${groupId}/admin/${memberId ?? ""}`,
      { method: "PUT" },
    ),

  sendMessage: (
    groupId: string,
    senderMemberId: string,
    content: string,
    mentionMemberIds: string[],
  ) =>
    apiFetch("/api/messages", {
      method: "POST",
      body: JSON.stringify({ groupId, senderMemberId, content, mentionMemberIds }),
    }),

  cancelRun: (runId: string) =>
    apiFetch<void>(`/api/runs/${runId}/cancel`, { method: "POST" }),

  retryRun: (runId: string) =>
    apiFetch<string>(`/api/runs/${runId}/retry`, { method: "POST" }),

  detectAgent: (memberId: string) =>
    apiFetch<string>(`/api/agents/${memberId}/detect`),

  getSettings: () => apiFetch<RuntimeSettings>("/api/settings"),

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

  // WebSocket — replaces Tauri listen()
  connectWS: (onMessage: (data: string) => void) => {
    const ws = new WebSocket(`${WS_BASE}/ws?token=${authToken ?? ""}`);
    ws.onmessage = (e) => onMessage(e.data);
    ws.onerror = () => console.error("WebSocket error");
    return ws;
  },
};
