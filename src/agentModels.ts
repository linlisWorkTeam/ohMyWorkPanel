/** Static fallback catalogs. Cursor live list comes from GET /api/agent-models. */
/** Keep static cursor list roughly in sync with `cursor-agent --list-models` / models.rs */

const STATIC: Record<string, string[]> = {
  codex: ["deepseek-v4-flash", "deepseek-chat", "deepseek-reasoner"],
  cursor: [
    "auto",
    "cursor-grok-4.6-high-fast",
    "cursor-grok-4.6-high",
    "cursor-grok-4.6-xhigh-fast",
    "cursor-grok-4.6-xhigh",
    "cursor-grok-4.6-medium-fast",
    "cursor-grok-4.6-medium",
    "cursor-grok-4.6-low-fast",
    "cursor-grok-4.6-low",
    "cursor-grok-4.5-high",
    "cursor-grok-4.5-high-fast",
    "cursor-grok-4.5-medium",
    "cursor-grok-4.5-medium-fast",
    "cursor-grok-4.5-low",
    "cursor-grok-4.5-low-fast",
    "composer-2.5",
    "composer-2.5-fast",
    "kimi-k3-max",
    "kimi-k3-high",
    "kimi-k3-low",
    "kimi-k2.7-code",
    "glm-5.2-high",
    "glm-5.2-max",
  ],
  "claude-code": ["sonnet", "opus", "haiku"],
  opencode: ["default", "claude-sonnet-4", "gpt-5"],
  openclaw: ["default"],
  "chatbot-deepseek": ["deepseek-v4-flash", "deepseek-chat", "deepseek-reasoner"],
  deepseek: ["deepseek-v4-flash", "deepseek-chat", "deepseek-reasoner"],
  "chatbot-opencode-go": ["deepseek-v4-flash", "deepseek-chat"],
  "opencode-go": ["deepseek-v4-flash", "deepseek-chat"],
  mock: [],
};

/** Live overlays from server (currently Cursor only; other adapters TODO). */
let liveOverlays: Record<string, string[]> = {};

export type AgentModelsPayload = {
  adapters?: Record<string, string[]>;
  cursorSource?: string;
  cursorSyncedAt?: number | null;
  todos?: string[];
};

export function applyAgentModelsPayload(payload: AgentModelsPayload | null | undefined) {
  const next: Record<string, string[]> = {};
  const adapters = payload?.adapters ?? {};
  // Only overlay adapters that returned a non-empty live list (Cursor).
  for (const [k, v] of Object.entries(adapters)) {
    if (Array.isArray(v) && v.length > 0) next[k] = v;
  }
  liveOverlays = next;
}

export function modelsForAdapter(adapter: string | null | undefined): string[] {
  if (!adapter) return [];
  const live = liveOverlays[adapter];
  if (live && live.length > 0) return live;
  return STATIC[adapter] ?? [];
}

export function defaultModelForAdapter(adapter: string | null | undefined): string {
  return modelsForAdapter(adapter)[0] ?? "";
}

/** Test helper */
export function _resetLiveOverlaysForTests() {
  liveOverlays = {};
}
