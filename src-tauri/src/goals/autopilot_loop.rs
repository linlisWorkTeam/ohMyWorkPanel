//! 自动推进后台循环：扫描 active + auto 目标，逐个派发一轮。
//!
//! 必须运行在 tokio 运行时里：`schedule_group` 内部会用 `tokio::spawn` 执行 run，
//! 在裸线程里调用会 panic。两个入口各自用自己运行时的 spawn（Tauri / axum）。

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::db::{now, AppResult};
use crate::scheduler::{schedule_group, SchedulerState};

use super::autopilot::{run_once, AutopilotPolicy};
use super::scheduler_adapter::{tick, SchedulerDispatcher, DEFAULT_MAX_POLLS};
use super::service;
use super::GoalStatus;

/// 推进单个目标一轮。
pub fn advance_once(db_path: &Path, goal_id: &str, sched: &SchedulerState) -> AppResult<()> {
    let goal =
        service::get_goal(db_path, goal_id)?.ok_or_else(|| format!("goal 不存在：{goal_id}"))?;
    if goal.status != GoalStatus::Active {
        return Ok(());
    }
    let actor = service::pick_goal_actor(db_path, &goal.group_id)?;
    let kick_sched = sched.clone();
    let dispatcher = SchedulerDispatcher::new(
        db_path.to_path_buf(),
        goal.group_id.clone(),
        actor.driver_member_id,
        actor.sender_member_id,
        Box::new(move |group_id| {
            schedule_group(kick_sched.clone(), group_id.to_string());
        }),
    );
    tick(db_path, goal_id, dispatcher, DEFAULT_MAX_POLLS)?;
    Ok(())
}

/// 扫描一轮并把每个目标的推进放到阻塞线程池里跑。
pub async fn scan_once(db_path: PathBuf, sched: SchedulerState) -> AppResult<()> {
    tokio::task::spawn_blocking(move || {
        let report = run_once(&db_path, now(), AutopilotPolicy::default(), |goal_id| {
            advance_once(&db_path, goal_id, &sched)
        })?;
        for (goal_id, error) in &report.failed {
            eprintln!("goal autopilot {goal_id}: {error}");
        }
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 常驻循环。调用方负责在自己的运行时里 spawn 它。
pub async fn run_forever(db_path: PathBuf, sched: SchedulerState, interval: Duration) {
    loop {
        if let Err(error) = scan_once(db_path.clone(), sched.clone()).await {
            eprintln!("goal autopilot scan: {error}");
        }
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_db, open_db};
    use crate::event_sender::EventSender;
    use crate::goals::{service, GoalBudget};
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};
    use tempfile::NamedTempFile;

    fn sched(path: &Path) -> SchedulerState {
        let (tx, _rx) = tokio::sync::broadcast::channel::<String>(8);
        SchedulerState {
            db_path: path.to_path_buf(),
            event_sender: EventSender::Web(tx),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            scheduling_groups: Arc::new(Mutex::new(HashSet::new())),
            live_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[test]
    fn advance_once_skips_non_active_goals() {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups (id, name, workspace_path, owner_member_id, created_at)
             VALUES ('g', '群', '/tmp', 'm-owner', 1)",
            [],
        )
        .unwrap();
        let goal =
            service::create_goal(file.path(), "g", "目标", GoalBudget::default(), false).unwrap();

        // draft 状态直接跳过，不触碰调度器。
        advance_once(file.path(), &goal.id, &sched(file.path())).unwrap();
    }
}
