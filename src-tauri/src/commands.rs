use crate::adapters::AdapterKind;
use crate::db::{
    create_task_run, get_group, get_groups, get_preset_roles, get_settings_from,
    group_state, id, insert_run_event, member_from_row, now, open_db, resolve_target_agent_ids, run_from_row, AppResult,
    create_roadmap_item_db, get_roadmap_items, update_roadmap_item_db, delete_roadmap_item_db,
    create_feature_db, get_features, update_feature_db, delete_feature_db,
    create_feature_task_db, get_feature_tasks, update_feature_task_db, delete_feature_task_db,
    get_roadmap_state_db,
    save_experience as save_experience_db, query_experiences as query_experiences_db,
    delete_experience as delete_experience_db,
};
use crate::logger::{self, LogEntry, LogQuery};
use crate::models::{
    AddMemberInput, Bootstrap, ChatEvent, CreateGroupInput, GroupState, Member, Message, PresetRole,
    RuntimeSettings, SendResult, TaskRun,
    RoadmapItem, Feature, FeatureTask, CreateRoadmapItemInput, UpdateRoadmapItemInput,
    CreateFeatureInput, UpdateFeatureInput, CreateFeatureTaskInput, UpdateFeatureTaskInput,
    RoadmapState, Experience,
};
use crate::scheduler::{emit, schedule_group, SchedulerState};
use crate::event_sender::EventSender;
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, State};
use tokio::process::Command;

/// Convert Tauri AppState+AppHandle to SchedulerState for use by scheduler functions
fn to_scheduler(state: &AppState, app: &AppHandle) -> SchedulerState {
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
        return Err("平台锁定的自举 Agent（bootstrap-dsh / linlis-super-harness）不可修改或移除。".into());
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
pub fn list_messages_before(
    group_id: String,
    before_created_at: i64,
    before_id: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<crate::models::MessagePage> {
    let conn = open_db(&state.db_path)?;
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let messages =
        crate::db::get_messages_before(&conn, &group_id, before_created_at, &before_id, limit)?;
    let has_more = messages.len() as i64 >= limit;
    Ok(crate::models::MessagePage {
        messages: crate::db::project_messages_for_client(messages),
        has_more,
    })
}

#[tauri::command]
pub fn get_message_channel_part(
    group_id: String,
    message_id: String,
    channel: String,
    state: State<'_, AppState>,
) -> AppResult<crate::models::MessageChannelPart> {
    let conn = open_db(&state.db_path)?;
    let text = crate::db::get_message_channel_text(&conn, &group_id, &message_id, &channel)?;
    Ok(crate::models::MessageChannelPart {
        message_id,
        channel: crate::message_content::normalize_channel(&channel),
        text,
    })
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
            if !role_names.contains(&role.name) { continue; }
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
pub fn add_member(input: AddMemberInput, state: State<'_, AppState>) -> AppResult<Member> {
    if !matches!(input.kind.as_str(), "user" | "agent" | "chatbot") || input.display_name.trim().is_empty() {
        return Err("成员类型或名称无效。".into());
    }
    let invite_mode = input.invite.unwrap_or(false);
    if invite_mode && input.kind != "user" {
        return Err("仅用户成员支持邀请链接。".into());
    }
    let conn = open_db(&state.db_path)?;
    let group = get_group(&conn, &input.group_id)?;
    if input.kind == "chatbot" && group.group_kind != "chat" {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM members WHERE group_id=?1 AND kind='chatbot' AND is_active=1",
                params![input.group_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists > 0 {
            return Err("项目群只能添加一个聊天机器人；聊天群可添加多个。".into());
        }
    }
    let member_id = id();
    let created_at = now();
    let color = input.avatar_color.unwrap_or_else(|| match input.kind.as_str() {
        "agent" => "#17a673".into(),
        "chatbot" => "#0ea5a0".into(),
        _ => "#5167f6".into(),
    });
    let mut auth_user_id: Option<String> = None;
    if input.kind == "user" && !invite_mode {
        auth_user_id = Some(crate::db::resolve_user_member_auth_id(
            &conn,
            &input.group_id,
            input.existing_auth_user_id.as_deref(),
            input.login_username.as_deref(),
            input.login_password.as_deref(),
        )?);
    }
    conn.execute(
        "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,auth_user_id) VALUES(?1,?2,?3,?4,?5,?6,1,?7,?8)",
        params![
            member_id,
            input.group_id,
            input.kind,
            input.display_name.trim(),
            color,
            input.role_description.trim(),
            created_at,
            auth_user_id
        ],
    )
    .map_err(|e| e.to_string())?;
    if input.kind == "agent" {
        let adapter = input.adapter.unwrap_or_else(|| "mock".into());
        AdapterKind::parse(&adapter)?;
        let default_ws = if group.workspace_path.trim().is_empty() {
            None
        } else {
            let _ = crate::memory::ensure_linlis_layout(
                std::path::Path::new(&group.workspace_path),
                Some(&member_id),
            );
            Some(
                crate::memory::default_agent_workspace(
                    std::path::Path::new(&group.workspace_path),
                    &member_id,
                )
                .to_string_lossy()
                .into_owned(),
            )
        };
        let model = input
            .model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        conn.execute(
            "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at,workspace_path,warm_status,model) VALUES(?1,?2,?3,'unknown',?4,?5,'cold',?6)",
            params![
                member_id,
                adapter,
                input.executable_path.filter(|p| !p.trim().is_empty()),
                created_at,
                default_ws,
                model
            ],
        )
        .map_err(|e| e.to_string())?;
    } else if input.kind == "chatbot" {
        let provider = input.chatbot_provider.as_deref().unwrap_or("opencode-go");
        let adapter = crate::adapters::chatbot::normalize_adapter(provider)?;
        let api_key = input
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "聊天机器人必须填写 API Key".to_string())?;
        let model = input
            .model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("deepseek-v4-flash");
        conn.execute(
            "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at,api_key,warm_status,model) VALUES(?1,?2,NULL,'ready',?3,?4,'cold',?5)",
            params![member_id, adapter, created_at, api_key, model],
        )
        .map_err(|e| e.to_string())?;
    }
    if invite_mode {
        let _ = crate::db::create_group_invite(&conn, &input.group_id, &member_id, None)?;
    }
    crate::db::get_member(&conn, &member_id)
}

#[tauri::command]
pub fn list_joinable_users(group_id: String, state: State<'_, AppState>) -> AppResult<Vec<crate::models::JoinableUser>> {
    let conn = open_db(&state.db_path)?;
    get_group(&conn, &group_id)?;
    crate::db::list_joinable_users(&conn, &group_id)
}

#[tauri::command]
pub async fn remove_member(
    group_id: String,
    member_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<()> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let group = get_group(&conn, &group_id)?;
    assert_member_mutable(&conn, &member_id)?;
    if group.owner_member_id == member_id {
        return Err("不能移除群主。".into());
    }
    let member: Member = conn
        .query_row(
            "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,COALESCE(m.tags,''),m.created_at,p.workspace_path,p.api_key,COALESCE(p.keep_alive,0),p.warm_status,p.model,m.auth_user_id FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.id=?1 AND m.group_id=?2",
            params![member_id, group_id],
            member_from_row,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "找不到成员。".to_string())?;
    if member.kind == "agent" {
        let mut stmt = conn
            .prepare("SELECT id FROM task_runs WHERE agent_member_id=?1 AND status='running'")
            .map_err(|e| e.to_string())?;
        let run_ids = stmt
            .query_map(params![member_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for run_id in run_ids {
            if let Some(token) = state
                .cancellations
                .lock()
                .map_err(|_| "取消锁不可用".to_string())?
                .get(&run_id)
            {
                token.store(true, Ordering::SeqCst);
            }
        }
        conn.execute(
            "UPDATE task_runs SET status='cancelled',completed_at=?1 WHERE agent_member_id=?2 AND status IN ('queued','running')",
            params![now(), member_id],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute(
        "UPDATE members SET is_active=0 WHERE id=?1",
        params![member_id],
    )
    .map_err(|e| e.to_string())?;
    if group.admin_member_id.as_deref() == Some(member_id.as_str()) {
        conn.execute(
            "UPDATE groups SET admin_member_id=NULL WHERE id=?1",
            params![group_id],
        )
        .map_err(|e| e.to_string())?;
    }
    let _ = app.emit("chat-event", ChatEvent {
            kind: "member_removed".into(),
            group_id,
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
        },
    );
    Ok(())
}

#[tauri::command]
pub fn set_admin(
    group_id: String,
    member_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<GroupState> {
    let conn = open_db(&state.db_path)?;
    let _ = get_group(&conn, &group_id)?;
    if let Some(id) = &member_id {
        assert_member_mutable(&conn, id)?;
        if !crate::db::member_is_default_responder_candidate(&conn, &group_id, id)? {
            return Err("默认响应者必须是本群活跃的 Agent 或聊天机器人。".into());
        }
    }
    conn.execute(
        "UPDATE groups SET admin_member_id=?1 WHERE id=?2",
        params![member_id, group_id],
    )
    .map_err(|e| e.to_string())?;
    // Only admin *agent* gets keep-alive; chatbots stay cold / fast HTTP.
    let _ = conn.execute(
        "UPDATE agent_profiles SET keep_alive=0, updated_at=?1 WHERE member_id IN (SELECT id FROM members WHERE group_id=?2 AND kind='agent')",
        params![now(), group_id],
    );
    if let Some(id) = &member_id {
        let kind: String = conn
            .query_row(
                "SELECT kind FROM members WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        if kind == "agent" {
            let _ = conn.execute(
                "UPDATE agent_profiles SET keep_alive=1, warm_status='warming', updated_at=?1 WHERE member_id=?2",
                params![now(), id],
            );
        }
    }
    group_state(&conn, &group_id)
}

#[tauri::command]
pub async fn send_message(
    group_id: String,
    sender_member_id: String,
    content: String,
    mention_member_ids: Vec<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<SendResult> {
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err("消息不能为空。".into());
    }
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let group = get_group(&conn, &group_id)?;
    let sender_count = conn
        .query_row(
            "SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND is_active=1",
            params![sender_member_id, group_id],
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?;
    if sender_count != 1 {
        return Err("发送者不属于该群或已被移除。".into());
    }
    let message = Message {
        id: id(),
        group_id: group_id.clone(),
        sender_member_id: sender_member_id.clone(),
        parent_run_id: None,
        content,
        status: "completed".into(),
        created_at: now(),
        has_thinking: false,
        has_artifact: false,
    };
    conn.execute(
        "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,?5,?6)",
        params![
            message.id,
            message.group_id,
            message.sender_member_id,
            message.content,
            message.status,
            message.created_at
        ],
    )
    .map_err(|e| e.to_string())?;
    for mentioned in &mention_member_ids {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO mentions(message_id,member_id) SELECT ?1,id FROM members WHERE id=?2 AND group_id=?3",
            params![message.id, mentioned, group_id],
        );
    }
    let mut slash_reply: Option<(String, crate::models::Message)> = None;
    let group_kind: String = conn
        .query_row("SELECT group_kind FROM groups WHERE id=?1", params![&group_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let sender_kind: String = conn
        .query_row("SELECT kind FROM members WHERE id=?1 AND group_id=?2", params![&sender_member_id, &group_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if let Some(reply_text) = crate::workflow::try_slash_command(&conn, &group_id, &group_kind, &sender_kind, &message.content)? {
        let reply = crate::models::Message {
            id: id(),
            group_id: group_id.clone(),
            sender_member_id: group.admin_member_id.clone().unwrap_or_else(|| sender_member_id.clone()),
            parent_run_id: None,
            content: reply_text,
            status: "completed".into(),
            created_at: now(),
            has_thinking: false,
            has_artifact: false,
        };
        conn.execute(
            "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,'completed',?5)",
            params![reply.id, reply.group_id, reply.sender_member_id, reply.content, reply.created_at],
        )
        .map_err(|e| e.to_string())?;
        slash_reply = Some((reply.id.clone(), reply));
    }
    let target_agents = if slash_reply.is_none() {
        resolve_target_agent_ids(
            &conn,
            &group_id,
            group.admin_member_id.as_deref(),
            &mention_member_ids,
        )?
    } else {
        Vec::new()
    };
    let mut run_ids = Vec::new();
    for agent_id in target_agents {
        run_ids.push(create_task_run(
            &conn,
            &group_id,
            &message.id,
            &agent_id,
            None,
            0,
        )?);
    }
    drop(conn);
    let _ = app.emit("chat-event", ChatEvent {
            kind: "message_created".into(),
            group_id: group_id.clone(),
            run_id: None,
            message_id: Some(message.id.clone()),
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
    if let Some((reply_id, _)) = slash_reply {
        let _ = app.emit("chat-event", ChatEvent {
            kind: "message_created".into(),
            group_id: group_id.clone(),
            run_id: None,
            message_id: Some(reply_id),
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
        });
    }
    schedule_group(to_scheduler(&state, &app), group_id.clone());
    Ok(SendResult { message, run_ids })
}

#[tauri::command]
pub async fn cancel_run(
    run_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<()> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let Some(info) = crate::db::cancel_run(&conn, &run_id)? else {
        return Ok(());
    };
    if let Some(token) = state
        .cancellations
        .lock()
        .map_err(|_| "取消锁不可用".to_string())?
        .get(&run_id)
    {
        token.store(true, Ordering::SeqCst);
    }
    insert_run_event(&conn, &run_id, "cancelled", "{}")?;
    let _ = app.emit("chat-event", ChatEvent {
            kind: "run_status".into(),
            group_id: info.group_id,
            run_id: Some(run_id),
            message_id: info.output_message_id,
            delta: None,
            status: Some("cancelled".into()),
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
    Ok(())
}

#[tauri::command]
pub async fn retry_run(
    run_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<String> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let Some((new_id, group_id)) = crate::db::retry_run(&conn, &run_id)? else {
        return Err("找不到任务。".to_string());
    };
    drop(conn);
    schedule_group(to_scheduler(&state, &app), group_id);
    Ok(new_id)
}

#[tauri::command]
pub async fn detect_agent(member_id: String, state: State<'_, AppState>) -> AppResult<String> {
    let conn = open_db(&state.db_path)?;
    let record = conn
        .query_row(
            "SELECT p.adapter,COALESCE(p.executable_path,''),m.group_id FROM agent_profiles p JOIN members m ON m.id=p.member_id WHERE p.member_id=?1",
            params![member_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "找不到 Agent 配置。".to_string())?;
    let kind = AdapterKind::parse(&record.0)?;
    if kind == AdapterKind::Mock {
        conn.execute(
            "UPDATE agent_profiles SET runtime_status='ready',updated_at=?1 WHERE member_id=?2",
            params![now(), member_id],
        )
        .map_err(|e| e.to_string())?;
        return Ok("ready".into());
    }
    let configured = if record.1.trim().is_empty() {
        None
    } else {
        Some(record.1.as_str())
    };
    let executable = kind.resolve_executable(configured)?;
    let status = match tokio::time::timeout(
        Duration::from_secs(5),
        Command::new(&executable).arg("--version").output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => "ready",
        _ => "unavailable",
    };
    conn.execute(
        "UPDATE agent_profiles SET runtime_status=?1,updated_at=?2 WHERE member_id=?3",
        params![status, now(), member_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(status.into())
}
 
 #[tauri::command]
pub fn ocr_image(image_path: String) -> AppResult<String> {
    crate::ocr::ocr_image(&image_path)
}

/// OCR from a base64-encoded image (e.g. clipboard paste)
#[tauri::command]
pub fn ocr_image_base64(base64_data: String) -> AppResult<String> {
    crate::ocr::ocr_image_base64(&base64_data)
}

#[tauri::command]
pub fn get_preset_roles_command(state: State<'_, AppState>) -> AppResult<Vec<PresetRole>> {
    get_preset_roles(&open_db(&state.db_path)?)
}
 
 // === PM: Roadmap Items ===
 
 #[tauri::command]
 pub fn list_roadmap_items(group_id: String, state: State<'_, AppState>) -> AppResult<Vec<RoadmapItem>> {
     get_roadmap_items(&open_db(&state.db_path)?, &group_id)
 }
 
 #[tauri::command]
 pub fn create_roadmap_item(input: CreateRoadmapItemInput, state: State<'_, AppState>) -> AppResult<RoadmapItem> {
     create_roadmap_item_db(&open_db(&state.db_path)?, &input)
 }
 
 #[tauri::command]
 pub fn update_roadmap_item(id: String, input: UpdateRoadmapItemInput, state: State<'_, AppState>) -> AppResult<RoadmapItem> {
     update_roadmap_item_db(&open_db(&state.db_path)?, &id, &input)
 }
 
 #[tauri::command]
 pub fn delete_roadmap_item(id: String, state: State<'_, AppState>) -> AppResult<()> {
     delete_roadmap_item_db(&open_db(&state.db_path)?, &id)
 }
 
 // === PM: Features ===
 
 #[tauri::command]
 pub fn list_features(group_id: String, state: State<'_, AppState>) -> AppResult<Vec<Feature>> {
     get_features(&open_db(&state.db_path)?, &group_id)
 }
 
 #[tauri::command]
 pub fn create_feature(input: CreateFeatureInput, state: State<'_, AppState>) -> AppResult<Feature> {
     create_feature_db(&open_db(&state.db_path)?, &input)
 }
 
 #[tauri::command]
 pub fn update_feature(id: String, input: UpdateFeatureInput, state: State<'_, AppState>) -> AppResult<Feature> {
     update_feature_db(&open_db(&state.db_path)?, &id, &input)
 }
 
 #[tauri::command]
 pub fn delete_feature(id: String, state: State<'_, AppState>) -> AppResult<()> {
     delete_feature_db(&open_db(&state.db_path)?, &id)
 }
 
 // === PM: Feature Tasks ===
 
 #[tauri::command]
 pub fn list_feature_tasks(feature_id: String, state: State<'_, AppState>) -> AppResult<Vec<FeatureTask>> {
     get_feature_tasks(&open_db(&state.db_path)?, &feature_id)
 }
 
 #[tauri::command]
 pub fn create_feature_task(input: CreateFeatureTaskInput, state: State<'_, AppState>) -> AppResult<FeatureTask> {
     create_feature_task_db(&open_db(&state.db_path)?, &input)
 }
 
 #[tauri::command]
 pub fn update_feature_task(id: String, input: UpdateFeatureTaskInput, state: State<'_, AppState>) -> AppResult<FeatureTask> {
     update_feature_task_db(&open_db(&state.db_path)?, &id, &input)
 }
 
 #[tauri::command]
 pub fn delete_feature_task(id: String, state: State<'_, AppState>) -> AppResult<()> {
     delete_feature_task_db(&open_db(&state.db_path)?, &id)
 }
 
// === PM: Aggregated State ===

#[tauri::command]
pub fn get_roadmap_state(group_id: String, state: State<'_, AppState>) -> AppResult<RoadmapState> {
    get_roadmap_state_db(&open_db(&state.db_path)?, &group_id)
}

// === Shared Memory: Experiences ===

#[tauri::command]
pub fn save_experience(
    group_id: String,
    source_member_id: String,
    title: String,
    content: String,
    tags: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<String> {
    save_experience_db(
        &open_db(&state.db_path)?,
        &group_id,
        &source_member_id,
        &title,
        &content,
        tags.as_deref().unwrap_or(""),
    )
}

#[tauri::command]
pub fn query_experiences(
    group_id: String,
    query: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<Vec<Experience>> {
    query_experiences_db(
        &open_db(&state.db_path)?,
        &group_id,
        query.as_deref().unwrap_or(""),
        limit.unwrap_or(20),
    )
}

#[tauri::command]
pub fn delete_experience(id: String, state: State<'_, AppState>) -> AppResult<bool> {
    delete_experience_db(&open_db(&state.db_path)?, &id)
}

// === Logs ===

#[tauri::command]
pub fn list_logs(
    limit: Option<i64>,
    offset: Option<i64>,
    level: Option<String>,
    source: Option<String>,
    since: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<Vec<LogEntry>> {
    logger::query_logs(&open_db(&state.db_path)?, &LogQuery { limit, offset, level, source, since })
}

#[tauri::command]
pub fn count_logs(level: Option<String>, state: State<'_, AppState>) -> AppResult<i64> {
    logger::count_logs(&open_db(&state.db_path)?, level.as_deref())
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppState>) -> AppResult<()> {
    let conn = open_db(&state.db_path)?;
    logger::clear_logs(&conn)?;
    logger::info(&conn, "logs", "logs cleared by user", None);
    Ok(())
}

#[tauri::command]
pub fn list_server_dir(path: String) -> AppResult<crate::fs_browse::DirListing> {
    crate::fs_browse::list_server_dir(&path)
}

#[tauri::command]
pub fn create_server_dir(parent: String, name: String) -> AppResult<String> {
    crate::fs_browse::create_server_dir(&parent, &name)
        .map(|p| p.to_string_lossy().into_owned())
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
pub async fn ops_release_status() -> crate::ops::ReleaseStatus {
    crate::ops::release_status().await
}

#[tauri::command]
pub fn ops_job_status() -> crate::ops::OpsJobState {
    crate::ops::ops_job_snapshot()
}

#[tauri::command]
pub fn ops_run_test_gate() -> AppResult<()> {
    crate::ops::kickoff_test_gate()
}

#[tauri::command]
pub fn ops_deploy_canary() -> AppResult<()> {
    crate::ops::kickoff_deploy_canary()
}

fn normalize_group_kind(raw: Option<&str>) -> String {
    match raw.map(str::trim).unwrap_or("project") {
        "chat" => "chat".into(),
        _ => "project".into(),
    }
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
#[tauri::command]
pub async fn set_run_review(
    run_id: String,
    decision: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<()> {
    let review = match decision.as_str() {
        "approved" | "rejected" => decision.as_str(),
        _ => return Err("decision 仅支持 approved / rejected".to_string()),
    };
    let status = if review == "approved" { "completed" } else { "changes_requested" };
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let run: TaskRun = conn
        .query_row(
            "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at FROM task_runs WHERE id=?1",
            params![run_id],
            run_from_row,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "找不到任务。".to_string())?;
    if run.status != "awaiting_review" {
        return Err(format!("任务 {} 当前状态 {}，不在待审批状态", run_id, run.status));
    }
    if !crate::db::set_run_review(&conn, &run_id, review, status)? {
        return Err("该任务不在待审批状态（可能已被处理）".to_string());
    }
    insert_run_event(&conn, &run_id, &format!("review_{}", review), &format!(r#"{{"reviewer":"human","decision":"{}"}}"#, review))?;
    let _ = app.emit("chat-event", ChatEvent {
        kind: "run_status".into(),
        group_id: run.group_id,
        run_id: Some(run_id),
        message_id: run.output_message_id,
        delta: None,
        status: Some(status.into()),
        error: None,
        channel: None,
        replace: None,
        phase: None,
        elapsed_ms: None,
        total_ms: None,
        seq: None,
        delta_count: None,
        rss_mib: None,
    });
    Ok(())
}

fn build_feedback(res: (i64, i64, Option<String>)) -> crate::models::MessageFeedback {
    crate::models::MessageFeedback { up: res.0, down: res.1, my_vote: res.2 }
}

/// 消息反馈：vote 为 "up"/"down" 时覆盖当前成员表决，为 None 时清除；返回聚合计数。
#[tauri::command]
pub async fn vote_message(
    message_id: String,
    member_id: String,
    vote: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<crate::models::MessageFeedback> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let res = crate::db::vote_message(&conn, &message_id, &member_id, vote.as_deref())?;
    Ok(build_feedback(res))
}

/// 读取某个消息的反馈聚合。
#[tauri::command]
pub async fn get_message_feedback(
    message_id: String,
    member_id: String,
    state: State<'_, AppState>,
) -> AppResult<crate::models::MessageFeedback> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let res = crate::db::get_message_feedback(&conn, &message_id, &member_id)?;
    Ok(build_feedback(res))
}

/// 读取 run 的阶段轨迹（时间线，P2 run 轨迹视图）。
#[tauri::command]
pub async fn get_run_phases(
    run_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<crate::models::RunPhaseEntry>> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    crate::db::list_run_phases(&conn, &run_id)
}
