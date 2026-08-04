//! Roadmap item orchestration: serial checklist dispatch via chat @mentions.

use crate::db::{
    create_task_run, get_feature_tasks, get_features, get_group, get_members, id, now, open_db,
    AppResult,
};
use crate::models::{Feature, FeatureTask, Group, Member, RoadmapOrchestration};
use crate::scheduler::{self, SchedulerState};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use tokio::sync::broadcast;

pub fn resolve_assignee(
    feature: &Feature,
    admin_member_id: Option<&str>,
) -> Result<String, String> {
    if let Some(aid) = feature
        .assignee_member_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Ok(aid.to_string());
    }
    if let Some(admin) = admin_member_id.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(admin.to_string());
    }
    Err(format!(
        "功能「{}」未指定 Agent，且群未设置管理员。",
        feature.title
    ))
}

/// Next undone checklist under roadmap-linked features (features then tasks by sort_order).
pub fn next_checklist<'a>(
    features: &'a [Feature],
    tasks: &'a [FeatureTask],
) -> Option<(&'a Feature, &'a FeatureTask)> {
    let mut feats: Vec<&Feature> = features.iter().collect();
    feats.sort_by_key(|f| (f.sort_order, f.created_at));
    for feature in feats {
        let mut mine: Vec<&FeatureTask> = tasks
            .iter()
            .filter(|t| t.feature_id == feature.id && !t.done)
            .collect();
        mine.sort_by_key(|t| (t.sort_order, t.created_at));
        if let Some(task) = mine.first() {
            return Some((feature, task));
        }
    }
    None
}

pub fn ensure_orchestrations_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS roadmap_orchestrations (
          id TEXT PRIMARY KEY,
          group_id TEXT NOT NULL,
          roadmap_item_id TEXT NOT NULL,
          status TEXT NOT NULL,
          cursor_feature_id TEXT,
          cursor_task_id TEXT,
          current_run_id TEXT,
          error_message TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_orch_group ON roadmap_orchestrations(group_id, updated_at);
        CREATE INDEX IF NOT EXISTS idx_orch_item_status ON roadmap_orchestrations(roadmap_item_id, status);
        "#,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn orch_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RoadmapOrchestration> {
    Ok(RoadmapOrchestration {
        id: row.get(0)?,
        group_id: row.get(1)?,
        roadmap_item_id: row.get(2)?,
        status: row.get(3)?,
        cursor_feature_id: row.get(4)?,
        cursor_task_id: row.get(5)?,
        current_run_id: row.get(6)?,
        error_message: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

const ORCH_SELECT: &str = "SELECT id,group_id,roadmap_item_id,status,cursor_feature_id,cursor_task_id,current_run_id,error_message,created_at,updated_at FROM roadmap_orchestrations";

pub fn get_orchestration(conn: &Connection, id: &str) -> AppResult<RoadmapOrchestration> {
    conn.query_row(
        &format!("{ORCH_SELECT} WHERE id=?1"),
        params![id],
        orch_from_row,
    )
    .map_err(|e| format!("编排不存在：{e}"))
}

pub fn list_orchestrations(conn: &Connection, group_id: &str) -> AppResult<Vec<RoadmapOrchestration>> {
    let mut stmt = conn
        .prepare(&format!(
            "{ORCH_SELECT} WHERE group_id=?1 ORDER BY updated_at DESC LIMIT 50"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], orch_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn active_orch_for_item(conn: &Connection, roadmap_item_id: &str) -> AppResult<Option<RoadmapOrchestration>> {
    conn.query_row(
        &format!("{ORCH_SELECT} WHERE roadmap_item_id=?1 AND status IN ('running','paused','failed') ORDER BY updated_at DESC LIMIT 1"),
        params![roadmap_item_id],
        orch_from_row,
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn emit_orch(tx: &broadcast::Sender<String>, orch: &RoadmapOrchestration) {
    let payload = json!({
        "kind": "orchestration_status",
        "groupId": orch.group_id,
        "group_id": orch.group_id,
        "orchestrationId": orch.id,
        "roadmapItemId": orch.roadmap_item_id,
        "status": orch.status,
        "cursorFeatureId": orch.cursor_feature_id,
        "cursorTaskId": orch.cursor_task_id,
        "currentRunId": orch.current_run_id,
        "error": orch.error_message,
    });
    let _ = tx.send(payload.to_string());
}

fn load_bound_work(
    conn: &Connection,
    group_id: &str,
    roadmap_item_id: &str,
) -> AppResult<(Vec<Feature>, Vec<FeatureTask>)> {
    let features: Vec<Feature> = get_features(conn, group_id)?
        .into_iter()
        .filter(|f| f.target_roadmap_item_id.as_deref() == Some(roadmap_item_id))
        .collect();
    if features.is_empty() {
        return Err("请先在看板把功能关联到该路线图项。".into());
    }
    let mut tasks = Vec::new();
    for f in &features {
        tasks.extend(get_feature_tasks(conn, &f.id)?);
    }
    if next_checklist(&features, &tasks).is_none() {
        // Allow start only if there is work; if all done, still error.
        let any_tasks = !tasks.is_empty();
        if !any_tasks {
            return Err("关联功能下还没有 checklist 子任务。".into());
        }
        return Err("该路线图项下的 checklist 已全部完成。".into());
    }
    Ok((features, tasks))
}

fn build_prompt(
    roadmap_title: &str,
    feature: &Feature,
    task: &FeatureTask,
    agent: &Member,
) -> String {
    format!(
        "@{agent} 【路线图闭环】{roadmap} / {feature} / {task}\n\
         请完成该 checklist 任务。工作区与群公告规则照常遵守。\n\
         完成后在最终回复中明确说明已完成（建议包含 TASK_DONE）。",
        agent = agent.display_name,
        roadmap = roadmap_title,
        feature = feature.title,
        task = task.title,
    )
}

fn dispatch_current(
    conn: &Connection,
    group: &Group,
    orch: &mut RoadmapOrchestration,
    roadmap_title: &str,
    feature: &Feature,
    task: &FeatureTask,
    agent: &Member,
) -> AppResult<String> {
    let content = build_prompt(roadmap_title, feature, task, agent);
    let msg_id = id();
    let created = now();
    conn.execute(
        "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,'completed',?5)",
        params![msg_id, group.id, group.owner_member_id, content, created],
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.execute(
        "INSERT OR IGNORE INTO mentions(message_id,member_id) VALUES(?1,?2)",
        params![msg_id, agent.id],
    );
    let run_id = create_task_run(conn, &group.id, &msg_id, &agent.id, None, 0)?;
    let ts = now();
    orch.cursor_feature_id = Some(feature.id.clone());
    orch.cursor_task_id = Some(task.id.clone());
    orch.current_run_id = Some(run_id.clone());
    orch.status = "running".into();
    orch.error_message = None;
    orch.updated_at = ts;
    conn.execute(
        "UPDATE roadmap_orchestrations SET status=?1,cursor_feature_id=?2,cursor_task_id=?3,current_run_id=?4,error_message=NULL,updated_at=?5 WHERE id=?6",
        params![
            orch.status,
            orch.cursor_feature_id,
            orch.cursor_task_id,
            orch.current_run_id,
            orch.updated_at,
            orch.id
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(run_id)
}

pub fn start_roadmap_item(
    db_path: &std::path::Path,
    roadmap_item_id: &str,
    tx: &broadcast::Sender<String>,
    sched: SchedulerState,
) -> AppResult<RoadmapOrchestration> {
    let conn = open_db(db_path)?;
    ensure_orchestrations_table(&conn)?;
    let (group_id, roadmap_title, item_status): (String, String, String) = conn
        .query_row(
            "SELECT group_id,title,status FROM roadmap_items WHERE id=?1",
            params![roadmap_item_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| "路线图项不存在。".to_string())?;
    if item_status == "done" {
        return Err("该路线图项已完成。".into());
    }
    if let Some(existing) = active_orch_for_item(&conn, roadmap_item_id)? {
        if existing.status == "running" {
            return Err("该路线图项已有进行中的编排。".into());
        }
        // paused → treat start as resume
        drop(conn);
        return resume_orchestration(db_path, &existing.id, tx, sched);
    }
    let group = get_group(&conn, &group_id)?;
    let (features, tasks) = load_bound_work(&conn, &group_id, roadmap_item_id)?;
    let (feature, task) = next_checklist(&features, &tasks).ok_or_else(|| "没有可执行的 checklist。".to_string())?;
    let agent_id = resolve_assignee(feature, group.admin_member_id.as_deref())?;
    let members = get_members(&conn, &group_id)?;
    let agent = members
        .iter()
        .find(|m| m.id == agent_id && m.is_active && (m.kind == "agent" || m.kind == "chatbot"))
        .ok_or_else(|| "指派的成员不是可用的 Agent/聊天机器人。".to_string())?
        .clone();

    let ts = now();
    let mut orch = RoadmapOrchestration {
        id: id(),
        group_id: group_id.clone(),
        roadmap_item_id: roadmap_item_id.to_string(),
        status: "running".into(),
        cursor_feature_id: None,
        cursor_task_id: None,
        current_run_id: None,
        error_message: None,
        created_at: ts,
        updated_at: ts,
    };
    conn.execute(
        "INSERT INTO roadmap_orchestrations(id,group_id,roadmap_item_id,status,cursor_feature_id,cursor_task_id,current_run_id,error_message,created_at,updated_at) VALUES(?1,?2,?3,?4,NULL,NULL,NULL,NULL,?5,?6)",
        params![orch.id, orch.group_id, orch.roadmap_item_id, orch.status, orch.created_at, orch.updated_at],
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.execute(
        "UPDATE roadmap_items SET status='in_progress' WHERE id=?1",
        params![roadmap_item_id],
    );
    dispatch_current(
        &conn,
        &group,
        &mut orch,
        &roadmap_title,
        feature,
        task,
        &agent,
    )?;
    emit_orch(tx, &orch);
    drop(conn);
    scheduler::schedule_group(sched, group_id);
    Ok(orch)
}

pub fn pause_orchestration(
    db_path: &std::path::Path,
    orch_id: &str,
    tx: &broadcast::Sender<String>,
) -> AppResult<RoadmapOrchestration> {
    let conn = open_db(db_path)?;
    ensure_orchestrations_table(&conn)?;
    let mut orch = get_orchestration(&conn, orch_id)?;
    if orch.status != "running" {
        return Err("只有进行中的编排可以暂停。".into());
    }
    orch.status = "paused".into();
    orch.error_message = Some("已手动暂停。".into());
    orch.updated_at = now();
    conn.execute(
        "UPDATE roadmap_orchestrations SET status=?1,error_message=?2,updated_at=?3 WHERE id=?4",
        params![orch.status, orch.error_message, orch.updated_at, orch.id],
    )
    .map_err(|e| e.to_string())?;
    emit_orch(tx, &orch);
    Ok(orch)
}

pub fn cancel_orchestration(
    db_path: &std::path::Path,
    orch_id: &str,
    tx: &broadcast::Sender<String>,
) -> AppResult<RoadmapOrchestration> {
    let conn = open_db(db_path)?;
    ensure_orchestrations_table(&conn)?;
    let mut orch = get_orchestration(&conn, orch_id)?;
    if matches!(orch.status.as_str(), "completed" | "cancelled") {
        return Err("编排已结束。".into());
    }
    orch.status = "cancelled".into();
    orch.current_run_id = None;
    orch.updated_at = now();
    conn.execute(
        "UPDATE roadmap_orchestrations SET status='cancelled',current_run_id=NULL,updated_at=?1 WHERE id=?2",
        params![orch.updated_at, orch.id],
    )
    .map_err(|e| e.to_string())?;
    emit_orch(tx, &orch);
    Ok(orch)
}

pub fn resume_orchestration(
    db_path: &std::path::Path,
    orch_id: &str,
    tx: &broadcast::Sender<String>,
    sched: SchedulerState,
) -> AppResult<RoadmapOrchestration> {
    let conn = open_db(db_path)?;
    ensure_orchestrations_table(&conn)?;
    let mut orch = get_orchestration(&conn, orch_id)?;
    if orch.status != "paused" && orch.status != "failed" {
        return Err("只有暂停/失败的编排可以继续。".into());
    }
    let group = get_group(&conn, &orch.group_id)?;
    let roadmap_title: String = conn
        .query_row(
            "SELECT title FROM roadmap_items WHERE id=?1",
            params![orch.roadmap_item_id],
            |r| r.get(0),
        )
        .map_err(|_| "路线图项不存在。".to_string())?;
    let (features, tasks) = load_bound_work(&conn, &orch.group_id, &orch.roadmap_item_id)?;
    let (feature, task) = next_checklist(&features, &tasks).ok_or_else(|| {
        // All done while paused
        "无剩余 checklist，请取消或等待完成收尾。".to_string()
    })?;
    let agent_id = resolve_assignee(feature, group.admin_member_id.as_deref())?;
    let members = get_members(&conn, &orch.group_id)?;
    let agent = members
        .iter()
        .find(|m| m.id == agent_id && m.is_active && (m.kind == "agent" || m.kind == "chatbot"))
        .ok_or_else(|| "指派的成员不是可用的 Agent/聊天机器人。".to_string())?
        .clone();
    let group_id = orch.group_id.clone();
    dispatch_current(
        &conn,
        &group,
        &mut orch,
        &roadmap_title,
        feature,
        task,
        &agent,
    )?;
    emit_orch(tx, &orch);
    drop(conn);
    scheduler::schedule_group(sched, group_id);
    Ok(orch)
}

/// Called when a task_run reaches a terminal status.
pub fn on_run_terminal(
    db_path: &std::path::Path,
    run_id: &str,
    succeeded: bool,
    error: Option<&str>,
    tx: &broadcast::Sender<String>,
    sched: SchedulerState,
) {
    let result = (|| -> AppResult<()> {
        let conn = open_db(db_path)?;
        ensure_orchestrations_table(&conn)?;
        let mut orch = match conn
            .query_row(
                &format!("{ORCH_SELECT} WHERE current_run_id=?1 AND status='running'"),
                params![run_id],
                orch_from_row,
            )
            .optional()
            .map_err(|e| e.to_string())?
        {
            Some(o) => o,
            None => return Ok(()),
        };
        if !succeeded {
            orch.status = "failed".into();
            orch.error_message = Some(error.unwrap_or("任务失败，编排已暂停，可点「继续」重试。").into());
            orch.updated_at = now();
            conn.execute(
                "UPDATE roadmap_orchestrations SET status=?1,error_message=?2,updated_at=?3 WHERE id=?4",
                params![orch.status, orch.error_message, orch.updated_at, orch.id],
            )
            .map_err(|e| e.to_string())?;
            emit_orch(tx, &orch);
            return Ok(());
        }

        if let Some(task_id) = orch.cursor_task_id.clone() {
            let _ = conn.execute(
                "UPDATE feature_tasks SET done=1 WHERE id=?1",
                params![task_id],
            );
        }
        if let Some(feature_id) = orch.cursor_feature_id.clone() {
            let remaining: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM feature_tasks WHERE feature_id=?1 AND done=0",
                    params![feature_id],
                    |r| r.get(0),
                )
                .unwrap_or(1);
            if remaining == 0 {
                let _ = conn.execute(
                    "UPDATE features SET status='done',updated_at=?1 WHERE id=?2",
                    params![now(), feature_id],
                );
            }
        }

        let group = get_group(&conn, &orch.group_id)?;
        let roadmap_title: String = conn
            .query_row(
                "SELECT title FROM roadmap_items WHERE id=?1",
                params![orch.roadmap_item_id],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "路线图".into());
        let (features, tasks) = match load_bound_work(&conn, &orch.group_id, &orch.roadmap_item_id) {
            Ok(v) => v,
            Err(_) => {
                // no remaining work
                orch.status = "completed".into();
                orch.current_run_id = None;
                orch.updated_at = now();
                conn.execute(
                    "UPDATE roadmap_orchestrations SET status='completed',current_run_id=NULL,updated_at=?1 WHERE id=?2",
                    params![orch.updated_at, orch.id],
                )
                .map_err(|e| e.to_string())?;
                let _ = conn.execute(
                    "UPDATE roadmap_items SET status='done' WHERE id=?1",
                    params![orch.roadmap_item_id],
                );
                emit_orch(tx, &orch);
                return Ok(());
            }
        };
        let Some((feature, task)) = next_checklist(&features, &tasks) else {
            orch.status = "completed".into();
            orch.current_run_id = None;
            orch.updated_at = now();
            conn.execute(
                "UPDATE roadmap_orchestrations SET status='completed',current_run_id=NULL,updated_at=?1 WHERE id=?2",
                params![orch.updated_at, orch.id],
            )
            .map_err(|e| e.to_string())?;
            let _ = conn.execute(
                "UPDATE roadmap_items SET status='done' WHERE id=?1",
                params![orch.roadmap_item_id],
            );
            emit_orch(tx, &orch);
            return Ok(());
        };
        let agent_id = resolve_assignee(feature, group.admin_member_id.as_deref())?;
        let members = get_members(&conn, &orch.group_id)?;
        let agent = members
            .iter()
            .find(|m| m.id == agent_id && m.is_active && (m.kind == "agent" || m.kind == "chatbot"))
            .ok_or_else(|| "指派的成员不可用。".to_string())?
            .clone();
        let group_id = orch.group_id.clone();
        dispatch_current(
            &conn,
            &group,
            &mut orch,
            &roadmap_title,
            feature,
            task,
            &agent,
        )?;
        emit_orch(tx, &orch);
        drop(conn);
        scheduler::schedule_group(sched, group_id);
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("orchestrator on_run_terminal: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(id: &str, order: i64, assignee: Option<&str>) -> Feature {
        Feature {
            id: id.into(),
            group_id: "g".into(),
            title: id.into(),
            description: String::new(),
            status: "backlog".into(),
            priority: "medium".into(),
            area: String::new(),
            assignee_member_id: assignee.map(str::to_string),
            target_roadmap_item_id: Some("r1".into()),
            sort_order: order,
            created_at: order,
            updated_at: order,
        }
    }

    fn task(id: &str, feature_id: &str, order: i64, done: bool) -> FeatureTask {
        FeatureTask {
            id: id.into(),
            feature_id: feature_id.into(),
            title: id.into(),
            done,
            sort_order: order,
            created_at: order,
        }
    }

    #[test]
    fn assignee_prefers_feature_then_admin() {
        let f = feature("f1", 0, Some("agent-a"));
        assert_eq!(resolve_assignee(&f, Some("admin")).unwrap(), "agent-a");
        let f2 = feature("f2", 0, None);
        assert_eq!(resolve_assignee(&f2, Some("admin")).unwrap(), "admin");
        assert!(resolve_assignee(&f2, None).is_err());
    }

    #[test]
    fn checklist_is_serial_across_features() {
        let features = vec![feature("f1", 1, None), feature("f2", 0, None)];
        let tasks = vec![
            task("t1", "f1", 0, false),
            task("t2", "f2", 0, false),
            task("t3", "f2", 1, true),
        ];
        let (f, t) = next_checklist(&features, &tasks).unwrap();
        assert_eq!(f.id, "f2");
        assert_eq!(t.id, "t2");
    }
}
