export type CliAdapterOption = { id: string; displayName: string };

export const CONNECTER_REMOTE_ADAPTER: CliAdapterOption = {
  id: "connecter-remote",
  displayName: "Connecter 远程 Agent",
};

/** Matches App.tsx historical <select> when GET /api/adapters is unavailable. */
export const FALLBACK_CLI_ADAPTERS: CliAdapterOption[] = [
  { id: "mock", displayName: "模拟 Agent（推荐体验）" },
  { id: "codex", displayName: "Codex CLI" },
  { id: "openclaw", displayName: "OpenClaw" },
  { id: "cursor", displayName: "Cursor CLI（agent/cursor-agent）" },
  { id: "claude-code", displayName: "Claude Code" },
  { id: "opencode", displayName: "OpenCode" },
  { id: "dsh", displayName: "DeepSeek Harness（dsh）" },
  CONNECTER_REMOTE_ADAPTER,
];

export function mergeCliAdapters(remote: CliAdapterOption[] | null | undefined): CliAdapterOption[] {
  if (Array.isArray(remote) && remote.length > 0) {
    const valid = remote.filter((row) => row && typeof row.id === "string" && row.id.trim().length > 0);
    return valid.some((row) => row.id === CONNECTER_REMOTE_ADAPTER.id)
      ? valid
      : [...valid, CONNECTER_REMOTE_ADAPTER];
  }
  return FALLBACK_CLI_ADAPTERS;
}

export type AgentAdapterDraft = {
  adapter: string;
  executablePath: string;
  model: string | undefined;
  connecterBaseUrl: string;
  connecterEnv: string;
  connecterGroupRef: string;
  connecterTargetSubjectId: string;
  connecterBearer: string;
};

/** Keep remote provider configuration separate from CLI and chatbot credentials. */
export function buildAgentAdapterPayload(draft: AgentAdapterDraft) {
  if (draft.adapter === CONNECTER_REMOTE_ADAPTER.id) {
    return {
      adapter: draft.adapter,
      connecterBaseUrl: draft.connecterBaseUrl.trim(),
      connecterEnv: draft.connecterEnv.trim(),
      connecterGroupRef: draft.connecterGroupRef.trim(),
      connecterTargetSubjectId: draft.connecterTargetSubjectId.trim(),
      connecterBearer: draft.connecterBearer,
    };
  }
  return {
    adapter: draft.adapter,
    executablePath: draft.executablePath,
    model: draft.model,
  };
}
