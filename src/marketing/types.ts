export type CampaignStatus =
  | "collecting"
  | "planning"
  | "writing"
  | "validating"
  | "awaiting_user"
  | "changes_requested"
  | "approved"
  | "no_content"
  | "failed";

export interface Evidence {
  id: string;
  kind: string;
  source: string;
  excerpt: string;
  contentHash: string;
  releaseState: "released" | "committed" | "unreleased";
}

export interface RepositorySnapshot {
  schemaVersion: number;
  repositoryRoot: string;
  baseRef: string | null;
  headRef: string;
  sourceMode: "committed" | "include_uncommitted";
  commits: { sha: string; subject: string }[];
  changedFiles: string[];
  uncommittedFiles: string[];
  evidence: Evidence[];
  config: {
    projectContext: string;
    brandGuide: string;
    channelTemplates: Record<string, string>;
    bannedPhrases: string[];
  };
  truncated: boolean;
  collectedAt: number;
}

export interface ContentBrief {
  schemaVersion: number;
  campaignId: string;
  publishability: "publish" | "hold" | "no_content";
  reason: string;
  audience: string[];
  coreMessage: string;
  updates: {
    id: string;
    title: string;
    summary: string;
    userValue: string;
    evidenceRefs: string[];
    releaseState: "released" | "committed" | "unreleased";
  }[];
  proofPoints: { id: string; text: string; evidenceRefs: string[] }[];
  doNotClaim: string[];
  channelAngles: Record<string, string>;
}

export interface ChannelDraft {
  channel: "xiaohongshu" | "x" | "zhihu" | "bilibili" | "github_release";
  title: string;
  body: string;
  claimRefs: string[];
}

export interface ValidationFinding {
  severity: "error" | "warning";
  code: string;
  message: string;
  path: string;
}

export interface ContentCampaign {
  id: string;
  groupId: string;
  requestedBy: string;
  plannerAgentId: string;
  writerAgentId: string;
  status: CampaignStatus;
  sourceMode: "committed" | "include_uncommitted";
  baseRef: string | null;
  headRef: string;
  snapshot: RepositorySnapshot;
  brief: ContentBrief | null;
  drafts: ChannelDraft[];
  validation: ValidationFinding[];
  plannerRunId: string | null;
  writerRunId: string | null;
  revision: number;
  feedback: string | null;
  feedbackBy: string | null;
  errorMessage: string | null;
  approvedBy: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface CreateCampaignInput {
  groupId: string;
  requestedBy: string;
  plannerAgentId?: string;
  writerAgentId?: string;
  sourceMode?: "committed" | "include_uncommitted";
  baseRef?: string;
}

export interface CampaignExport {
  filename: string;
  markdown: string;
}
