/** Keep in sync with src-tauri/src/adapters/models.rs */

const CATALOG: Record<string, string[]> = {
  codex: ["gpt-5", "o3", "o4-mini", "gpt-4.1"],
  // IDs from `cursor-agent --list-models` (account-dependent; keep in sync)
  cursor: [
    "auto",
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

export function modelsForAdapter(adapter: string | null | undefined): string[] {
  if (!adapter) return [];
  return CATALOG[adapter] ?? [];
}

export function defaultModelForAdapter(adapter: string | null | undefined): string {
  return modelsForAdapter(adapter)[0] ?? "";
}
