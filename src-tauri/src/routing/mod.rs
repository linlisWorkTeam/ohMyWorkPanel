//! Leader Agent 委派失败路由（SDD change 0003）。
//!
//! 分类 → 重试 / 升级 / 停止。只依赖既有 task_runs / messages / run_events，不改 schema。

use rusqlite::{params, OptionalExtension};

use crate::db::{
    create_task_run, get_group, get_members, get_settings_from, id, insert_run_event, now, open_db,
};
use crate::scheduler::SchedulerState;

/// 默认最多重试次数（首败后重派一次）。
pub const MAX_RETRIES: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    Retryable,
    Permanent,
    Cancelled,
    Unknown,
}

impl FailureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            FailureClass::Retryable => "retryable",
            FailureClass::Permanent => "permanent",
            FailureClass::Cancelled => "cancelled",
            FailureClass::Unknown => "unknown",
        }
    }
}

const RETRYABLE_PATTERNS: &[&str] = &[
    "timeout",
    "timed out",
    "connection",
    "reset by peer",
    "broken pipe",
    "unexpected eof",
    "rate limit",
    "too many requests",
    "429",
    "500",
    "502",
    "503",
    "504",
    "overloaded",
    "temporarily",
    "try again",
    "spawn",
    "failed to start",
];

const PERMANENT_PATTERNS: &[&str] = &[
    "401",
    "403",
    "unauthorized",
    "forbidden",
    "api key",
    "credential",
    "login",
    "subscription",
    "quota",
    "billing",
    "unsupported",
    "not installed",
    "command not found",
    "no such file",
    "invalid config",
    "permission denied",
];

const CANCELLED_PATTERNS: &[&str] = &["cancelled", "canceled", "aborted by user"];

/// 按错误文本启发式分类；宁可重试一次，也不把可恢复错误直接升级。
pub fn classify(error: &str) -> FailureClass {
    let text = error.to_lowercase();
    if CANCELLED_PATTERNS
        .iter()
        .any(|pattern| text.contains(pattern))
    {
        return FailureClass::Cancelled;
    }
    // 鉴权/配置类优先于宽泛的「connection」等词。
    if PERMANENT_PATTERNS
        .iter()
        .any(|pattern| text.contains(pattern))
    {
        return FailureClass::Permanent;
    }
    if RETRYABLE_PATTERNS
        .iter()
        .any(|pattern| text.contains(pattern))
    {
        return FailureClass::Retryable;
    }
    FailureClass::Unknown
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAction {
    Retry,
    Escalate,
    Stop,
}

#[derive(Debug, Clone, Copy)]
pub struct RouteInput {
    pub has_parent: bool,
    pub is_reviewer: bool,
    pub is_leader_run: bool,
    pub has_leader: bool,
    pub depth: i64,
    pub max_depth: i64,
    /// 已包含本次失败的失败次数。
    pub attempts: u32,
    pub max_retries: u32,
    pub class: FailureClass,
}

pub fn decide(input: &RouteInput) -> RouteAction {
    if !input.has_parent || input.is_reviewer || input.is_leader_run {
        return RouteAction::Stop;
    }
    if input.depth >= input.max_depth {
        return RouteAction::Stop;
    }
    if input.class == FailureClass::Cancelled {
        return RouteAction::Stop;
    }
    if input.class == FailureClass::Permanent {
        return if input.has_leader {
            RouteAction::Escalate
        } else {
            RouteAction::Stop
        };
    }
    if input.attempts <= input.max_retries {
        RouteAction::Retry
    } else if input.has_leader {
        RouteAction::Escalate
    } else {
        RouteAction::Stop
    }
}

#[derive(Debug)]
struct FailedRun {
    group_id: String,
    root_message_id: String,
    agent_member_id: String,
    parent_run_id: Option<String>,
    depth: i64,
    status: String,
    review_status: Option<String>,
    reviewer_member_id: Option<String>,
}

fn load_run(state: &SchedulerState, run_id: &str) -> Option<FailedRun> {
    let conn = open_db(&state.db_path).ok()?;
    conn.query_row(
        "SELECT group_id,root_message_id,agent_member_id,parent_run_id,depth,status,review_status,reviewer_member_id FROM task_runs WHERE id=?1",
        params![run_id],
        |row| {
            Ok(FailedRun {
                group_id: row.get(0)?,
                root_message_id: row.get(1)?,
                agent_member_id: row.get(2)?,
                parent_run_id: row.get(3)?,
                depth: row.get(4)?,
                status: row.get(5)?,
                review_status: row.get(6)?,
                reviewer_member_id: row.get(7)?,
            })
        },
    )
    .optional()
    .ok()
    .flatten()
}

/// 失败路由入口：只创建 run/消息，不调度；返回 (group_id, 新 run ids)。
pub fn route_failed_run(
    state: &SchedulerState,
    run_id: &str,
    error: &str,
) -> Option<(String, Vec<String>)> {
    let run = load_run(state, run_id)?;
    if run.status != "failed" {
        return None;
    }

    let conn = open_db(&state.db_path).ok()?;
    let group = get_group(&conn, &run.group_id).ok()?;
    let members = get_members(&conn, &run.group_id).ok()?;
    let settings = get_settings_from(&conn).ok()?;

    let is_leader_run = group
        .admin_member_id
        .as_deref()
        .map(|leader| leader == run.agent_member_id)
        .unwrap_or(false);
    let has_leader = group.admin_member_id.is_some();
    let is_reviewer =
        run.reviewer_member_id.is_some() || run.review_status.as_deref() == Some("pending");

    let attempts: u32 = match &run.parent_run_id {
        Some(parent) => conn
            .query_row(
                "SELECT COUNT(*) FROM task_runs WHERE parent_run_id=?1 AND agent_member_id=?2 AND status='failed'",
                params![parent, run.agent_member_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0) as u32,
        None => 0,
    };

    let action = decide(&RouteInput {
        has_parent: run.parent_run_id.is_some(),
        is_reviewer,
        is_leader_run,
        has_leader,
        depth: run.depth,
        max_depth: settings.max_delegation_depth,
        attempts: attempts.max(1),
        max_retries: MAX_RETRIES,
        class: classify(error),
    });

    let class = classify(error).as_str();
    match action {
        RouteAction::Stop => None,
        RouteAction::Retry => {
            let retry_id = create_task_run(
                &conn,
                &run.group_id,
                &run.root_message_id,
                &run.agent_member_id,
                run.parent_run_id.as_deref(),
                run.depth,
            )
            .ok()?;
            let payload = serde_json::json!({
                "action": "retry",
                "class": class,
                "attempts": attempts,
                "retryRunId": retry_id,
            })
            .to_string();
            let _ = insert_run_event(&conn, run_id, "route", &payload);
            Some((run.group_id, vec![retry_id]))
        }
        RouteAction::Escalate => {
            let leader = group.admin_member_id.clone()?;
            let target_name = members
                .iter()
                .find(|member| member.id == run.agent_member_id)
                .map(|member| member.display_name.clone())
                .unwrap_or_else(|| run.agent_member_id.clone());
            let summary = truncate_chars(error.trim(), 200);
            let content = format!(
                "[路由升级] 目标 {target_name} 调用失败（{class}）：{summary}\n请重新指派给其他 Agent，或直接接手完成。"
            );
            let escalation_message_id = id();
            conn.execute(
                "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,'completed',?5)",
                params![escalation_message_id, run.group_id, leader, content, now()],
            )
            .ok()?;
            let escalation_run_id = create_task_run(
                &conn,
                &run.group_id,
                &escalation_message_id,
                &leader,
                run.parent_run_id.as_deref(),
                run.depth,
            )
            .ok()?;
            let payload = serde_json::json!({
                "action": "escalate",
                "class": class,
                "attempts": attempts,
                "escalationMessageId": escalation_message_id,
                "escalationRunId": escalation_run_id,
            })
            .to_string();
            let _ = insert_run_event(&conn, run_id, "route", &payload);
            Some((run.group_id, vec![escalation_run_id]))
        }
    }
}

fn truncate_chars(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests;
