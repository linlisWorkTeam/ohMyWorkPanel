use serde::{Deserialize, Serialize};

pub const REQUIRED_CHANNELS: [&str; 5] =
    ["xiaohongshu", "x", "zhihu", "bilibili", "github_release"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub id: String,
    pub kind: String,
    pub source: String,
    pub excerpt: String,
    pub content_hash: String,
    pub release_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub sha: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MarketingConfig {
    pub project_context: String,
    pub brand_guide: String,
    pub channel_templates: std::collections::BTreeMap<String, String>,
    pub banned_phrases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySnapshot {
    pub schema_version: i64,
    pub repository_root: String,
    pub base_ref: Option<String>,
    pub head_ref: String,
    pub source_mode: String,
    pub commits: Vec<CommitSummary>,
    pub changed_files: Vec<String>,
    pub uncommitted_files: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub config: MarketingConfig,
    pub truncated: bool,
    pub collected_at: i64,
}

impl RepositorySnapshot {
    pub fn has_candidate_updates(&self) -> bool {
        !self.commits.is_empty()
            || (self.source_mode == "include_uncommitted" && !self.uncommitted_files.is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentUpdate {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub user_value: String,
    pub evidence_refs: Vec<String>,
    pub release_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofPoint {
    pub id: String,
    pub text: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentBrief {
    pub schema_version: i64,
    pub campaign_id: String,
    pub publishability: String,
    pub reason: String,
    pub audience: Vec<String>,
    pub core_message: String,
    pub updates: Vec<ContentUpdate>,
    pub proof_points: Vec<ProofPoint>,
    pub do_not_claim: Vec<String>,
    pub channel_angles: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelDraft {
    pub channel: String,
    pub title: String,
    pub body: String,
    pub claim_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftBundle {
    pub schema_version: i64,
    pub campaign_id: String,
    pub drafts: Vec<ChannelDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationFinding {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCampaign {
    pub id: String,
    pub group_id: String,
    pub requested_by: String,
    pub planner_agent_id: String,
    pub writer_agent_id: String,
    pub status: String,
    pub source_mode: String,
    pub base_ref: Option<String>,
    pub head_ref: String,
    pub snapshot: RepositorySnapshot,
    pub brief: Option<ContentBrief>,
    pub drafts: Vec<ChannelDraft>,
    pub validation: Vec<ValidationFinding>,
    pub planner_run_id: Option<String>,
    pub writer_run_id: Option<String>,
    pub revision: i64,
    pub feedback: Option<String>,
    pub feedback_by: Option<String>,
    pub error_message: Option<String>,
    pub approved_by: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCampaignInput {
    pub group_id: String,
    pub requested_by: String,
    pub planner_agent_id: Option<String>,
    pub writer_agent_id: Option<String>,
    pub source_mode: Option<String>,
    pub base_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviseCampaignInput {
    pub actor_member_id: String,
    pub feedback: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveCampaignInput {
    pub actor_member_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignExport {
    pub filename: String,
    pub markdown: String,
}
