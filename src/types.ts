export type MemberKind = "user" | "agent" | "chatbot";
export type RunStatus = "queued" | "running" | "awaiting_review" | "changes_requested" | "completed" | "failed" | "cancelled" | "interrupted";
export type RunPhase =
  | "queued"
  | "starting"
  | "preparing"
  | "cli_spawn"
  | "awaiting_first_token"
  | "streaming"
  | "finalizing"
  | "completed"
  | "failed"
  | string;

export interface Group {
  id: string;
  name: string;
  workspacePath: string;
  ownerMemberId: string;
  adminMemberId: string | null;
  createdAt: number;
  announcement?: string;
  announcementUpdatedAt?: number | null;
  /** project (default) | chat */
  groupKind?: "project" | "chat" | string;
  archived?: boolean;
  /** Built-in seed group — not deletable; may allow absolute agent workspace. */
  isSystem?: boolean;
}

export interface DirEntryInfo {
  name: string;
  path: string;
  isDir: boolean;
}

export interface DirListing {
  path: string;
  parent: string | null;
  entries: DirEntryInfo[];
}

export interface ReleaseSlotStatus {
  slot: string;
  port: number;
  httpStatus: number | null;
  release: Record<string, unknown> | null;
  dataDir: string;
}

export interface ReleaseStatus {
  prod: ReleaseSlotStatus;
  canary: ReleaseSlotStatus;
  note: string;
}

export interface OpsJobState {
  running: boolean;
  kind: string;
  exitCode: number | null;
  log: string;
  startedAt: number | null;
  finishedAt: number | null;
}

export interface Member {
  id: string;
  groupId: string;
  kind: MemberKind;
  displayName: string;
  avatarColor: string;
  roleDescription: string;
  isActive: boolean;
  adapter: "mock" | "codex" | "claude-code" | "opencode" | "openclaw" | "cursor" | "chatbot-opencode-go" | "chatbot-deepseek" | string | null;
  executablePath: string | null;
  runtimeStatus: "unknown" | "ready" | "unavailable" | null;
  tags: string;
  createdAt: number;
  workspacePath?: string | null;
  apiKeySet?: boolean;
  keepAlive?: boolean;
  warmStatus?: string | null;
  /** Preferred model; empty/null = provider default */
  model?: string | null;
  /** Linked login users.id for kind=user */
  authUserId?: string | null;
}

export interface Message {
  id: string;
  groupId: string;
  senderMemberId: string;
  parentRunId: string | null;
  content: string;
  status: string;
  createdAt: number;
  /** Full thinking text is lazy-loaded on expand */
  hasThinking?: boolean;
  /** Full artifact text is lazy-loaded on expand */
  hasArtifact?: boolean;
}

export interface MessageChannelPart {
  messageId: string;
  channel: string;
  text: string;
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
  reviewStatus: "pending" | "approved" | "rejected" | null;
  reviewerMemberId: string | null;
  createdAt: number;
  startedAt: number | null;
  completedAt: number | null;
  phase?: RunPhase | null;
  phaseUpdatedAt?: number | null;
}

export interface GroupState {
  group: Group;
  members: Member[];
  messages: Message[];
  runs: TaskRun[];
  /** Older than hot window (default 100) still in DB */
  messagesHasMore?: boolean;
  messagesTotal?: number;
}

export interface MessagePage {
  messages: Message[];
  hasMore: boolean;
}

export interface ChatEvent {
  kind: "message_created" | "message_delta" | "run_status" | "member_removed" | "scheduler_error" | "run_heartbeat" | "ws_reconnected" | string;
  groupId: string;
  runId: string | null;
  messageId: string | null;
  delta: string | null;
  status: RunStatus | string | null;
  error: string | null;
  /** thinking | artifact | final */
  channel?: string | null;
  /** When true, delta replaces channel text instead of appending */
  replace?: boolean | null;
  phase?: RunPhase | null;
  elapsedMs?: number | null;
  totalMs?: number | null;
  seq?: number | null;
  deltaCount?: number | null;
  rssMib?: number | null;
}

export interface PresetRole {
  name: string;
  adapter: string;
  roleDescription: string;
  avatarColor: string;
}

export interface RuntimeSettings {
  maxConcurrentRuns: number;
  runTimeoutSeconds: number;
  contextMessageLimit: number;
  /** Chat group / chatbot native window (default 12). */
  chatContextMessageLimit?: number;
  maxDelegationDepth: number;
  heartbeatAuto?: boolean;
  heartbeatFocusSeconds?: number;
  heartbeatBackgroundSeconds?: number;
}

export interface MetricsSample {
  rssMib: number;
  cpuPct: number;
  ts: number;
}

export interface ExtensionTab {
  id: string;
  title: string;
  route: string;
  entry: string;
  peerOf?: string[];
  disabledWhenUnloaded?: boolean;
}

export interface ExtensionStatus {
  id: string;
  name: string;
  version: string;
  kind: string;
  enabled: boolean;
  healthy: boolean;
  healthDetail: string;
  baseUrl: string;
  tabs: ExtensionTab[];
  a2aSkills: string[];
  mediaPlane: string;
}

export interface A2aDispatchResult {
  accepted: boolean;
  skill: string;
  sessionId?: string | null;
  message: string;
}
 
 // === Project Management ===
 
 export interface RoadmapItem {
   id: string;
   groupId: string;
   title: string;
   description: string;
   status: string;
   priority: string;
   targetDate: string | null;
   sortOrder: number;
   createdAt: number;
 }
 
 export interface Feature {
   id: string;
   groupId: string;
   title: string;
   description: string;
   status: string;
   priority: string;
   area: string;
   assigneeMemberId: string | null;
   targetRoadmapItemId: string | null;
   sortOrder: number;
   createdAt: number;
   updatedAt: number;
 }
 
 export interface FeatureTask {
   id: string;
   featureId: string;
   title: string;
   done: boolean;
   sortOrder: number;
   createdAt: number;
 }
 
 export interface RoadmapState {
   groupId: string;
   items: RoadmapItem[];
   features: Feature[];
   tasks: FeatureTask[];
 }

export interface RoadmapOrchestration {
  id: string;
  groupId: string;
  roadmapItemId: string;
  status: string;
  cursorFeatureId: string | null;
  cursorTaskId: string | null;
  currentRunId: string | null;
  errorMessage: string | null;
  createdAt: number;
  updatedAt: number;
}
 
 export interface CreateRoadmapItemInput {
   groupId: string;
   title: string;
   description?: string;
   status?: string;
   priority?: string;
   targetDate?: string;
 }
 
 export interface UpdateRoadmapItemInput {
   title?: string;
   description?: string;
   status?: string;
   priority?: string;
   targetDate?: string;
   sortOrder?: number;
 }
 
 export interface CreateFeatureInput {
   groupId: string;
   title: string;
   description?: string;
   status?: string;
   priority?: string;
   area?: string;
   assigneeMemberId?: string;
   targetRoadmapItemId?: string;
 }
 
 export interface UpdateFeatureInput {
   title?: string;
   description?: string;
   status?: string;
   priority?: string;
   area?: string;
   assigneeMemberId?: string;
   targetRoadmapItemId?: string;
   sortOrder?: number;
 }
 
 export interface CreateFeatureTaskInput {
   featureId: string;
   title: string;
 }
 
 export interface UpdateFeatureTaskInput {
  title?: string;
  done?: boolean;
  sortOrder?: number;
}

// === Shared Memory: Experiences ===

export interface Experience {
  id: string;
  groupId: string;
  sourceMemberId: string;
  title: string;
  content: string;
  tags: string;
  createdAt: number;
  updatedAt: number;
}

export interface SaveExperienceInput {
  groupId: string;
  /** Tauri 模式使用；Web 模式由服务端从登录令牌取用户身份 */
  sourceMemberId: string;
  title: string;
  content: string;
  tags?: string;
}

// === Logs ===

export type LogLevel = "debug" | "info" | "warn" | "error";

export interface LogEntry {
  id: string;
  level: string;
  source: string;
  message: string;
  details: string | null;
  createdAt: number;
}

export interface LogQueryFilter {
  limit?: number;
  offset?: number;
  level?: LogLevel;
  source?: string;
  since?: number;
}
