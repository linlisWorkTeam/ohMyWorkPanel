//! 成员 IPC 命令（P4 入口瘦身）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use super::*;

#[tauri::command]
pub fn add_member(input: AddMemberInput, state: State<'_, AppState>) -> AppResult<Member> {
    if !matches!(input.kind.as_str(), "user" | "agent" | "chatbot")
        || input.display_name.trim().is_empty()
    {
        return Err("成员类型或名称无效。".into());
    }
    let invite_mode = input.invite.unwrap_or(false);
    if invite_mode && input.kind != "user" {
        return Err("仅用户成员支持邀请链接。".into());
    }
    let requested_adapter = input.adapter.as_deref().unwrap_or("mock");
    let connecter_profile = if input.kind == "agent"
        && crate::agents::adapters::connecter_remote::is_connecter_remote(requested_adapter)
    {
        Some(
            crate::agents::adapters::connecter_remote::validate_profile_input(
                input.connecter_base_url.as_deref(),
                input.connecter_bearer.as_deref(),
                input.connecter_group_ref.as_deref(),
                input.connecter_target_subject_id.as_deref(),
                input.connecter_env.as_deref(),
            )?,
        )
    } else {
        None
    };
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
    let color = input
        .avatar_color
        .unwrap_or_else(|| match input.kind.as_str() {
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
        if !crate::agents::adapters::connecter_remote::is_connecter_remote(&adapter) {
            crate::adapters::manifest::resolve_adapter(&adapter)?;
        }
        let default_ws = if group.workspace_path.trim().is_empty() {
            None
        } else {
            let _ = crate::memory::ensure_ohmyworkpanel_layout(
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
        if let Some(profile) = connecter_profile {
            crate::db::insert_connecter_provider_profile(
                &conn,
                &crate::db::ConnecterProviderProfile {
                    member_id: member_id.clone(),
                    base_url: profile.base_url,
                    bearer_token: profile.bearer_token,
                    group_ref: profile.group_ref,
                    target_subject_id: profile.target_subject_id,
                    env: profile.env,
                },
            )?;
        }
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
            .unwrap_or("deepseek-chat");
        let api_url = input
            .api_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        crate::adapters::chatbot::validate_api_url(api_url.as_deref())?;
        conn.execute(
            "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at,api_key,warm_status,model,api_url) VALUES(?1,?2,NULL,'ready',?3,?4,'cold',?5,?6)",
            params![member_id, adapter, created_at, crate::db::encrypt_secret(api_key)?, model, api_url],
        )
        .map_err(|e| e.to_string())?;
    }
    if invite_mode {
        let _ = crate::db::create_group_invite(&conn, &input.group_id, &member_id, None)?;
    }
    crate::db::get_member(&conn, &member_id)
}

#[tauri::command]
pub fn list_joinable_users(
    group_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<crate::models::JoinableUser>> {
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
    let _ = app.emit(
        "chat-event",
        ChatEvent {
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
            .query_row("SELECT kind FROM members WHERE id=?1", params![id], |r| {
                r.get(0)
            })
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
