use crate::adapters::AdapterKind;
use crate::db::{
    create_feature_db, create_feature_task_db, create_roadmap_item_db, create_task_run,
    delete_experience as delete_experience_db, delete_feature_db, delete_feature_task_db,
    delete_roadmap_item_db, get_feature_tasks, get_features, get_group, get_groups,
    get_preset_roles, get_roadmap_items, get_roadmap_state_db, get_settings_from, group_state, id,
    insert_run_event, member_from_row, now, open_db, query_experiences as query_experiences_db,
    resolve_target_agent_ids, run_from_row, save_experience as save_experience_db,
    update_feature_db, update_feature_task_db, update_roadmap_item_db, AppResult,
};
use crate::event_sender::EventSender;
use crate::logger::{self, LogEntry, LogQuery};
use crate::models::{
    AddMemberInput, Bootstrap, ChatEvent, CreateFeatureInput, CreateFeatureTaskInput,
    CreateGroupInput, CreateRoadmapItemInput, Experience, Feature, FeatureTask, GroupState, Member,
    Message, PresetRole, RoadmapItem, RoadmapState, RuntimeSettings, SendResult, TaskRun,
    UpdateFeatureInput, UpdateFeatureTaskInput, UpdateRoadmapItemInput,
};
use crate::scheduler::{schedule_group, SchedulerState};
use crate::AppState;
pub mod goals;
pub use goals::*;
pub mod workflow;
pub use workflow::*;
pub mod marketing;
pub use marketing::*;
pub mod members;
pub use members::*;
pub mod pm;
pub use pm::*;
pub mod observability;
pub use observability::*;
pub mod ops;
pub use ops::*;
pub mod chat;
pub use chat::*;
pub mod update;
pub use update::*;
pub mod agents;
pub use agents::*;

fn normalize_group_kind(raw: Option<&str>) -> String {
    match raw.map(str::trim).unwrap_or("project") {
        "chat" => "chat".into(),
        _ => "project".into(),
    }
}

use rusqlite::{params, OptionalExtension};
use std::{sync::atomic::Ordering, time::Duration};
use tauri::{AppHandle, Emitter, State};
use tokio::process::Command;

/// Convert Tauri AppState+AppHandle to SchedulerState for use by scheduler functions
pub(crate) fn to_scheduler(state: &AppState, app: &AppHandle) -> SchedulerState {
    SchedulerState {
        db_path: state.db_path.clone(),
        event_sender: EventSender::Tauri(app.clone()),
        cancellations: state.cancellations.clone(),
        scheduling_groups: state.scheduling_groups.clone(),
        live_sessions: state.live_sessions.clone(),
    }
}

/// Reject mutations on platform-locked bootstrap agents (`agent_profiles.system_locked=1`).
fn assert_member_mutable(conn: &rusqlite::Connection, member_id: &str) -> AppResult<()> {
    let locked: i64 = conn
        .query_row(
            "SELECT COALESCE(p.system_locked,0) FROM agent_profiles p WHERE p.member_id=?1",
            params![member_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if locked != 0 {
        return Err(
            "平台锁定的自举 Agent（bootstrap-dsh / linlis-super-harness）不可修改或移除。".into(),
        );
    }
    Ok(())
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> AppResult<Bootstrap> {
    Ok(Bootstrap {
        groups: get_groups(&open_db(&state.db_path)?)?,
    })
}

#[tauri::command]
pub fn get_group_state(group_id: String, state: State<'_, AppState>) -> AppResult<GroupState> {
    group_state(&open_db(&state.db_path)?, &group_id)
}

#[tauri::command]
pub fn get_runtime_settings(state: State<'_, AppState>) -> AppResult<RuntimeSettings> {
    get_settings_from(&open_db(&state.db_path)?)
}

#[tauri::command]
pub fn update_runtime_settings(
    settings: RuntimeSettings,
    state: State<'_, AppState>,
) -> AppResult<RuntimeSettings> {
    if settings.max_concurrent_runs < 1
        || settings.run_timeout_seconds < 30
        || settings.context_message_limit < 5
        || !(5..=40).contains(&settings.chat_context_message_limit)
        || !(0..=4).contains(&settings.max_delegation_depth)
    {
        return Err("运行设置超出允许范围。".into());
    }
    if !(1..=30).contains(&settings.heartbeat_focus_seconds)
        || !(1..=60).contains(&settings.heartbeat_background_seconds)
    {
        return Err("心跳间隔超出允许范围。".into());
    }
    let conn = open_db(&state.db_path)?;
    for (key, value) in [
        ("max_concurrent_runs", settings.max_concurrent_runs),
        ("run_timeout_seconds", settings.run_timeout_seconds),
        ("context_message_limit", settings.context_message_limit),
        (
            "chat_context_message_limit",
            settings.chat_context_message_limit,
        ),
        ("max_delegation_depth", settings.max_delegation_depth),
        (
            "heartbeat_auto",
            if settings.heartbeat_auto { 1 } else { 0 },
        ),
        ("heartbeat_focus_seconds", settings.heartbeat_focus_seconds),
        (
            "heartbeat_background_seconds",
            settings.heartbeat_background_seconds,
        ),
    ] {
        conn.execute(
            "INSERT INTO app_settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }
    get_settings_from(&conn)
}

#[tauri::command]
pub fn create_group(input: CreateGroupInput, state: State<'_, AppState>) -> AppResult<GroupState> {
    let name = input.name.trim();
    let owner_name = input.owner_name.trim();
    if name.is_empty() || owner_name.is_empty() {
        return Err("群名称和群主名称不能为空。".into());
    }
    let group_kind = normalize_group_kind(input.group_kind.as_deref());
    let workspace_path = if group_kind == "chat" {
        String::new()
    } else {
        crate::fs_browse::resolve_server_dir(&input.workspace_path)?
            .to_string_lossy()
            .into_owned()
    };
    let group_id = id();
    let owner_id = id();
    let created_at = now();
    let conn = open_db(&state.db_path)?;
    conn.execute(
        "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,group_kind,archived) VALUES(?1,?2,?3,?4,NULL,?5,?6,0)",
        params![group_id, name, workspace_path, owner_id, created_at, group_kind],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES(?1,?2,'user',?3,'#5167f6','群主',1,?4)",
        params![owner_id, group_id, owner_name, created_at],
    )
    .map_err(|e| e.to_string())?;
    // Auto-create agent members from selected preset roles
    if let Some(role_names) = &input.preset_roles {
        let all_roles = get_preset_roles(&conn)?;
        for role in all_roles {
            if !role_names.contains(&role.name) {
                continue;
            }
            let mid = id();
            conn.execute(
                "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES(?1,?2,'agent',?3,?4,?5,1,?6)",
                params![mid, group_id, role.name, role.avatar_color, role.role_description, created_at],
            ).map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at) VALUES(?1,?2,NULL,'unknown',?3)",
                params![mid, role.adapter, created_at],
            ).map_err(|e| e.to_string())?;
        }
    }
    // 普通项目群预制极简 bootstrap-dsh（不可修改；chat 群不建）
    crate::db::ensure_minimal_bootstrap_dsh(&conn, &group_id, &group_kind)?;
    group_state(&conn, &group_id)
}

#[tauri::command]
pub fn list_server_dir(path: String) -> AppResult<crate::fs_browse::DirListing> {
    crate::fs_browse::list_server_dir(&path)
}

#[tauri::command]
pub fn create_server_dir(parent: String, name: String) -> AppResult<String> {
    crate::fs_browse::create_server_dir(&parent, &name).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn update_group_workspace_cmd(
    group_id: String,
    workspace_path: String,
    state: State<'_, AppState>,
) -> AppResult<crate::models::Group> {
    let workspace = crate::fs_browse::resolve_server_dir(&workspace_path)?;
    let conn = open_db(&state.db_path)?;
    crate::db::update_group_workspace(&conn, &group_id, workspace.to_string_lossy().as_ref())
}

#[tauri::command]
pub fn update_member_workspace_cmd(
    member_id: String,
    workspace_path: String,
    state: State<'_, AppState>,
) -> AppResult<crate::models::Member> {
    let conn = open_db(&state.db_path)?;
    assert_member_mutable(&conn, &member_id)?;
    let member = conn
        .query_row(
            &format!("{} WHERE m.id=?1", crate::db::MEMBER_SELECT),
            rusqlite::params![member_id],
            crate::db::member_from_row,
        )
        .map_err(|e| e.to_string())?;
    let group = get_group(&conn, &member.group_id)?;
    let resolved = crate::memory::resolve_agent_workspace(
        std::path::Path::new(&group.workspace_path),
        &member_id,
        Some(&workspace_path),
        group.is_system,
    )?;
    crate::db::set_member_workspace(&conn, &member_id, resolved.to_string_lossy().as_ref())?;
    conn.query_row(
        &format!("{} WHERE m.id=?1", crate::db::MEMBER_SELECT),
        rusqlite::params![member_id],
        crate::db::member_from_row,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_group_announcement(
    group_id: String,
    state: State<'_, AppState>,
) -> AppResult<crate::models::Group> {
    let conn = open_db(&state.db_path)?;
    get_group(&conn, &group_id)
}

#[tauri::command]
pub fn set_group_announcement_cmd(
    group_id: String,
    announcement: String,
    state: State<'_, AppState>,
) -> AppResult<crate::models::Group> {
    let conn = open_db(&state.db_path)?;
    let g = crate::db::set_group_announcement(&conn, &group_id, announcement.trim())?;
    let _ = crate::fs_browse::sync_announcement_rule(
        std::path::Path::new(&g.workspace_path),
        &g.announcement,
    );
    Ok(g)
}

#[tauri::command]
pub fn set_group_archived_cmd(
    group_id: String,
    archived: bool,
    state: State<'_, AppState>,
) -> AppResult<crate::models::Group> {
    let conn = open_db(&state.db_path)?;
    crate::db::set_group_archived(&conn, &group_id, archived)
}

#[tauri::command]
pub fn update_member_model_cmd(
    member_id: String,
    model: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Member> {
    let conn = open_db(&state.db_path)?;
    assert_member_mutable(&conn, &member_id)?;
    crate::db::set_member_model(&conn, &member_id, model.as_deref())?;
    conn.query_row(
        &format!("{} WHERE m.id=?1", crate::db::MEMBER_SELECT),
        params![member_id],
        member_from_row,
    )
    .map_err(|e| e.to_string())
}

/// 人类对「待审批」任务的裁决（P2：审批内联）。
/// decision: approved → review_status=approved/status=completed；rejected → rejected/changes_requested。
/// 解析 "x.y.z" / "x.y.z-beta" / "x.y.z+build" → 数值三元组（忽略预发布/构建后缀）。
/// （比较逻辑与单测见 models::parse_version / models::is_newer_version，双特性共同编译。）
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub enabled: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub has_update: bool,
    pub notes: Option<String>,
    pub download_url: Option<String>,
    pub sha256: Option<String>,
    pub error: Option<String>,
}

/// 检查更新：拉取更新清单（{version,notes,url,sha256}），与当前版本比较。
/// 未配置 manifest_url 时的自动发现候选源（依次尝试，取第一个可解析的清单）：
/// 1) 本机灰度 Web 服务（出包脚本同步 dist/update.json）
/// 2) 云端发布源（ECS 灰度 Web；正式发布时部署清单+安装包）
const DEFAULT_MANIFEST_CANDIDATES: [&str; 2] = [
    "http://127.0.0.1:8082/update.json",
    "http://101.132.60.79:8081/update.json",
];

// ============================================================================
// 版本 / Wave 工作流（Desktop 桥——与 web handler 同语义；桌面为单机无登录）
// ============================================================================

/// 桌面版版本管理权限：发送者须为本群活跃成员，且为群主或管理员成员。
fn require_desktop_version_manager(
    conn: &rusqlite::Connection,
    group: &crate::models::Group,
    sender_member_id: &str,
) -> AppResult<()> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND is_active=1",
            params![sender_member_id, group.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if count != 1 {
        return Err("发送者不是本群活跃成员。".into());
    }
    let is_manager = group.owner_member_id == sender_member_id
        || group.admin_member_id.as_deref() == Some(sender_member_id);
    if !is_manager {
        return Err("仅群主或管理员可管理版本".into());
    }
    Ok(())
}

/// 桌面版：以群管理员 Agent 发起一次 kickoff 任务（消息 + mention + run + 调度）。
fn kickoff_admin_run_desktop(
    conn: &rusqlite::Connection,
    state: &AppState,
    app: &AppHandle,
    group: &crate::models::Group,
    sender_member_id: &str,
    content: String,
) -> AppResult<String> {
    let admin = group
        .admin_member_id
        .clone()
        .ok_or_else(|| "未设置管理员 Agent".to_string())?;
    let msg = Message {
        id: id(),
        group_id: group.id.clone(),
        sender_member_id: sender_member_id.to_string(),
        parent_run_id: None,
        content,
        status: "completed".into(),
        created_at: now(),
        has_thinking: false,
        has_artifact: false,
    };
    conn.execute(
        "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,?5,?6)",
        params![msg.id, msg.group_id, msg.sender_member_id, msg.content, msg.status, msg.created_at],
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.execute(
        "INSERT OR IGNORE INTO mentions(message_id,member_id) VALUES(?1,?2)",
        params![msg.id, admin],
    );
    let rid = create_task_run(conn, &group.id, &msg.id, &admin, None, 0)?;
    let _ = app.emit(
        "chat-event",
        ChatEvent {
            kind: "message_created".into(),
            group_id: group.id.clone(),
            run_id: None,
            message_id: Some(msg.id),
            delta: None,
            status: Some("completed".into()),
            error: None,
            channel: None,
            replace: None,
            phase: None,
            elapsed_ms: None,
            total_ms: None,
            seq: None,
            delta_count: None,
            rss_mib: None,
        },
    );
    schedule_group(to_scheduler(state, app), group.id.clone());
    Ok(rid)
}

fn admin_display_name(conn: &rusqlite::Connection, group: &crate::models::Group) -> String {
    group
        .admin_member_id
        .as_ref()
        .and_then(|aid| {
            conn.query_row(
                "SELECT display_name FROM members WHERE id=?1",
                params![aid],
                |r| r.get::<_, String>(0),
            )
            .ok()
        })
        .unwrap_or_else(|| "管理员".into())
}
