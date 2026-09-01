use crate::adapters::{self, chatbot, AdapterKind};
use crate::db::{
    create_task_run, get_agent_api_key, get_chat_context_summary, get_cli_session_id, get_group,
    get_members, get_messages_after_created_at, get_settings_from, id, insert_run_event,
    member_from_row, message_from_row, now, open_db, run_from_row, set_cli_session_id, set_run_phase,
    upsert_chat_context_summary, AppResult,
};
use crate::event_sender::EventSender;
use crate::logger;
use crate::memory;
use crate::message_content::{apply_channel_delta, parts_to_plain_text};
use crate::models::{ChatEvent, ExecutionContext, Experience, TaskRun};
use crate::orchestrator;
#[cfg(feature = "gui")]
use tauri::Emitter;
use rusqlite::{params, OptionalExtension};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use std::path::PathBuf;

/// Common scheduler state available to both Tauri and Web modes
#[derive(Clone, Debug)]
pub struct SchedulerState {
    pub db_path: PathBuf,
    pub event_sender: EventSender,
    pub cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    pub scheduling_groups: Arc<Mutex<std::collections::HashSet<String>>>,
    /// In-memory Live sessions: group_id → started_at (ms). Cleared on process restart.
    pub live_sessions: Arc<Mutex<HashMap<String, i64>>>,
}

impl SchedulerState {
    pub fn is_live_active(&self, group_id: &str) -> bool {
        self.live_sessions
            .lock()
            .map(|m| m.contains_key(group_id))
            .unwrap_or(false)
    }

    pub fn mark_live_started(&self, group_id: &str) {
        if let Ok(mut m) = self.live_sessions.lock() {
            m.insert(group_id.to_string(), now());
        }
    }

    pub fn mark_live_stopped(&self, group_id: &str) {
        if let Ok(mut m) = self.live_sessions.lock() {
            m.remove(group_id);
        }
    }
}

static EVENT_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn emit(state: &SchedulerState, mut event: ChatEvent) {
    event.seq = Some(EVENT_SEQ.fetch_add(1, Ordering::Relaxed));
    // Tauri mode
    #[cfg(feature = "gui")]
    if let EventSender::Tauri(app) = &state.event_sender {
        let _ = app.emit("chat-event", &event);
        return;
    }
    // Web mode: serialize full ChatEvent (camelCase + delta) for the browser WS client
    let EventSender::Web(tx) = &state.event_sender else {
        return;
    };
    if let Ok(payload) = serde_json::to_string(&event) {
        let _ = tx.send(payload);
    }
}

fn emit_phase(state: &SchedulerState, group_id: &str, run_id: &str, phase: &str) {
    let Ok(conn) = open_db(&state.db_path) else {
        return;
    };
    let Ok((elapsed, total)) = set_run_phase(&conn, run_id, phase) else {
        return;
    };
    logger::info(
        &conn,
        "run_phase",
        &format!("run={run_id} phase={phase} +{elapsed}ms total={total}ms"),
        None,
    );
    emit(
        state,
        ChatEvent {
            kind: "run_status".into(),
            group_id: group_id.into(),
            run_id: Some(run_id.into()),
            message_id: None,
            delta: None,
            status: None,
            error: None,
            channel: None,
            replace: None,
            phase: Some(phase.into()),
            elapsed_ms: Some(elapsed),
            total_ms: Some(total),
            seq: None,
            delta_count: None,
            rss_mib: None,
        },
    );
}

/// Decide which queued runs may start now (read-only planning).
/// Same agent stays serial; different agents may run in parallel up to `available` slots.
pub(crate) fn plan_queued_starts(
    conn: &rusqlite::Connection,
    group_id: &str,
    available: i64,
) -> AppResult<Vec<(String, String)>> {
    if available <= 0 {
        return Ok(Vec::new());
    }
    if crate::release_drain::is_enabled(conn)? {
        return Ok(Vec::new());
    }
    let mut busy_agents: HashSet<String> = HashSet::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT agent_member_id FROM task_runs WHERE group_id=?1 AND status='running'",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![group_id], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for row in rows {
            busy_agents.insert(row.map_err(|e| e.to_string())?);
        }
    }
    let mut stmt = conn
        .prepare(
            "SELECT id,agent_member_id FROM task_runs WHERE group_id=?1 AND status='queued' ORDER BY created_at",
        )
        .map_err(|e| e.to_string())?;
    let queued = stmt
        .query_map(params![group_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut starts = Vec::new();
    for (run_id, agent_id) in queued {
        if starts.len() as i64 >= available {
            break;
        }
        if busy_agents.contains(&agent_id) {
            continue;
        }
        busy_agents.insert(agent_id.clone());
        starts.push((run_id, agent_id));
    }
    Ok(starts)
}

pub fn schedule_group(state: SchedulerState, group_id: String) {
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
        let planned = plan_queued_starts(&conn, &group_id, available)?;
        let mut starts = Vec::new();
        for (run_id, agent_id) in planned {
            let message_id = id();
            conn.execute(
                "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,?4,'','streaming',?5)",
                params![message_id, group_id, agent_id, run_id, now()],
            )
            .map_err(|e| e.to_string())?;
            let ts = now();
            conn.execute(
                "UPDATE task_runs SET status='running',output_message_id=?1,started_at=?2,phase='starting',phase_updated_at=?2 WHERE id=?3 AND status='queued'",
                params![message_id, ts, run_id],
            )
            .map_err(|e| e.to_string())?;
            insert_run_event(&conn, &run_id, "started", "{}")?;
            let _ = set_run_phase(&conn, &run_id, "starting");
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
                    &state,
                    ChatEvent {
                        kind: "run_status".into(),
                        group_id: group_id.clone(),
                        run_id: Some(run_id.clone()),
                        message_id,
                        status: Some("running".into()),
                        delta: None,
                        error: None,
                        channel: None,
                        replace: None,
                        phase: Some("starting".into()),
                        elapsed_ms: None,
                        total_ms: None,
            seq: None,
            delta_count: None,
            rss_mib: None,
        },
                );
                let child_state = state.clone();
                tokio::spawn(async move {
                    execute_run(child_state, run_id).await;
                });
            }
        }
        Err(error) => emit(
            &state,
            ChatEvent {
                kind: "scheduler_error".into(),
                group_id,
                run_id: None,
                message_id: None,
                delta: None,
                status: None,
                error: Some(error),
                channel: None,
                replace: None,
                        phase: None,
                        elapsed_ms: None,
                        total_ms: None,
            seq: None,
            delta_count: None,
            rss_mib: None,
        },
        ),
    }
}

fn get_execution_context(state: &SchedulerState, run_id: &str) -> AppResult<ExecutionContext> {
    let conn = open_db(&state.db_path)?;
    let run: TaskRun = conn
        .query_row(
            "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at FROM task_runs WHERE id=?1",
            params![run_id],
            run_from_row,
        )
        .map_err(|e| e.to_string())?;
    let group = get_group(&conn, &run.group_id)?;
    let agent = conn
        .query_row(
            "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,COALESCE(m.tags,''),m.created_at,p.workspace_path,p.api_key,COALESCE(p.keep_alive,0),p.warm_status,p.model,m.auth_user_id FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.id=?1",
            params![run.agent_member_id],
            member_from_row,
        )
        .map_err(|e| e.to_string())?;
    if !agent.is_active {
        return Err("该 Agent 已被移除。".into());
    }
    let settings = get_settings_from(&conn)?;
    let history_limit = crate::context_policy::effective_context_message_limit(
        &group.group_kind,
        &agent.kind,
        settings.context_message_limit,
        settings.chat_context_message_limit,
    );
    let max_history_chars = crate::context_policy::effective_history_char_budget(
        &group.group_kind,
        &agent.kind,
    );
    let mut stmt = conn
        .prepare(
            "SELECT m.id,m.group_id,m.sender_member_id,m.parent_run_id,m.content,m.status,m.created_at FROM messages m WHERE m.group_id=?1 ORDER BY m.created_at DESC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let mut history = stmt
        .query_map(params![group.id, history_limit], message_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    history.reverse();
    let members = get_members(&conn, &group.id)?;
    let display: HashMap<_, _> = members
        .iter()
        .map(|m| (m.id.clone(), m.display_name.clone()))
        .collect();
    // Cap history to keep argv / HTTP body under budget (chat/chatbot tighter).
    let mut line_parts: Vec<String> = history
        .iter()
        .map(|m| {
            crate::context_policy::format_history_line(
                display
                    .get(&m.sender_member_id)
                    .map(String::as_str)
                    .unwrap_or("成员"),
                &truncate_chars(&parts_to_plain_text(&m.content), 2_000),
                m.created_at,
            )
        })
        .collect();
    let mut lines = line_parts.join("\n");
    while lines.len() > max_history_chars && line_parts.len() > 1 {
        line_parts.remove(0);
        lines = line_parts.join("\n");
    }
    if lines.chars().count() > max_history_chars {
        lines = format!(
            "…(前文已截断)\n{}",
            truncate_chars_end(&lines, max_history_chars)
        );
    }
    let root_raw = history
        .iter()
        .find(|m| m.id == run.root_message_id)
        .map(|m| parts_to_plain_text(&m.content))
        .or_else(|| {
            conn.query_row(
                "SELECT content FROM messages WHERE id=?1",
                params![run.root_message_id],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(|content| parts_to_plain_text(&content))
        })
        .unwrap_or_default();
    // Self-Marketing keeps the visible root message compact and expands its frozen,
    // evidence-bounded prompt only inside the scheduler.
    let root = crate::marketing::expand_internal_prompt(&conn, &root_raw)
        .unwrap_or_else(|| truncate_chars(&root_raw, 8_000));

    // G3: Inject relevant past experiences from shared memory
    let experiences = (|| -> AppResult<Vec<Experience>> {
        let query_text = root.split_whitespace().take(10).collect::<Vec<_>>().join(" ");
        if query_text.is_empty() { return Ok(vec![]); }
        let mut stmt = conn.prepare(
            "SELECT id,group_id,source_member_id,title,content,tags,created_at,updated_at FROM experiences WHERE group_id=?1 AND (content LIKE ?2 OR title LIKE ?2 OR tags LIKE ?2) ORDER BY created_at DESC LIMIT 5"
        ).map_err(|e| e.to_string())?;
        let pattern = format!("%{}%", query_text);
        let rows = stmt.query_map(params![group.id, pattern], |r| {
            Ok(Experience {
                id: r.get(0)?,
                group_id: r.get(1)?,
                source_member_id: r.get(2)?,
                title: r.get(3)?,
                content: r.get(4)?,
                tags: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        }).map_err(|e| e.to_string())?;
        let mut results = Vec::new();
        for row in rows { results.push(row.map_err(|e| e.to_string())?); }
        Ok(results)
    })().unwrap_or_default();

    let experience_block = if experiences.is_empty() {
        String::new()
    } else {
        let entries: Vec<String> = experiences.iter().map(|e|
            format!("- ({}) {}: {}", e.title, e.tags, &e.content[..e.content.len().min(200)])
        ).collect();
        format!("\n相关经验记忆：\n{}", entries.join("\n"))
    };

    // G2: Inject review context if this is a review task
    let review_block = if let Some(ref parent_id) = run.parent_run_id {
        (|| -> Option<String> {
            let conn = open_db(&state.db_path).ok()?;
            let parent: TaskRun = conn.query_row(
                "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at FROM task_runs WHERE id=?1",
                params![parent_id],
                run_from_row,
            ).ok()?;
            if parent.review_status.as_deref() == Some("pending") && parent.reviewer_member_id.as_deref() == Some(&agent.id) {
                let parent_output = parent.output_message_id.as_ref().and_then(|mid| {
                    conn.query_row("SELECT content FROM messages WHERE id=?1", params![mid], |r| r.get::<_,String>(0)).ok()
                }).unwrap_or_default();
                let parent_output = parts_to_plain_text(&parent_output);
                // Get the original agent's display name
                let orig_agent_name: String = conn.query_row(
                    "SELECT display_name FROM members WHERE id=?1", params![parent.agent_member_id], |r| r.get::<_,String>(0)
                ).unwrap_or_else(|_| parent.agent_member_id.clone());
                Some(format!(
                    "\n\n❗ 当前任务是审查 @{} 的工作成果。请仔细审查以下内容，判断是否通过。\n- 如果通过，在回复中包含「**APPROVED**」\n- 如果需要修改，在回复中包含「CHANGES_REQUESTED: <修改建议>」\n\n待审查内容：\n{}",
                    orig_agent_name, parent_output
                ))
            } else { None }
        })()
    } else { None };

    let announcement_block = format_announcement_block(&group.announcement, 4096);
    let epitaph_block = crate::context_seams::epitaph_handoff_block(
        std::path::Path::new(&group.workspace_path),
        1200,
    );
    let live_block = crate::live_prompt::live_prompt_suffix(
        state.is_live_active(&group.id),
        &agent.kind,
        &agent.id,
        group.admin_member_id.as_deref(),
    );
    let mem_excerpt = memory::read_memory_excerpt(
        std::path::Path::new(&group.workspace_path),
        Some(&agent.id),
        1600,
    );
    let memory_block = if mem_excerpt.trim().is_empty() {
        String::new()
    } else {
        format!("\n{mem_excerpt}\n")
    };
    // Shared Wiki pack — same for Cursor/Codex/OpenClaw (fail-open).
    let wiki_block = crate::wiki_context::wiki_context_block(
        &group.name,
        &root,
        &group.announcement,
    );
    let sections: Vec<_> = [
        crate::context_seams::section("announcement", "group.announcement", &announcement_block),
        crate::context_seams::section("epitaph", "docs/epitaph", &epitaph_block),
        crate::context_seams::section("live", "live_prompt", &live_block),
        crate::context_seams::section("memory", ".ohmyworkpanel/memory", &memory_block),
        crate::context_seams::section("wiki", "WorkPanelWiki.retrieve", &wiki_block),
        crate::context_seams::section("experience", "experiences", &experience_block),
    ]
    .into_iter()
    .flatten()
    .collect();
    let context_ledger = crate::context_seams::ledger_prompt_line(&sections);
    let ledger_json = crate::context_seams::ledger_json(&sections);
    let _ = crate::logger::log(
        &conn,
        crate::logger::LogLevel::Info,
        "context_seams",
        if context_ledger.is_empty() {
            "【已注入上下文】（无）"
        } else {
            &context_ledger
        },
        Some(&ledger_json),
    );
    let ledger_suffix = if context_ledger.is_empty() {
        String::new()
    } else {
        format!("\n{context_ledger}")
    };
    let prompt = format!(
        "你是群聊中的 Agent「{}」。职责：{}。\n工作目录：{}{}{}{}{}{}\n请只完成当前任务，明确说明结果与风险。需要其他 Agent 协作时，使用 @成员名 提及。\n任务根消息：{}\n最近群聊：{}\n你可以将重要经验通过 `!保存经验 <标题>: <内容> #标签` 保存。{}{}{}",
        agent.display_name,
        agent.role_description,
        group.workspace_path,
        announcement_block,
        epitaph_block,
        live_block,
        memory_block,
        wiki_block,
        root,
        lines,
        experience_block,
        review_block.as_deref().unwrap_or_default(),
        ledger_suffix
    );
    Ok(ExecutionContext {
        run,
        group,
        agent,
        prompt,
        settings,
        recent_chat: lines,
        root_task: root,
        context_ledger,
    })
}

async fn execute_run(state: SchedulerState, run_id: String) {
    let context = match get_execution_context(&state, &run_id) {
        Ok(v) => {
            emit_phase(&state, &v.group.id, &run_id, "preparing");
            if !v.context_ledger.is_empty() {
                let mut ev = ChatEvent::bare("context_injected", &v.group.id);
                ev.run_id = Some(run_id.clone());
                ev.delta = Some(v.context_ledger.clone());
                emit(&state, ev);
            }
            v
        }
        Err(error) => {
            finish_failed(&state, &run_id, &error).await;
            return;
        }
    };
    let token = Arc::new(AtomicBool::new(false));
    if let Ok(mut tokens) = state.cancellations.lock() {
        tokens.insert(run_id.clone(), token.clone());
    }
    let outcome = run_agent(&state, &context, &token).await;
    if token.load(Ordering::SeqCst) {
    } else if let Err(error) = outcome {
        emit_phase(&state, &context.group.id, &run_id, "failed");
        finish_failed(&state, &run_id, &error).await;
    } else {
        emit_phase(&state, &context.group.id, &run_id, "finalizing");
        finish_completed(&state, &context).await;
        emit_phase(&state, &context.group.id, &run_id, "completed");
    }
    if let Ok(mut tokens) = state.cancellations.lock() {
        tokens.remove(&run_id);
    }
    schedule_group(state, context.group.id);
}

async fn run_agent(
    state: &SchedulerState,
    context: &ExecutionContext,
    token: &Arc<AtomicBool>,
) -> AppResult<()> {
    let adapter_name = context.agent.adapter.as_deref().unwrap_or("mock");

    // Fast path: HTTP chatbot (no CLI / no tools)
    if context.agent.kind == "chatbot" || chatbot::is_chatbot_adapter(adapter_name) {
        emit_phase(state, &context.group.id, &context.run.id, "awaiting_first_token");
        let conn = open_db(&state.db_path)?;
        let api_key = get_agent_api_key(&conn, &context.agent.id)?
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| "聊天机器人未配置 API Key。".to_string())?;
        let root = if context.root_task.trim().is_empty() {
            extract_root_from_prompt(&context.prompt)
        } else {
            context.root_task.clone()
        };
        let ann = truncate_chars(context.group.announcement.trim(), 800);
        let role = truncate_chars(context.agent.role_description.trim(), 4000);
        let live_block = crate::live_prompt::live_prompt_suffix(
            state.is_live_active(&context.group.id),
            &context.agent.kind,
            &context.agent.id,
            context.group.admin_member_id.as_deref(),
        );
        // 人设（role_description）必须进 system：否则 chatbot 只剩空壳昵称，无法做剧本/NPC。
        let system = format!(
            "你是群聊机器人「{}」。不要调用工具、不要改代码/文件。结合下方群聊上下文（最近原文 + 必要时的历史摘要）理解指代与话题；不要编造摘要里没有的事实。{}{}{}",
            context.agent.display_name,
            if role.is_empty() {
                "\n只做轻量对话，回答简洁。".to_string()
            } else {
                format!("\n职责/人设：{role}")
            },
            if ann.is_empty() {
                String::new()
            } else {
                format!("\n群公告：{ann}")
            },
            live_block
        );
        let model = context.agent.model.as_deref();
        let api_url = crate::db::get_agent_api_url(&conn, &context.agent.id)?;
        let keep = context
            .settings
            .chat_context_message_limit
            .clamp(5, 40) as usize;
        let window = build_chatbot_rolling_window(
            state,
            &context.group.id,
            keep,
            adapter_name,
            api_url.as_deref(),
            &api_key,
            model,
            token,
        )
        .await
        .unwrap_or_else(|_| context.recent_chat.clone());
        let messages = chatbot::build_chat_messages(
            &system,
            &window,
            &context.agent.display_name,
            &root,
        );
        // Live short replies: tighter completion budget (TTS < 50 汉字).
        // Non-live: allow longer narrative (e.g. TRPG scene openers).
        let max_tokens = if live_block.is_empty() { 1024 } else { 128 };
        let text = chatbot::run_chatbot_completion(
            adapter_name,
            api_url.as_deref(),
            &api_key,
            &messages,
            max_tokens,
            token,
            model,
        )
        .await
        .map_err(|e| {
            // 诊断事件：失败时把 provider/model/apiUrl 落进 run_events，便于桌面端日志排障
            let _ = insert_run_event(
                &conn,
                &context.run.id,
                "debug_chatbot",
                &serde_json::json!({
                    "provider": adapter_name,
                    "model": model,
                    "apiUrl": api_url,
                    "error": e,
                })
                .to_string(),
            );
            e
        })?;
        emit_phase(state, &context.group.id, &context.run.id, "streaming");
        append_delta(state, &context.run, "final", &text, false)?;
        return Ok(());
    }

    let spec = adapters::manifest::resolve_adapter(adapter_name)?;
    let delta_count = Arc::new(AtomicU64::new(0));
    let hb_stop = Arc::new(AtomicBool::new(false));
    {
        let interval = if context.settings.heartbeat_auto {
            context.settings.heartbeat_focus_seconds.max(1) as u64
        } else {
            context.settings.heartbeat_background_seconds.max(1) as u64
        };
        spawn_run_heartbeat(
            state.clone(),
            context.group.id.clone(),
            context.run.id.clone(),
            interval,
            delta_count.clone(),
            hb_stop.clone(),
        );
    }
    let make_on_delta = || {
        let state_clone = state.clone();
        let run = context.run.clone();
        let group_id = context.group.id.clone();
        let delta_count = delta_count.clone();
        move |channel: String, delta: String, replace: bool| {
            let state = state_clone.clone();
            let run = run.clone();
            let group_id = group_id.clone();
            let delta_count = delta_count.clone();
            async move {
                delta_count.fetch_add(1, Ordering::Relaxed);
                // First token → streaming phase
                if let Ok(conn) = open_db(&state.db_path) {
                    let phase: Option<String> = conn
                        .query_row(
                            "SELECT phase FROM task_runs WHERE id=?1",
                            params![run.id],
                            |r| r.get(0),
                        )
                        .ok()
                        .flatten();
                    if phase.as_deref() != Some("streaming") {
                        drop(conn);
                        emit_phase(&state, &group_id, &run.id, "streaming");
                    }
                }
                append_delta(&state, &run, &channel, &delta, replace)
            }
        }
    };

    if spec.builtin_kind() == Some(AdapterKind::Mock) {
        emit_phase(state, &context.group.id, &context.run.id, "streaming");
        let result = adapters::run_mock_stream(token, make_on_delta()).await;
        hb_stop.store(true, Ordering::Relaxed);
        result?;
        return Ok(());
    }

    emit_phase(state, &context.group.id, &context.run.id, "cli_spawn");
    let executable = spec.resolve_executable(context.agent.executable_path.as_deref())?;
    let mut session_id = if spec.persists_session() {
        let conn = open_db(&state.db_path)?;
        get_cli_session_id(&conn, &context.agent.id)?
    } else {
        None
    };

    let prompt = if spec.builtin_kind() == Some(AdapterKind::Cursor) && session_id.is_some() {
        short_resume_prompt(context)
    } else {
        context.prompt.clone()
    };

    let cwd = memory::resolve_agent_workspace(
        std::path::Path::new(&context.group.workspace_path),
        &context.agent.id,
        context.agent.workspace_path.as_deref(),
        context.group.is_system,
    )?;
    let _ = memory::ensure_ohmyworkpanel_layout(std::path::Path::new(&context.group.workspace_path), Some(&context.agent.id));

    let model = context.agent.model.as_deref();
    // Member profile key is optional; adapters::codex::resolve_api_key also reads
    // process env / ~/.codex/auth.json so Codex works under systemd.
    let codex_key = if spec.builtin_kind() == Some(AdapterKind::Codex) {
        let conn = open_db(&state.db_path)?;
        get_agent_api_key(&conn, &context.agent.id)?.filter(|k| !k.trim().is_empty())
    } else {
        None
    };
    let timeout = spec.timeout_secs(context.settings.run_timeout_seconds as u64);
    emit_phase(state, &context.group.id, &context.run.id, "awaiting_first_token");
    let result = adapters::run_streaming(
        spec.clone(),
        &executable,
        &cwd,
        &prompt,
        session_id.as_deref(),
        model,
        timeout,
        token,
        codex_key.as_deref(),
        make_on_delta(),
    )
    .await;

    let captured = match result {
        Ok(captured) => captured,
        Err(error)
            if spec.builtin_kind() == Some(AdapterKind::Cursor)
                && session_id.is_some()
                && is_resume_failure(&error) =>
        {
            // Invalid session → clear and retry once without resume.
            let conn = open_db(&state.db_path)?;
            set_cli_session_id(&conn, &context.agent.id, None)?;
            session_id = None;
            adapters::run_streaming(
                spec.clone(),
                &executable,
                &cwd,
                &context.prompt,
                None,
                model,
                timeout,
                token,
                None,
                make_on_delta(),
            )
            .await?
        }
        Err(error) => {
            hb_stop.store(true, Ordering::Relaxed);
            return Err(error);
        }
    };
    hb_stop.store(true, Ordering::Relaxed);

    if spec.persists_session() {
        if let Some(id) = captured.or(session_id) {
            let conn = open_db(&state.db_path)?;
            set_cli_session_id(&conn, &context.agent.id, Some(&id))?;
        }
    }
    Ok(())
}

fn spawn_run_heartbeat(
    state: SchedulerState,
    group_id: String,
    run_id: String,
    interval_secs: u64,
    delta_count: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
) {
    let started = std::time::Instant::now();
    tokio::spawn(async move {
        let secs = interval_secs.max(1);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let still_running = open_db(&state.db_path)
                .ok()
                .and_then(|conn| {
                    conn.query_row(
                        "SELECT status FROM task_runs WHERE id=?1",
                        params![run_id],
                        |r| r.get::<_, String>(0),
                    )
                    .ok()
                })
                .map(|s| s == "running" || s == "queued")
                .unwrap_or(false);
            if !still_running {
                break;
            }
            let mut ev = ChatEvent::bare("run_heartbeat", &group_id);
            ev.run_id = Some(run_id.clone());
            ev.status = Some("running".into());
            ev.elapsed_ms = Some(started.elapsed().as_millis() as i64);
            ev.delta_count = Some(delta_count.load(Ordering::Relaxed) as i64);
            ev.rss_mib = crate::metrics::read_rss_mib();
            emit(&state, ev);
        }
    });
}

fn format_announcement_block(announcement: &str, max_chars: usize) -> String {
    let trimmed = announcement.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let body = truncate_chars(trimmed, max_chars);
    format!("\n\n【群公告 / 项目级规则 — 所有 Agent 必须遵守】\n{body}\n")
}

fn short_resume_prompt(context: &ExecutionContext) -> String {
    let task = extract_root_from_prompt(&context.prompt);
    let announcement_block = format_announcement_block(&context.group.announcement, 2048);
    let epitaph_block = crate::context_seams::epitaph_handoff_block(
        std::path::Path::new(&context.group.workspace_path),
        800,
    );
    // Resume sessions keep the Live constraint if the prepared prompt had it.
    let live_block = if context.prompt.contains("【PanelLive") {
        format!("\n\n{}", crate::live_prompt::PANELLIVE_LLM_PROMPT_FALLBACK)
    } else {
        String::new()
    };
    let ledger = if context.context_ledger.is_empty() {
        String::new()
    } else {
        format!("\n{}", context.context_ledger)
    };
    format!(
        "你是群聊中的 Agent「{}」。职责：{}。\n工作目录：{}{}{}{}\n请只完成当前任务，明确说明结果与风险。需要其他 Agent 协作时，使用 @成员名 提及。\n任务根消息：{}\n（续接同一 CLI session，无需重复历史。）\n你可以将重要经验通过 `!保存经验 <标题>: <内容> #标签` 保存。{}",
        context.agent.display_name,
        context.agent.role_description,
        context.group.workspace_path,
        announcement_block,
        epitaph_block,
        live_block,
        task,
        ledger
    )
}

fn extract_root_from_prompt(prompt: &str) -> String {
    const START: &str = "任务根消息：";
    const END: &str = "\n最近群聊：";
    if let Some(start) = prompt.find(START) {
        let rest = &prompt[start + START.len()..];
        if let Some(end) = rest.find(END) {
            return rest[..end].to_string();
        }
        return rest.lines().next().unwrap_or("").to_string();
    }
    "请继续处理群聊中刚 @ 你的最新任务。".into()
}

/// Rolling summary for chatbot: when unsummarized messages exceed `keep_limit`, fold the
/// overflow into `chat_context_summaries` (LLM, extractive fallback), then accumulate recent lines.
async fn build_chatbot_rolling_window(
    state: &SchedulerState,
    group_id: &str,
    keep_limit: usize,
    provider: &str,
    api_url: Option<&str>,
    api_key: &str,
    model: Option<&str>,
    token: &Arc<AtomicBool>,
) -> AppResult<String> {
    use crate::context_policy::{
        build_fold_summary_prompt, compose_window_with_summary, extractive_fold_summary,
        format_history_line, split_rolling_window, CHAT_FOLD_BATCH,
    };

    let conn = open_db(&state.db_path)?;
    let existing = get_chat_context_summary(&conn, group_id)?;
    let after = existing
        .as_ref()
        .map(|s| s.through_created_at)
        .unwrap_or(0);
    let mut summary_text = existing
        .as_ref()
        .map(|s| s.summary_text.clone())
        .unwrap_or_default();
    let load_n = (keep_limit.saturating_add(CHAT_FOLD_BATCH)) as i64;
    let mut msgs = get_messages_after_created_at(&conn, group_id, after, load_n)?;
    msgs.retain(|m| {
        m.status != "streaming" && !parts_to_plain_text(&m.content).trim().is_empty()
    });
    let members = get_members(&conn, group_id)?;
    let display: HashMap<_, _> = members
        .iter()
        .map(|m| (m.id.clone(), m.display_name.clone()))
        .collect();
    let line_of = |m: &crate::models::Message| {
        format_history_line(
            display
                .get(&m.sender_member_id)
                .map(String::as_str)
                .unwrap_or("成员"),
            &truncate_chars(&parts_to_plain_text(&m.content), 2_000),
            m.created_at,
        )
    };

    let (to_fold, keep) = split_rolling_window(&msgs, keep_limit);
    if !to_fold.is_empty() {
        let folded = to_fold.iter().map(line_of).collect::<Vec<_>>().join("\n");
        let prompt = build_fold_summary_prompt(&summary_text, &folded);
        let summary_messages: Vec<serde_json::Value> = vec![
            serde_json::json!({"role":"system","content":"你是会话摘要助手，只输出压缩后的中文摘要正文。"}),
            serde_json::json!({"role":"user","content":prompt}),
        ];
        let new_summary = match chatbot::run_chatbot_completion(
            provider,
            api_url,
            api_key,
            &summary_messages,
            256,
            token,
            model,
        )
        .await
        {
            Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => extractive_fold_summary(&summary_text, &folded, 600),
        };
        let through = to_fold.last().map(|m| m.created_at).unwrap_or(after);
        let conn = open_db(&state.db_path)?;
        upsert_chat_context_summary(&conn, group_id, &new_summary, through)?;
        summary_text = new_summary;
    }

    let mut recent = keep.iter().map(line_of).collect::<Vec<_>>().join("\n");
    let budget = crate::context_policy::effective_history_char_budget("chat", "chatbot");
    if recent.chars().count() > budget {
        recent = format!(
            "…(前文已截断)\n{}",
            truncate_chars_end(&recent, budget)
        );
    }
    Ok(compose_window_with_summary(&summary_text, &recent))
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    let trimmed: String = input.chars().take(max_chars).collect();
    format!("{trimmed}…(截断)")
}

fn truncate_chars_end(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    input.chars().skip(count - max_chars).collect()
}

fn is_resume_failure(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("resume")
        || lower.contains("session")
        || lower.contains("chat")
        || lower.contains("not found")
        || lower.contains("unknown")
        || lower.contains("invalid")
}

fn append_delta(
    state: &SchedulerState,
    run: &TaskRun,
    channel: &str,
    delta: &str,
    replace: bool,
) -> AppResult<()> {
    if delta.is_empty() {
        return Ok(());
    }
    let conn = open_db(&state.db_path)?;
    let output_id = run
        .output_message_id
        .as_ref()
        .ok_or_else(|| "任务缺少输出消息。".to_string())?;
    let current: String = match conn
        .query_row(
            "SELECT content FROM messages WHERE id=?1 AND status='streaming'",
            params![output_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        Some(value) => value,
        None => return Ok(()),
    };
    let next = apply_channel_delta(&current, channel, delta, replace);
    conn.execute(
        "UPDATE messages SET content=?1 WHERE id=?2 AND status='streaming'",
        params![next, output_id],
    )
    .map_err(|e| e.to_string())?;
    let payload = serde_json::json!({ "channel": channel, "delta": delta, "replace": replace }).to_string();
    insert_run_event(&conn, &run.id, "delta", &payload)?;
    // Lazy channels stay in DB only; UI fetches on expand (keeps WS/list payloads small).
    let lazy = crate::message_content::is_lazy_channel(channel);
    emit(
        state,
        ChatEvent {
            kind: "message_delta".into(),
            group_id: run.group_id.clone(),
            run_id: Some(run.id.clone()),
            message_id: Some(output_id.clone()),
            delta: if lazy { None } else { Some(delta.into()) },
            status: Some("streaming".into()),
            error: None,
            channel: Some(crate::message_content::normalize_channel(channel)),
            replace: if lazy { None } else { Some(replace) },
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

async fn finish_failed(state: &SchedulerState, run_id: &str, error: &str) {
    let result = (|| -> AppResult<(String, Option<String>)> {
        let conn = open_db(&state.db_path)?;
        let run: TaskRun = conn
            .query_row(
                "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at FROM task_runs WHERE id=?1",
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
            state,
            ChatEvent {
                kind: "run_status".into(),
                group_id,
                run_id: Some(run_id.into()),
                message_id,
                delta: None,
                status: Some("failed".into()),
                error: Some(error.into()),
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
        if let EventSender::Web(tx) = &state.event_sender {
            orchestrator::on_run_terminal(
                &state.db_path,
                run_id,
                false,
                Some(error),
                tx,
                state.clone(),
            );
        }
        crate::marketing::on_run_terminal(state, run_id, false, Some(error));
    }
}

async fn finish_completed(state: &SchedulerState, context: &ExecutionContext) {
    // G2: Check if this run is a review response for a parent task
    if let Some(ref parent_run_id) = context.run.parent_run_id {
        if let Some(ref _reviewer_id) = context.run.reviewer_member_id {
            // This IS the review run - check if parent needs review result
        } else {
            // Check if parent is awaiting review
            let check_parent_review = (|| -> AppResult<Option<String>> {
                let conn = open_db(&state.db_path)?;
                let parent: TaskRun = conn.query_row(
                    "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at FROM task_runs WHERE id=?1",
                    params![parent_run_id],
                    run_from_row,
                ).map_err(|e| e.to_string())?;
                if parent.review_status.as_deref() == Some("pending") && parent.reviewer_member_id.as_deref() == Some(&context.agent.id) {
                    let output_id = context.run.output_message_id.as_ref().ok_or_else(|| "缺少输出".to_string())?;
                    let output_content: String = conn.query_row(
                        "SELECT content FROM messages WHERE id=?1", params![output_id], |r| r.get::<_,String>(0)
                    ).map_err(|e| e.to_string())?;
                    let approved = parts_to_plain_text(&output_content)
                        .to_uppercase()
                        .contains("APPROVED");
                    let new_status = if approved { "completed" } else { "changes_requested" };
                    let new_review = if approved { "approved" } else { "rejected" };
                    conn.execute(
                        "UPDATE task_runs SET review_status=?1,status=?2 WHERE id=?3",
                        params![new_review, new_status, parent_run_id],
                    ).map_err(|e| e.to_string())?;
                    insert_run_event(&conn, parent_run_id, &format!("review_{}", new_review),
                        &format!(r#"{{"reviewer":"{}","output":"{}"}}"#, context.agent.id, output_content))?;
                    logger::info(&conn, "review", &format!("review {} by {} for run {}", new_review, context.agent.display_name, parent_run_id), None);
                    return Ok(Some(new_status.into()));
                }
                Ok(None)
            })();
            let _ = check_parent_review;
        }
    }
    let result = (|| -> AppResult<Option<(String, bool)>> {
        let conn = open_db(&state.db_path)?;
        let output_id = context
            .run
            .output_message_id
            .as_ref()
            .ok_or_else(|| "任务缺少输出消息。".to_string())?;
        let content_raw: String = conn
            .query_row(
                "SELECT content FROM messages WHERE id=?1",
                params![output_id],
                |r| r.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())?;
        let content = parts_to_plain_text(&content_raw);
        // G2: Check for review request via !review @AgentName
        let members = get_members(&conn, &context.group.id)?;
        let mut reviewer_id = None;
        for m in &members {
            if m.kind == "agent" && m.is_active && m.id != context.agent.id {
                if content.contains(&format!("!review @{}", m.display_name)) {
                    reviewer_id = Some(m.id.clone());
                    break;
                }
            }
        }
        if let Some(ref rev_id) = reviewer_id {
            // Task goes to "awaiting_review" instead of "completed"
            conn.execute(
                "UPDATE task_runs SET status='awaiting_review',review_status='pending',reviewer_member_id=?1,completed_at=?2 WHERE id=?3 AND status='running'",
                params![rev_id, now(), context.run.id],
            ).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE messages SET content=CASE WHEN length(content)=0 THEN '已完成。' ELSE content END,status='completed' WHERE id=?1",
                params![output_id],
            ).map_err(|e| e.to_string())?;
            insert_run_event(&conn, &context.run.id, "awaiting_review",
                &format!(r#"{{"reviewer":"{}"}}"#, rev_id))?;
            // Create review run for the reviewer
            let review_run_id = create_task_run(
                &conn, &context.group.id, output_id, rev_id,
                Some(&context.run.id), context.run.depth + 1,
            )?;
            logger::info(&conn, "review", &format!("review requested by {}: run {} → {}", context.agent.display_name, context.run.id, review_run_id), None);
            return Ok(Some((output_id.clone(), true)));
        }
        // No review: normal completion
        let changed = conn
            .execute(
                "UPDATE task_runs SET status='completed',completed_at=?1 WHERE id=?2 AND status='running'",
                params![now(), context.run.id],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            return Ok(None);
        }
        conn.execute(
            "UPDATE messages SET content=CASE WHEN length(content)=0 THEN '已完成。' ELSE content END,status='completed' WHERE id=?1",
            params![output_id],
        ).map_err(|e| e.to_string())?;
        insert_run_event(&conn, &context.run.id, "completed", "{}")?;
        Ok(Some((output_id.clone(), false)))
    })();
    if let Ok(Some((message_id, is_review))) = result {
        let status = if is_review { "awaiting_review" } else { "completed" };
        emit(
            state,
            ChatEvent {
                kind: "run_status".into(),
                group_id: context.group.id.clone(),
                run_id: Some(context.run.id.clone()),
                message_id: Some(message_id.clone()),
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
        if !is_review {
            if let EventSender::Web(tx) = &state.event_sender {
                orchestrator::on_run_terminal(
                    &state.db_path,
                    &context.run.id,
                    true,
                    None,
                    tx,
                    state.clone(),
                );
            }
            crate::marketing::on_run_terminal(state, &context.run.id, true, None);
        }
        delegate_from_admin(state, context, &message_id).await;
    }
}

async fn delegate_from_admin(
    state: &SchedulerState,
    context: &ExecutionContext,
    output_message_id: &str,
) {
    let created = (|| -> AppResult<Vec<String>> {
        let conn = open_db(&state.db_path)?;
        let group = get_group(&conn, &context.group.id)?;
        // Don't delegate further from review runs
        if context.run.reviewer_member_id.is_some() || context.run.review_status.as_deref() == Some("pending") {
            return Ok(vec![]);
        }
        if context.run.depth >= context.settings.max_delegation_depth {
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
                if m.kind != "agent" || !m.is_active || m.id == context.agent.id {
                    return false;
                }
                // Check for @mention (A2A) or !review @mention (review request)
                content.contains(&format!("@{}", m.display_name))
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
            schedule_group(state.clone(), context.group.id.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::plan_queued_starts;
    use crate::db::{init_db, open_db};
    use rusqlite::params;

    fn seed_group(path: &std::path::Path) {
        init_db(path).unwrap();
        let conn = open_db(path).unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.', 'u',NULL,1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('u','g','user','u','#000','',1,1,'')",
            [],
        )
        .unwrap();
        for (id, name) in [("a", "AgentA"), ("b", "AgentB")] {
            conn.execute(
                "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES(?1,'g','agent',?2,'#000','',1,1,'')",
                params![id, name],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at,cli_session_id) VALUES(?1,'mock',NULL,'unknown',1,NULL)",
                params![id],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO messages VALUES('m','g','u',NULL,'x','completed',1)",
            [],
        )
        .unwrap();
        // A already running; A queued; B queued — only B should start.
        conn.execute(
            "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,depth,status,created_at) VALUES('r-run','g','m','a',0,'running',1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,depth,status,created_at) VALUES('r-a','g','m','a',0,'queued',2)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,depth,status,created_at) VALUES('r-b','g','m','b',0,'queued',3)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn same_agent_stays_serial_other_agent_can_start() {
        let file = tempfile::NamedTempFile::new().unwrap();
        seed_group(file.path());
        let conn = open_db(file.path()).unwrap();
        let starts = plan_queued_starts(&conn, "g", 3).unwrap();
        assert_eq!(starts, vec![("r-b".into(), "b".into())]);
    }

    #[test]
    fn available_zero_starts_nothing() {
        let file = tempfile::NamedTempFile::new().unwrap();
        seed_group(file.path());
        let conn = open_db(file.path()).unwrap();
        let starts = plan_queued_starts(&conn, "g", 0).unwrap();
        assert!(starts.is_empty());
    }

    #[test]
    fn announcement_block_empty_and_present() {
        assert!(super::format_announcement_block("", 100).is_empty());
        let block = super::format_announcement_block("必须跑测试门禁", 100);
        assert!(block.contains("群公告"));
        assert!(block.contains("必须跑测试门禁"));
    }
}
