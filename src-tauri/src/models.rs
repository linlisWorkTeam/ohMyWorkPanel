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
    /// `project` (default) or `chat`
    #[serde(default = "default_group_kind")]
    pub group_kind: String,
    #[serde(default)]
    pub archived: bool,
    /// Built-in seed / system group (e.g. ohMyWorkPanel); not deletable.
    #[serde(default)]
    pub is_system: bool,
    /// Unread message count for the current viewer (web list API).
    #[serde(default)]
    pub unread_count: i64,
}

fn default_group_kind() -> String {
    "project".into()
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
    #[serde(default)]
    pub workspace_path: Option<String>,
    /// Whether API key is stored (never expose raw key).
    #[serde(default)]
    pub api_key_set: bool,
    #[serde(default)]
    pub keep_alive: bool,
    #[serde(default)]
    pub warm_status: Option<String>,
    /// Preferred model id for this agent/chatbot (empty = provider default).
    #[serde(default)]
    pub model: Option<String>,
    /// Custom OpenAI-compatible base URL (chatbot provider "custom"); empty = provider default.
    #[serde(default)]
    pub api_url: Option<String>,
    /// Linked login account (`users.id`) for kind=user members.
    #[serde(default)]
    pub auth_user_id: Option<String>,
    /// User placeholder awaiting invite accept (`auth_user_id` still null).
    #[serde(default)]
    pub invite_pending: bool,
    /// Platform-locked bootstrap agent (seed system groups): read-only, cannot be
    /// edited/removed/reassigned by users; only ohMyWorkPanel group's agent holds full
    /// self-bootstrap write capability (linlis-super-harness).
    #[serde(default)]
    pub system_locked: bool,
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
    /// True when full DB content has a non-empty thinking channel (lazy-loaded in UI).
    #[serde(default)]
    pub has_thinking: bool,
    /// True when full DB content has a non-empty artifact channel (lazy-loaded in UI).
    #[serde(default)]
    pub has_artifact: bool,
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
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub phase_updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupState {
    pub group: Group,
    pub members: Vec<Member>,
    pub messages: Vec<Message>,
    pub runs: Vec<TaskRun>,
    /// True when older messages exist beyond the hot window (default 100).
    #[serde(default)]
    pub messages_has_more: bool,
    /// Total message count in group (for UI hints).
    #[serde(default)]
    pub messages_total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePage {
    pub messages: Vec<Message>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageChannelPart {
    pub message_id: String,
    pub channel: String,
    pub text: String,
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
    /// `project` | `chat` (default project)
    pub group_kind: Option<String>,
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
    /// chatbot provider: opencode-go | deepseek | custom
    pub chatbot_provider: Option<String>,
    pub api_key: Option<String>,
    /// Custom OpenAI-compatible base URL（provider=custom 时必填，其余忽略）
    pub api_url: Option<String>,
    /// Optional model override at create time
    pub model: Option<String>,
    /// Login username for kind=user (creates `users` row)
    pub login_username: Option<String>,
    /// Login password for kind=user
    pub login_password: Option<String>,
    /// Link an existing `users.id` into the group instead of creating a new login
    pub existing_auth_user_id: Option<String>,
    /// Create a pending user + 24h invite link (kind=user only; no login fields).
    #[serde(default)]
    pub invite: Option<bool>,
}

/// Login account that can be linked as a group user member.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinableUser {
    pub id: String,
    pub username: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rss_mib: Option<f64>,
}

impl ChatEvent {
    pub fn bare(kind: impl Into<String>, group_id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            group_id: group_id.into(),
            run_id: None,
            message_id: None,
            delta: None,
            status: None,
            error: None,
            channel: None,
            replace: None,
            phase: None,
            elapsed_ms: None,
            total_ms: None,
            seq: None,
            delta_count: None,
            rss_mib: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettings {
    pub max_concurrent_runs: i64,
    pub run_timeout_seconds: i64,
    pub context_message_limit: i64,
    /// Smaller native window for chat groups / chatbot (default 12). No summary/RAG.
    #[serde(default = "default_chat_context_message_limit")]
    pub chat_context_message_limit: i64,
    pub max_delegation_depth: i64,
    /// Auto heartbeat rate (focus 1s / background 5s by default).
    #[serde(default = "default_true")]
    pub heartbeat_auto: bool,
    #[serde(default = "default_heartbeat_focus")]
    pub heartbeat_focus_seconds: i64,
    #[serde(default = "default_heartbeat_background")]
    pub heartbeat_background_seconds: i64,
}

fn default_true() -> bool {
    true
}
fn default_chat_context_message_limit() -> i64 {
    12
}
fn default_heartbeat_focus() -> i64 {
    1
}
fn default_heartbeat_background() -> i64 {
    5
}

#[derive(Clone)]
pub struct ExecutionContext {
    pub run: TaskRun,
    pub group: Group,
    pub agent: Member,
    pub prompt: String,
    pub settings: RuntimeSettings,
    /// Recent group chat plain lines (oldest→newest), for chatbot native window context.
    pub recent_chat: String,
    /// Root task / triggering message plain text.
    pub root_task: String,
    /// Compact injected-context ledger line (also logged / WS).
    pub context_ledger: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapOrchestration {
    pub id: String,
    pub group_id: String,
    pub roadmap_item_id: String,
    pub status: String,
    pub cursor_feature_id: Option<String>,
    pub cursor_task_id: Option<String>,
    pub current_run_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
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

 /// 消息反馈聚合（👍/👎）。
 #[derive(Debug, Clone, Serialize)]
 #[serde(rename_all = "camelCase")]
 pub struct MessageFeedback {
     pub up: i64,
     pub down: i64,
     #[serde(skip_serializing_if = "Option::is_none")]
     pub my_vote: Option<String>,
 }

 /// run 阶段轨迹条目（时间线）。
 #[derive(Debug, Clone, Serialize)]
 #[serde(rename_all = "camelCase")]
 pub struct RunPhaseEntry {
     pub phase: String,
     pub note: String,
     pub created_at: i64,
 }

#[cfg(test)]
mod contract_tests {
    use super::*;

    fn to_json<T: serde::Serialize>(value: &T) -> serde_json::Value {
        serde_json::to_value(value).unwrap()
    }

    /// 契约锁（评审 #8 务实版）：前端 types.ts 期望 camelCase；
    /// 本测试防止 Rust DTO 再出现 my_vote/created_at 类 snake 泄漏。
    #[test]
    fn feedback_and_phase_use_camel_case() {
        let fb = MessageFeedback { up: 1, down: 2, my_vote: Some("up".into()) };
        let v = to_json(&fb);
        assert_eq!(v["up"], 1);
        assert_eq!(v["down"], 2);
        assert_eq!(v["myVote"], "up");
        assert!(v.get("my_vote").is_none(), "snake_case 键不得泄漏");

        let entry = RunPhaseEntry { phase: "streaming".into(), note: String::new(), created_at: 42 };
        let w = to_json(&entry);
        assert_eq!(w["phase"], "streaming");
        assert_eq!(w["createdAt"], 42);
        assert!(w.get("created_at").is_none());
    }

    #[test]
    fn chat_event_keys_are_camel_case() {
        let mut e = ChatEvent::bare("run_status", "g");
        e.run_id = Some("r".into());
        e.message_id = Some("m".into());
        e.status = Some("running".into());
        e.seq = Some(7);
        let v = to_json(&e);
        assert_eq!(v["groupId"], "g");
        assert_eq!(v["runId"], "r");
        assert_eq!(v["messageId"], "m");
        assert_eq!(v["seq"], 7);
        assert!(v.get("group_id").is_none());
        assert!(v.get("elapsed_ms").is_none(), "None 字段应被 skip_serializing_if 省略");
    }

    #[test]
    fn group_task_run_member_shape_matches_frontend() {
        let group = Group {
            id: "g".into(), name: "g".into(), workspace_path: "/w".into(),
            owner_member_id: "o".into(), admin_member_id: None, created_at: 1,
            announcement: String::new(), announcement_updated_at: None,
            group_kind: "project".into(), archived: false, is_system: true, unread_count: 3,
        };
        let gv = to_json(&group);
        for key in ["id", "name", "workspacePath", "ownerMemberId", "createdAt", "groupKind", "isSystem", "unreadCount"] {
            assert!(gv.get(key).is_some(), "缺少 {}", key);
        }
        assert!(gv.get("workspace_path").is_none());
        assert_eq!(gv["groupKind"], "project");

        let run = TaskRun {
            id: "r".into(), group_id: "g".into(), root_message_id: "m".into(),
            agent_member_id: "a".into(), parent_run_id: None, depth: 0, status: "queued".into(),
            output_message_id: None, error_message: None, review_status: None,
            reviewer_member_id: None, created_at: 1, started_at: None, completed_at: None,
            phase: Some("starting".into()), phase_updated_at: Some(9),
        };
        let rv = to_json(&run);
        for key in ["rootMessageId", "agentMemberId", "outputMessageId", "reviewStatus", "reviewerMemberId", "phaseUpdatedAt"] {
            assert!(rv.get(key).is_some(), "缺少 {}", key);
        }
        assert_eq!(rv["phase"], "starting");

        let member = Member {
            id: "a".into(), group_id: "g".into(), kind: "agent".into(), display_name: "A".into(),
            avatar_color: "#000".into(), role_description: String::new(), is_active: true,
            adapter: Some("mock".into()), executable_path: None, runtime_status: Some("ready".into()),
            tags: String::new(), created_at: 1, workspace_path: None, api_key_set: true,
            keep_alive: false, warm_status: None, model: None, auth_user_id: None,
            invite_pending: false, system_locked: false, api_url: Some("https://api.example.com/v1".into()),
        };
        let mv = to_json(&member);
        for key in ["displayName", "avatarColor", "roleDescription", "isActive", "runtimeStatus", "apiKeySet", "keepAlive", "authUserId", "invitePending", "systemLocked", "apiUrl"] {
            assert!(mv.get(key).is_some(), "缺少 {}", key);
        }
        assert!(mv.get("api_key").is_none(), "原始 API key 永不外泄（只暴露 apiKeySet）");
        assert!(mv.get("display_name").is_none());
    }
}
