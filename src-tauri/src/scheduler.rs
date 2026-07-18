use crate::adapters::{self, AdapterKind};
use crate::db::{
    create_task_run, get_group, get_members, get_settings_from, id, insert_run_event,
    member_from_row, message_from_row, now, open_db, run_from_row, settings_or, AppResult,
};
use crate::models::{ChatEvent, ExecutionContext, TaskRun};
use crate::AppState;
use rusqlite::params;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{AppHandle, Emitter};

pub fn emit(app: &AppHandle, event: ChatEvent) {
    let _ = app.emit("chat-event", event);
}

pub fn schedule_group(state: AppState, app: AppHandle, group_id: String) {
    let inserted = {
        let Ok(mut guard) = state.scheduling_groups.lock() else {
            return;
        };
        guard.insert(group_id.clone())
    };
    if !inserted {
        return;
    }
    let scheduled = (|| -> AppResult<Vec<(String, Option<String>)>> {
        let conn = open_db(&state.db_path)?;
        let settings = get_settings_from(&conn)?;
        let running = conn
            .query_row(
                "SELECT COUNT(*) FROM task_runs WHERE group_id=?1 AND status='running'",
                params![group_id],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?;
        let available = (settings.max_concurrent_runs - running).max(0);
        let mut stmt = conn
            .prepare(
                "SELECT id,agent_member_id FROM task_runs WHERE group_id=?1 AND status='queued' ORDER BY created_at LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let queued = stmt
            .query_map(params![group_id, available], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let mut starts = Vec::new();
        for (run_id, agent_id) in queued {
            let message_id = id();
            conn.execute(
                "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,?4,'','streaming',?5)",
                params![message_id, group_id, agent_id, run_id, now()],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE task_runs SET status='running',output_message_id=?1,started_at=?2 WHERE id=?3 AND status='queued'",
                params![message_id, now(), run_id],
            )
            .map_err(|e| e.to_string())?;
            insert_run_event(&conn, &run_id, "started", "{}")?;
            starts.push((run_id, Some(message_id)));
        }
        Ok(starts)
    })();
    {
        if let Ok(mut guard) = state.scheduling_groups.lock() {
            guard.remove(&group_id);
        }
    }
    match scheduled {
        Ok(starts) => {
            for (run_id, message_id) in starts {
                emit(
                    &app,
                    ChatEvent {
                        kind: "run_status".into(),
                        group_id: group_id.clone(),
                        run_id: Some(run_id.clone()),
                        message_id,
                        status: Some("running".into()),
                        delta: None,
                        error: None,
                    },
                );
                let child_state = state.clone();
                let child_app = app.clone();
                tokio::spawn(async move {
                    execute_run(child_state, child_app, run_id).await;
                });
            }
        }
        Err(error) => emit(
            &app,
            ChatEvent {
                kind: "scheduler_error".into(),
                group_id,
                run_id: None,
                message_id: None,
                delta: None,
                status: None,
                error: Some(error),
            },
        ),
    }
}

fn get_execution_context(state: &AppState, run_id: &str) -> AppResult<ExecutionContext> {
    let conn = open_db(&state.db_path)?;
    let run: TaskRun = conn
        .query_row(
            "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,created_at,started_at,completed_at FROM task_runs WHERE id=?1",
            params![run_id],
            run_from_row,
        )
        .map_err(|e| e.to_string())?;
    let group = get_group(&conn, &run.group_id)?;
    let agent = conn
        .query_row(
            "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,m.created_at FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.id=?1",
            params![run.agent_member_id],
            member_from_row,
        )
        .map_err(|e| e.to_string())?;
    if !agent.is_active {
        return Err("该 Agent 已被移除。".into());
    }
    let mut stmt = conn
        .prepare(
            "SELECT m.id,m.group_id,m.sender_member_id,m.parent_run_id,m.content,m.status,m.created_at FROM messages m WHERE m.group_id=?1 ORDER BY m.created_at DESC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let mut history = stmt
        .query_map(
            params![group.id, settings_or(&conn, "context_message_limit", 40)?],
            message_from_row,
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    history.reverse();
    let members = get_members(&conn, &group.id)?;
    let display: HashMap<_, _> = members
        .iter()
        .map(|m| (m.id.clone(), m.display_name.clone()))
        .collect();
    let lines = history
        .iter()
        .map(|m| {
            format!(
                "{}: {}",
                display
                    .get(&m.sender_member_id)
                    .cloned()
                    .unwrap_or_else(|| "成员".into()),
                m.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let root = history
        .iter()
        .find(|m| m.id == run.root_message_id)
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let prompt = format!(
        "你是群聊中的 Agent「{}」。职责：{}。\n工作目录：{}\n请只完成当前任务，明确说明结果与风险。需要其他 Agent 协作时，仅在你是管理员时使用 @成员名 提及。\n任务根消息：{}\n最近群聊：\n{}",
        agent.display_name, agent.role_description, group.workspace_path, root, lines
    );
    Ok(ExecutionContext {
        run,
        group,
        agent,
        prompt,
        settings: get_settings_from(&conn)?,
    })
}

async fn execute_run(state: AppState, app: AppHandle, run_id: String) {
    let context = match get_execution_context(&state, &run_id) {
        Ok(v) => v,
        Err(error) => {
            finish_failed(&state, &app, &run_id, &error).await;
            return;
        }
    };
    let token = Arc::new(AtomicBool::new(false));
    if let Ok(mut tokens) = state.cancellations.lock() {
        tokens.insert(run_id.clone(), token.clone());
    }
    let outcome = run_agent(&state, &app, &context, &token).await;
    if token.load(Ordering::SeqCst) {
    } else if let Err(error) = outcome {
        finish_failed(&state, &app, &run_id, &error).await;
    } else {
        finish_completed(&state, &app, &context).await;
    }
    if let Ok(mut tokens) = state.cancellations.lock() {
        tokens.remove(&run_id);
    }
    schedule_group(state, app, context.group.id);
}

async fn run_agent(
    state: &AppState,
    app: &AppHandle,
    context: &ExecutionContext,
    token: &Arc<AtomicBool>,
) -> AppResult<()> {
    let adapter_name = context.agent.adapter.as_deref().unwrap_or("mock");
    let kind = AdapterKind::parse(adapter_name)?;
    let on_delta = |delta: String| {
        let state = state.clone();
        let app = app.clone();
        let run = context.run.clone();
        async move { append_delta(&state, &app, &run, &delta) }
    };

    if kind == AdapterKind::Mock {
        return adapters::run_mock_stream(token, on_delta).await;
    }

    let executable = kind.resolve_executable(context.agent.executable_path.as_deref())?;
    adapters::run_streaming(
        kind,
        &executable,
        std::path::Path::new(&context.group.workspace_path),
        &context.prompt,
        context.settings.run_timeout_seconds as u64,
        token,
        on_delta,
    )
    .await
}

fn append_delta(
    state: &AppState,
    app: &AppHandle,
    run: &TaskRun,
    delta: &str,
) -> AppResult<()> {
    let conn = open_db(&state.db_path)?;
    let output_id = run
        .output_message_id
        .as_ref()
        .ok_or_else(|| "任务缺少输出消息。".to_string())?;
    let changed = conn
        .execute(
            "UPDATE messages SET content=content || ?1 WHERE id=?2 AND status='streaming'",
            params![delta, output_id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Ok(());
    }
    insert_run_event(&conn, &run.id, "delta", delta)?;
    emit(
        app,
        ChatEvent {
            kind: "message_delta".into(),
            group_id: run.group_id.clone(),
            run_id: Some(run.id.clone()),
            message_id: Some(output_id.clone()),
            delta: Some(delta.into()),
            status: Some("streaming".into()),
            error: None,
        },
    );
    Ok(())
}

async fn finish_failed(state: &AppState, app: &AppHandle, run_id: &str, error: &str) {
    let result = (|| -> AppResult<(String, Option<String>)> {
        let conn = open_db(&state.db_path)?;
        let run: TaskRun = conn
            .query_row(
                "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,created_at,started_at,completed_at FROM task_runs WHERE id=?1",
                params![run_id],
                run_from_row,
            )
            .map_err(|e| e.to_string())?;
        let changed = conn
            .execute(
                "UPDATE task_runs SET status='failed',error_message=?1,completed_at=?2 WHERE id=?3 AND status='running'",
                params![error, now(), run_id],
            )
            .map_err(|e| e.to_string())?;
        if changed > 0 {
            if let Some(message_id) = &run.output_message_id {
                conn.execute(
                    "UPDATE messages SET status='failed' WHERE id=?1",
                    params![message_id],
                )
                .map_err(|e| e.to_string())?;
            }
            insert_run_event(&conn, run_id, "failed", error)?;
        }
        Ok((run.group_id, run.output_message_id))
    })();
    if let Ok((group_id, message_id)) = result {
        emit(
            app,
            ChatEvent {
                kind: "run_status".into(),
                group_id,
                run_id: Some(run_id.into()),
                message_id,
                delta: None,
                status: Some("failed".into()),
                error: Some(error.into()),
            },
        );
    }
}

async fn finish_completed(state: &AppState, app: &AppHandle, context: &ExecutionContext) {
    let result = (|| -> AppResult<Option<String>> {
        let conn = open_db(&state.db_path)?;
        let changed = conn
            .execute(
                "UPDATE task_runs SET status='completed',completed_at=?1 WHERE id=?2 AND status='running'",
                params![now(), context.run.id],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            return Ok(None);
        }
        let output_id = context
            .run
            .output_message_id
            .as_ref()
            .ok_or_else(|| "任务缺少输出消息。".to_string())?;
        conn.execute(
            "UPDATE messages SET content=CASE WHEN length(content)=0 THEN '已完成。' ELSE content END,status='completed' WHERE id=?1",
            params![output_id],
        )
        .map_err(|e| e.to_string())?;
        insert_run_event(&conn, &context.run.id, "completed", "{}")?;
        Ok(Some(output_id.clone()))
    })();
    if let Ok(Some(message_id)) = result {
        emit(
            app,
            ChatEvent {
                kind: "run_status".into(),
                group_id: context.group.id.clone(),
                run_id: Some(context.run.id.clone()),
                message_id: Some(message_id.clone()),
                delta: None,
                status: Some("completed".into()),
                error: None,
            },
        );
        delegate_from_admin(state, app, context, &message_id).await;
    }
}

async fn delegate_from_admin(
    state: &AppState,
    app: &AppHandle,
    context: &ExecutionContext,
    output_message_id: &str,
) {
    let created = (|| -> AppResult<Vec<String>> {
        let conn = open_db(&state.db_path)?;
        let group = get_group(&conn, &context.group.id)?;
        if group.admin_member_id.as_deref() != Some(context.agent.id.as_str())
            || context.run.depth >= context.settings.max_delegation_depth
        {
            return Ok(vec![]);
        }
        let content = conn
            .query_row(
                "SELECT content FROM messages WHERE id=?1",
                params![output_message_id],
                |r| r.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())?;
        let members = get_members(&conn, &group.id)?;
        let target_ids = members
            .iter()
            .filter(|m| {
                m.kind == "agent"
                    && m.is_active
                    && m.id != context.agent.id
                    && content.contains(&format!("@{}", m.display_name))
            })
            .map(|m| m.id.clone())
            .collect::<Vec<_>>();
        let mut runs = Vec::new();
        for target in target_ids {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO mentions(message_id,member_id) VALUES(?1,?2)",
                params![output_message_id, target],
            );
            runs.push(create_task_run(
                &conn,
                &group.id,
                output_message_id,
                &target,
                Some(&context.run.id),
                context.run.depth + 1,
            )?);
        }
        Ok(runs)
    })();
    if let Ok(run_ids) = created {
        if !run_ids.is_empty() {
            schedule_group(state.clone(), app.clone(), context.group.id.clone());
        }
    }
}
