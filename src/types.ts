export type MemberKind = "user" | "agent";
export type RunStatus = "queued" | "running" | "completed" | "failed" | "cancelled" | "interrupted";

export interface Group {
  id: string;
  name: string;
  workspacePath: string;
  ownerMemberId: string;
  adminMemberId: string | null;
  createdAt: number;
}

export interface Member {
  id: string;
  groupId: string;
  kind: MemberKind;
  displayName: string;
  avatarColor: string;
  roleDescription: string;
  isActive: boolean;
  adapter: "mock" | "codex" | "claude-code" | "opencode" | "cursor" | null;
  executablePath: string | null;
  runtimeStatus: "unknown" | "ready" | "unavailable" | null;
  createdAt: number;
}

export interface Message {
  id: string;
  groupId: string;
  senderMemberId: string;
  parentRunId: string | null;
  content: string;
  status: string;
  createdAt: number;
}

export interface TaskRun {
  id: string;
  groupId: string;
  rootMessageId: string;
  agentMemberId: string;
  parentRunId: string | null;
  depth: number;
  status: RunStatus;
  outputMessageId: string | null;
  errorMessage: string | null;
  createdAt: number;
  startedAt: number | null;
  completedAt: number | null;
}

export interface GroupState {
  group: Group;
  members: Member[];
  messages: Message[];
  runs: TaskRun[];
}

export interface ChatEvent {
  kind: "message_created" | "message_delta" | "run_status" | "member_removed" | "scheduler_error";
  groupId: string;
  runId: string | null;
  messageId: string | null;
  delta: string | null;
  status: RunStatus | string | null;
  error: string | null;
}

export interface RuntimeSettings {
  maxConcurrentRuns: number;
  runTimeoutSeconds: number;
  contextMessageLimit: number;
  maxDelegationDepth: number;
}
