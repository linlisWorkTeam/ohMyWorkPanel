//! 聊天 / 运行 IPC 命令（P4 入口瘦身）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use super::*;

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
        .query_row(
            "SELECT group_kind FROM groups WHERE id=?1",
            params![&group_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let sender_kind: String = conn
        .query_row(
            "SELECT kind FROM members WHERE id=?1 AND group_id=?2",
            params![&sender_member_id, &group_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if let Some(reply_text) = crate::workflow::try_slash_command(
        &conn,
        &group_id,
        &group_kind,
        &sender_kind,
        &message.content,
    )? {
        let reply = crate::models::Message {
            id: id(),
            group_id: group_id.clone(),
            sender_member_id: group
                .admin_member_id
                .clone()
                .unwrap_or_else(|| sender_member_id.clone()),
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
    let _ = app.emit(
        "chat-event",
        ChatEvent {
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
        let _ = app.emit(
            "chat-event",
            ChatEvent {
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
            },
        );
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
    let remote_cancel = crate::db::get_connecter_cancel_target(&conn, &run_id)?;
    let live_worker = {
        let tokens = state
            .cancellations
            .lock()
            .map_err(|_| "取消锁不可用".to_string())?;
        if let Some(token) = tokens.get(&run_id) {
            token.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    };
    insert_run_event(&conn, &run_id, "cancelled", "{}")?;
    drop(conn);
    if !live_worker {
        if let Some((profile, dispatch_id)) = remote_cancel {
            let _ =
                crate::agents::adapters::connecter_remote::cancel_dispatch(&profile, &dispatch_id)
                    .await;
        }
    }
    let _ = app.emit(
        "chat-event",
        ChatEvent {
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
    let status = if review == "approved" {
        "completed"
    } else {
        "changes_requested"
    };
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
        return Err(format!(
            "任务 {} 当前状态 {}，不在待审批状态",
            run_id, run.status
        ));
    }
    if !crate::db::set_run_review(&conn, &run_id, review, status)? {
        return Err("该任务不在待审批状态（可能已被处理）".to_string());
    }
    insert_run_event(
        &conn,
        &run_id,
        &format!("review_{}", review),
        &format!(r#"{{"reviewer":"human","decision":"{}"}}"#, review),
    )?;
    let _ = app.emit(
        "chat-event",
        ChatEvent {
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
        },
    );
    Ok(())
}

fn build_feedback(res: (i64, i64, Option<String>)) -> crate::models::MessageFeedback {
    crate::models::MessageFeedback {
        up: res.0,
        down: res.1,
        my_vote: res.2,
    }
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
