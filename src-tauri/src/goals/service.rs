//! Goal 应用服务：校验、事务与状态迁移的组合入口。
//!
//! 传输层（Tauri / Web）只做协议与权限解析，业务校验与状态迁移集中在这里，
//! 保证两个运行时的行为一致。

use std::cell::Cell;
use std::path::Path;

use crate::db::{id, now, open_db, AppResult};

use super::store;
use super::{Goal, GoalBudget, RoundOutcome, RoundVerdict};

/// 目标描述上限，避免把整篇需求塞进一行。
pub const MAX_OBJECTIVE_CHARS: usize = 2000;

pub fn create_goal(
    db_path: &Path,
    group_id: &str,
    objective: &str,
    budget: GoalBudget,
    require_approval_each_round: bool,
) -> AppResult<Goal> {
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return Err("groupId 必填".into());
    }
    let objective = objective.trim();
    if objective.is_empty() {
        return Err("目标不能为空".into());
    }
    if objective.chars().count() > MAX_OBJECTIVE_CHARS {
        return Err(format!("目标过长（上限 {MAX_OBJECTIVE_CHARS} 字）"));
    }
    ensure_group_exists(db_path, group_id)?;

    let mut goal = Goal::new(id(), group_id, objective, budget, now());
    goal.require_approval_each_round = require_approval_each_round;
    store::create_goal(db_path, &goal)?;
    Ok(goal)
}

fn ensure_group_exists(db_path: &Path, group_id: &str) -> AppResult<()> {
    let conn = open_db(db_path)?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM groups WHERE id = ?1",
            [group_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if count == 0 {
        Err(format!("群不存在：{group_id}"))
    } else {
        Ok(())
    }
}

pub fn start_goal(db_path: &Path, goal_id: &str) -> AppResult<Goal> {
    store::update_goal(db_path, goal_id, |goal| Ok(goal.start(now())?))
}

pub fn pause_goal(db_path: &Path, goal_id: &str) -> AppResult<Goal> {
    store::update_goal(db_path, goal_id, |goal| Ok(goal.pause()?))
}

pub fn approve_goal(db_path: &Path, goal_id: &str) -> AppResult<Goal> {
    store::update_goal(db_path, goal_id, |goal| Ok(goal.approve()?))
}

pub fn abort_goal(db_path: &Path, goal_id: &str) -> AppResult<Goal> {
    store::update_goal(db_path, goal_id, |goal| Ok(goal.abort_by_human()?))
}

/// 记录一轮并返回目标的最新快照与引擎决策。轮次与状态在同一事务内落库。
pub fn record_round(
    db_path: &Path,
    goal_id: &str,
    verdict: RoundVerdict,
    tokens_used: u64,
) -> AppResult<(Goal, RoundOutcome)> {
    let outcome = Cell::new(RoundOutcome::Continue);
    let goal = store::update_goal(db_path, goal_id, |goal| {
        outcome.set(goal.record_round(verdict, tokens_used, now())?);
        Ok(())
    })?;
    Ok((goal, outcome.get()))
}

pub fn get_goal(db_path: &Path, goal_id: &str) -> AppResult<Option<Goal>> {
    store::load_goal(db_path, goal_id)
}

pub fn list_goals(db_path: &Path, group_id: &str) -> AppResult<Vec<Goal>> {
    if group_id.trim().is_empty() {
        return Err("groupId 必填".into());
    }
    store::list_goals(db_path, group_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::goals::{GoalStatus, StopReason};
    use tempfile::NamedTempFile;

    fn db() -> NamedTempFile {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups (id, name, workspace_path, owner_member_id, created_at)
             VALUES ('grp-1', '测试群', '/tmp', 'm1', 1)",
            [],
        )
        .unwrap();
        file
    }

    fn budget() -> GoalBudget {
        GoalBudget::default()
    }

    #[test]
    fn creates_and_lists_goals_for_a_group() {
        let file = db();
        let goal = create_goal(file.path(), "grp-1", "把首页画出来", budget(), false).unwrap();
        assert_eq!(goal.status, GoalStatus::Draft);

        let listed = list_goals(file.path(), "grp-1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, goal.id);
        assert!(get_goal(file.path(), &goal.id).unwrap().is_some());
    }

    #[test]
    fn rejects_empty_objective_and_unknown_group() {
        let file = db();
        assert!(create_goal(file.path(), "grp-1", "  ", budget(), false).is_err());
        assert!(create_goal(file.path(), "", "目标", budget(), false).is_err());
        let error = create_goal(file.path(), "missing", "目标", budget(), false)
            .err()
            .unwrap();
        assert!(error.contains("群不存在"));
    }

    #[test]
    fn lifecycle_persists_status_and_rounds() {
        let file = db();
        let goal = create_goal(file.path(), "grp-1", "把首页画出来", budget(), false).unwrap();
        start_goal(file.path(), &goal.id).unwrap();
        let (after, outcome) =
            record_round(file.path(), &goal.id, RoundVerdict::GoalMet, 42).unwrap();

        assert_eq!(outcome, RoundOutcome::Stopped(StopReason::GoalMet));
        assert_eq!(after.status, GoalStatus::Completed);

        let loaded = get_goal(file.path(), &goal.id).unwrap().unwrap();
        assert_eq!(loaded.rounds.len(), 1);
        assert_eq!(loaded.rounds[0].tokens_used, 42);
        assert_eq!(loaded.stop_reason, Some(StopReason::GoalMet));
    }

    #[test]
    fn approval_flow_gates_the_next_round() {
        let file = db();
        let goal = create_goal(file.path(), "grp-1", "推进", budget(), true).unwrap();
        start_goal(file.path(), &goal.id).unwrap();

        let (after, outcome) =
            record_round(file.path(), &goal.id, RoundVerdict::Progress, 1).unwrap();
        assert_eq!(outcome, RoundOutcome::AwaitApproval);
        assert_eq!(after.status, GoalStatus::AwaitingApproval);

        let approved = approve_goal(file.path(), &goal.id).unwrap();
        assert_eq!(approved.status, GoalStatus::Active);
    }

    #[test]
    fn aborting_is_terminal() {
        let file = db();
        let goal = create_goal(file.path(), "grp-1", "推进", budget(), false).unwrap();
        start_goal(file.path(), &goal.id).unwrap();
        let aborted = abort_goal(file.path(), &goal.id).unwrap();
        assert_eq!(aborted.status, GoalStatus::Aborted);
        assert!(record_round(file.path(), &goal.id, RoundVerdict::Progress, 1).is_err());
    }

    #[test]
    fn budget_exhaustion_persists_stop_reason() {
        let file = db();
        let goal = create_goal(
            file.path(),
            "grp-1",
            "推进",
            GoalBudget {
                max_rounds: 1,
                ..GoalBudget::default()
            },
            false,
        )
        .unwrap();
        start_goal(file.path(), &goal.id).unwrap();

        let (after, outcome) =
            record_round(file.path(), &goal.id, RoundVerdict::Progress, 1).unwrap();

        assert_eq!(outcome, RoundOutcome::Stopped(StopReason::RoundsExhausted));
        assert_eq!(after.status, GoalStatus::Aborted);
        let loaded = get_goal(file.path(), &goal.id).unwrap().unwrap();
        assert_eq!(loaded.stop_reason, Some(StopReason::RoundsExhausted));
    }
}
