export type CliAdapterOption = { id: string; displayName: string };

/** Matches App.tsx historical <select> when GET /api/adapters is unavailable. */
export const FALLBACK_CLI_ADAPTERS: CliAdapterOption[] = [
  { id: "mock", displayName: "模拟 Agent（推荐体验）" },
  { id: "codex", displayName: "Codex CLI" },
  { id: "openclaw", displayName: "OpenClaw" },
  { id: "cursor", displayName: "Cursor CLI（agent/cursor-agent）" },
  { id: "claude-code", displayName: "Claude Code" },
  { id: "opencode", displayName: "OpenCode" },
  { id: "dsh", displayName: "DeepSeek Harness（dsh）" },
];

export function mergeCliAdapters(remote: CliAdapterOption[] | null | undefined): CliAdapterOption[] {
  if (Array.isArray(remote) && remote.length > 0) {
    return remote.filter((row) => row && typeof row.id === "string" && row.id.trim().length > 0);
  }
  return FALLBACK_CLI_ADAPTERS;
}
