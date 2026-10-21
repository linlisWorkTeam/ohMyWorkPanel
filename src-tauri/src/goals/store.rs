//! Goal 的 SQLite 持久化。
//!
//! 表结构见 `db_migrations::migrate_v8`。预算以 JSON 快照保存，枚举以 snake_case
//! 字符串落库，便于直接用 sqlite3 排查。

use std::path::Path;

use rusqlite::{params, OptionalExtension};

use crate::db::{open_db, AppResult};

use super::{Goal, GoalBudget, GoalRound, GoalStatus, RoundVerdict, StopReason};

fn status_to_str(status: GoalStatus) -> &'static str {
    match status {
        GoalStatus::Draft => "draft",
        GoalStatus::Active => "active",
        GoalStatus::Paused => "paused",
        GoalStatus::AwaitingApproval => "awaiting_approval",
        GoalStatus::Completed => "completed",
        GoalStatus::Aborted => "aborted",
    }
}

fn status_from_str(value: &str) -> AppResult<GoalStatus> {
    Ok(match value {
        "draft" => GoalStatus::Draft,
        "active" => GoalStatus::Active,
        "paused" => GoalStatus::Paused,
        "awaiting_approval" => GoalStatus::AwaitingApproval,
        "completed" => GoalStatus::Completed,
        "aborted" => GoalStatus::Aborted,
        other => return Err(format!("未知 goal 状态：{other}")),
    })
}

fn verdict_to_str(verdict: RoundVerdict) -> &'static str {
    match verdict {
        RoundVerdict::Progress => "progress",
        RoundVerdict::NoProgress => "no_progress",
        RoundVerdict::Failed => "failed",
        RoundVerdict::GoalMet => "goal_met",
    }
}

fn verdict_from_str(value: &str) -> AppResult<RoundVerdict> {
    Ok(match value {
        "progress" => RoundVerdict::Progress,
        "no_progress" => RoundVerdict::NoProgress,
        "failed" => RoundVerdict::Failed,
        "goal_met" => RoundVerdict::GoalMet,
        other => return Err(format!("未知轮次判定：{other}")),
    })
}

fn stop_reason_to_str(reason: StopReason) -> &'static str {
    match reason {
        StopReason::GoalMet => "goal_met",
        StopReason::RoundsExhausted => "rounds_exhausted",
        StopReason::WallTimeExhausted => "wall_time_exhausted",
        StopReason::TokensExhausted => "tokens_exhausted",
        StopReason::NoProgress => "no_progress",
        StopReason::ConsecutiveFailures => "consecutive_failures",
        StopReason::AbortedByHuman => "aborted_by_human",
    }
}

fn stop_reason_from_str(value: &str) -> AppResult<StopReason> {
    Ok(match value {
        "goal_met" => StopReason::GoalMet,
        "rounds_exhausted" => StopReason::RoundsExhausted,
        "wall_time_exhausted" => StopReason::WallTimeExhausted,
        "tokens_exhausted" => StopReason::TokensExhausted,
        "no_progress" => StopReason::NoProgress,
        "consecutive_failures" => StopReason::ConsecutiveFailures,
        "aborted_by_human" => StopReason::AbortedByHuman,
        other => return Err(format!("未知停止原因：{other}")),
    })
}

/// 新建目标（不写轮次）。
pub fn create_goal(db_path: &Path, goal: &Goal) -> AppResult<()> {
    let conn = open_db(db_path)?;
    let budget = serde_json::to_string(&goal.budget).map_err(|error| error.to_string())?;
    conn.execute(
        r#"INSERT INTO goals
           (id, group_id, objective, status, budget_json, require_approval_each_round,
            created_at, started_at, stop_reason)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
        params![
            goal.id,
            goal.group_id,
            goal.objective,
            status_to_str(goal.status),
            budget,
            goal.require_approval_each_round as i64,
            goal.created_at,
            goal.started_at,
            goal.stop_reason.map(stop_reason_to_str),
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// 更新目标的可变字段（状态、启动时间、停止原因、审批开关、预算）。
pub fn save_goal(db_path: &Path, goal: &Goal) -> AppResult<()> {
    let conn = open_db(db_path)?;
    let budget = serde_json::to_string(&goal.budget).map_err(|error| error.to_string())?;
    let changed = conn
        .execute(
            r#"UPDATE goals SET
                 objective = ?2, status = ?3, budget_json = ?4,
                 require_approval_each_round = ?5, started_at = ?6, stop_reason = ?7
               WHERE id = ?1"#,
            params![
                goal.id,
                goal.objective,
                status_to_str(goal.status),
                budget,
                goal.require_approval_each_round as i64,
                goal.started_at,
                goal.stop_reason.map(stop_reason_to_str),
            ],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err(format!("goal 不存在：{}", goal.id));
    }
    Ok(())
}

/// 追加（或覆盖）一轮记录。
pub fn append_round(db_path: &Path, goal_id: &str, round: &GoalRound) -> AppResult<()> {
    let conn = open_db(db_path)?;
    conn.execute(
        r#"INSERT OR REPLACE INTO goal_rounds (goal_id, idx, verdict, tokens_used, finished_at)
           VALUES (?1, ?2, ?3, ?4, ?5)"#,
        params![
            goal_id,
            round.index as i64,
            verdict_to_str(round.verdict),
            round.tokens_used as i64,
            round.finished_at,
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// 读取单个目标（含全部轮次）；不存在返回 `None`。
pub fn load_goal(db_path: &Path, goal_id: &str) -> AppResult<Option<Goal>> {
    let conn = open_db(db_path)?;
    let row = conn
        .query_row(
            r#"SELECT id, group_id, objective, status, budget_json,
                      require_approval_each_round, created_at, started_at, stop_reason
               FROM goals WHERE id = ?1"#,
            [goal_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let Some((id, group_id, objective, status, budget, approval, created_at, started_at, stop)) =
        row
    else {
        return Ok(None);
    };

    let budget: GoalBudget = serde_json::from_str(&budget).map_err(|error| error.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT idx, verdict, tokens_used, finished_at FROM goal_rounds WHERE goal_id = ?1 ORDER BY idx ASC",
        )
        .map_err(|error| error.to_string())?;
    let raw_rounds = stmt
        .query_map([goal_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    let mut rounds = Vec::with_capacity(raw_rounds.len());
    for (index, verdict, tokens_used, finished_at) in raw_rounds {
        rounds.push(GoalRound {
            index: index as u32,
            verdict: verdict_from_str(&verdict)?,
            tokens_used: tokens_used as u64,
            finished_at,
        });
    }

    Ok(Some(Goal {
        id,
        group_id,
        objective,
        budget,
        status: status_from_str(&status)?,
        created_at,
        started_at,
        rounds,
        stop_reason: stop.as_deref().map(stop_reason_from_str).transpose()?,
        require_approval_each_round: approval != 0,
    }))
}

/// 按创建时间倒序列出某群的 goal id。
pub fn list_goal_ids(db_path: &Path, group_id: &str) -> AppResult<Vec<String>> {
    let conn = open_db(db_path)?;
    let mut stmt = conn
        .prepare("SELECT id FROM goals WHERE group_id = ?1 ORDER BY created_at DESC")
        .map_err(|error| error.to_string())?;
    let ids = stmt
        .query_map([group_id], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(ids)
}

/// 列出某群的目标（含轮次）。
pub fn list_goals(db_path: &Path, group_id: &str) -> AppResult<Vec<Goal>> {
    let mut goals = Vec::new();
    for id in list_goal_ids(db_path, group_id)? {
        if let Some(goal) = load_goal(db_path, &id)? {
            goals.push(goal);
        }
    }
    Ok(goals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{id, init_db, now};
    use tempfile::NamedTempFile;

    fn db() -> NamedTempFile {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        file
    }

    fn sample_goal(group: &str) -> Goal {
        Goal::new(id(), group, "把首页渲染出来", GoalBudget::default(), now())
    }

    #[test]
    fn round_trips_goal_with_rounds() {
        let file = db();
        let mut goal = sample_goal("grp-1");
        goal.start(now()).unwrap();
        goal.record_round(RoundVerdict::Progress, 12, now())
            .unwrap();
        create_goal(file.path(), &goal).unwrap();
        save_goal(file.path(), &goal).unwrap();
        for round in &goal.rounds {
            append_round(file.path(), &goal.id, round).unwrap();
        }

        let loaded = load_goal(file.path(), &goal.id).unwrap().unwrap();
        assert_eq!(loaded, goal);
    }

    #[test]
    fn save_goal_persists_terminal_state() {
        let file = db();
        let mut goal = sample_goal("grp-1");
        goal.start(now()).unwrap();
        create_goal(file.path(), &goal).unwrap();
        goal.abort_by_human().unwrap();
        save_goal(file.path(), &goal).unwrap();

        let loaded = load_goal(file.path(), &goal.id).unwrap().unwrap();
        assert_eq!(loaded.status, GoalStatus::Aborted);
        assert_eq!(loaded.stop_reason, Some(StopReason::AbortedByHuman));
    }

    #[test]
    fn lists_goals_newest_first() {
        let file = db();
        let mut older = sample_goal("grp-2");
        older.created_at = 1_000;
        let mut newer = sample_goal("grp-2");
        newer.created_at = 2_000;
        create_goal(file.path(), &older).unwrap();
        create_goal(file.path(), &newer).unwrap();

        let goals = list_goals(file.path(), "grp-2").unwrap();

        assert_eq!(goals.len(), 2);
        assert_eq!(goals[0].id, newer.id);
        assert_eq!(goals[1].id, older.id);
    }

    #[test]
    fn missing_goal_returns_none() {
        let file = db();
        assert!(load_goal(file.path(), "nope").unwrap().is_none());
    }

    #[test]
    fn saving_unknown_goal_fails() {
        let file = db();
        let goal = sample_goal("grp-3");
        let error = save_goal(file.path(), &goal).err().unwrap();
        assert!(error.contains("goal 不存在"));
    }
}
