use serde::{Deserialize, Serialize};

/// A predefined agent role template used during group creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetRole {
    pub name: String,
    pub adapter: String,
    pub role_description: String,
    pub avatar_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    pub workspace_path: String,
    pub owner_member_id: String,
    pub admin_member_id: Option<String>,
    pub created_at: i64,
    #[serde(default)]
    pub announcement: String,
    #[serde(default)]
    pub announcement_updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: String,
    pub group_id: String,
    pub kind: String,
    pub display_name: String,
    pub avatar_color: String,
    pub role_description: String,
    pub is_active: bool,
    pub adapter: Option<String>,
    pub executable_path: Option<String>,
    pub runtime_status: Option<String>,
    pub tags: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub group_id: String,
    pub sender_member_id: String,
    pub parent_run_id: Option<String>,
    pub content: String,
    pub status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRun {
    pub id: String,
    pub group_id: String,
    pub root_message_id: String,
    pub agent_member_id: String,
    pub parent_run_id: Option<String>,
    pub depth: i64,
    pub status: String,
    pub output_message_id: Option<String>,
    pub error_message: Option<String>,
    pub review_status: Option<String>,
    pub reviewer_member_id: Option<String>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupState {
    pub group: Group,
    pub members: Vec<Member>,
    pub messages: Vec<Message>,
    pub runs: Vec<TaskRun>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub groups: Vec<Group>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGroupInput {
    pub name: String,
    pub workspace_path: String,
    pub owner_name: String,
    pub preset_roles: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMemberInput {
    pub group_id: String,
    pub kind: String,
    pub display_name: String,
    pub role_description: String,
    pub avatar_color: Option<String>,
    pub adapter: Option<String>,
    pub executable_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    pub message: Message,
    pub run_ids: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    pub kind: String,
    pub group_id: String,
    pub run_id: Option<String>,
    pub message_id: Option<String>,
    pub delta: Option<String>,
    pub status: Option<String>,
    pub error: Option<String>,
    /// thinking | artifact | final — omitted/None means final for older clients
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// When true, `delta` replaces the channel text instead of appending.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replace: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettings {
    pub max_concurrent_runs: i64,
    pub run_timeout_seconds: i64,
    pub context_message_limit: i64,
    pub max_delegation_depth: i64,
}

#[derive(Clone)]
pub struct ExecutionContext {
    pub run: TaskRun,
    pub group: Group,
    pub agent: Member,
    pub prompt: String,
    pub settings: RuntimeSettings,
}
 
 // === Project Management ===
 
 #[derive(Debug, Clone, Serialize, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct RoadmapItem {
     pub id: String,
     pub group_id: String,
     pub title: String,
     pub description: String,
     pub status: String,
     pub priority: String,
     pub target_date: Option<String>,
     pub sort_order: i64,
     pub created_at: i64,
 }
 
 #[derive(Debug, Clone, Serialize, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct Feature {
     pub id: String,
     pub group_id: String,
     pub title: String,
     pub description: String,
     pub status: String,
     pub priority: String,
     pub area: String,
     pub assignee_member_id: Option<String>,
     pub target_roadmap_item_id: Option<String>,
     pub sort_order: i64,
     pub created_at: i64,
     pub updated_at: i64,
 }
 
 #[derive(Debug, Clone, Serialize, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct FeatureTask {
     pub id: String,
     pub feature_id: String,
     pub title: String,
     pub done: bool,
     pub sort_order: i64,
     pub created_at: i64,
 }
 
 #[derive(Debug, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct CreateRoadmapItemInput {
     pub group_id: String,
     pub title: String,
     pub description: Option<String>,
     pub status: Option<String>,
     pub priority: Option<String>,
     pub target_date: Option<String>,
 }
 
 #[derive(Debug, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct UpdateRoadmapItemInput {
     pub title: Option<String>,
     pub description: Option<String>,
     pub status: Option<String>,
     pub priority: Option<String>,
     pub target_date: Option<String>,
     pub sort_order: Option<i64>,
 }
 
 #[derive(Debug, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct CreateFeatureInput {
     pub group_id: String,
     pub title: String,
     pub description: Option<String>,
     pub status: Option<String>,
     pub priority: Option<String>,
     pub area: Option<String>,
     pub assignee_member_id: Option<String>,
     pub target_roadmap_item_id: Option<String>,
 }
 
 #[derive(Debug, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct UpdateFeatureInput {
     pub title: Option<String>,
     pub description: Option<String>,
     pub status: Option<String>,
     pub priority: Option<String>,
     pub area: Option<String>,
     pub assignee_member_id: Option<String>,
     pub target_roadmap_item_id: Option<String>,
     pub sort_order: Option<i64>,
 }
 
 #[derive(Debug, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct CreateFeatureTaskInput {
     pub feature_id: String,
     pub title: String,
 }
 
 #[derive(Debug, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct UpdateFeatureTaskInput {
     pub title: Option<String>,
     pub done: Option<bool>,
     pub sort_order: Option<i64>,
 }
 
 #[derive(Debug, Clone, Serialize, Deserialize)]
 #[serde(rename_all = "camelCase")]
 pub struct Experience {
     pub id: String,
     pub group_id: String,
     pub source_member_id: String,
     pub title: String,
     pub content: String,
     pub tags: String,
     pub created_at: i64,
     pub updated_at: i64,
 }

 #[derive(Debug, Clone, Serialize)]
 #[serde(rename_all = "camelCase")]
 pub struct RoadmapState {
     pub group_id: String,
     pub items: Vec<RoadmapItem>,
     pub features: Vec<Feature>,
     pub tasks: Vec<FeatureTask>,
 }
