//! 自动推进：扫描 `active + auto_advance` 的目标，按冷却时间逐个推进一轮。
//!
//! 这里不创建线程、不持有调度器：调用方传入「推进一个 goal」的闭包。
//! 这样后台循环、重启恢复与单测共用同一套扫描规则。

use std::path::Path;

use crate::db::AppResult;

use super::{store, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutopilotPolicy {
    /// 同一目标两轮之间至少间隔多久（毫秒）。
    pub cooldown_ms: i64,
}

impl Default for AutopilotPolicy {
    fn default() -> Self {
        Self { cooldown_ms: 5_000 }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct AutopilotReport {
    pub advanced: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// 扫描一轮。只推进同时满足：`active`、`auto_advance`、冷却已过的目标。
///
/// 单个目标失败不会中断其他目标；失败原因原样带回，便于日志与界面提示。
pub fn run_once<F>(
    db_path: &Path,
    at: Timestamp,
    policy: AutopilotPolicy,
    mut advance: F,
) -> AppResult<AutopilotReport>
where
    F: FnMut(&str) -> AppResult<()>,
{
    let mut report = AutopilotReport::default();
    for goal in store::list_active_goals(db_path)? {
        if !goal.auto_advance {
            continue;
        }
        if let Some(last) = goal.rounds.last() {
            if at.saturating_sub(last.finished_at) < policy.cooldown_ms {
                report.skipped.push(goal.id);
                continue;
            }
        }
        match advance(&goal.id) {
            Ok(()) => report.advanced.push(goal.id),
            Err(error) => report.failed.push((goal.id, error)),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_db, now, open_db};
    use crate::goals::{service, Goal, GoalBudget, RoundVerdict};
    use std::cell::RefCell;
    use tempfile::NamedTempFile;

    fn db() -> NamedTempFile {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups (id, name, workspace_path, owner_member_id, created_at)
             VALUES ('g', '群', '/tmp', 'm-owner', 1)",
            [],
        )
        .unwrap();
        file
    }

    fn active_goal(file: &NamedTempFile, auto: bool) -> Goal {
        let goal = service::create_goal(
            file.path(),
            "g",
            "把首页画出来",
            GoalBudget::default(),
            false,
        )
        .unwrap();
        let goal = if auto {
            service::set_auto_advance(file.path(), &goal.id, true).unwrap()
        } else {
            goal
        };
        service::start_goal(file.path(), &goal.id).unwrap()
    }

    #[test]
    fn advances_auto_goals_only() {
        let file = db();
        let auto = active_goal(&file, true);
        let manual = active_goal(&file, false);
        let shared: &'static RefCell<Vec<String>> = Box::leak(Box::new(RefCell::new(Vec::new())));
        let advance = move |goal_id: &str| {
            shared.borrow_mut().push(goal_id.to_string());
            Ok(())
        };

        let report = run_once(file.path(), now(), AutopilotPolicy::default(), advance).unwrap();

        assert_eq!(report.advanced, vec![auto.id.clone()]);
        assert!(!report.advanced.contains(&manual.id));
        assert_eq!(*shared.borrow(), vec![auto.id]);
    }

    #[test]
    fn respects_cooldown() {
        let file = db();
        let goal = active_goal(&file, true);
        service::record_round(file.path(), &goal.id, RoundVerdict::Progress, 1).unwrap();
        let calls = RefCell::new(0);
        let advance = |_: &str| {
            *calls.borrow_mut() += 1;
            Ok(())
        };

        let skipped = run_once(
            file.path(),
            now(),
            AutopilotPolicy {
                cooldown_ms: 60_000,
            },
            advance,
        )
        .unwrap();
        assert!(skipped.advanced.is_empty());
        assert_eq!(skipped.skipped.len(), 1);

        let advance = |_: &str| {
            *calls.borrow_mut() += 1;
            Ok(())
        };
        let advanced = run_once(
            file.path(),
            now() + 60_000,
            AutopilotPolicy {
                cooldown_ms: 60_000,
            },
            advance,
        )
        .unwrap();
        assert_eq!(advanced.advanced.len(), 1);
        assert_eq!(*calls.borrow(), 1);
    }

    #[test]
    fn one_failure_does_not_block_others() {
        let file = db();
        let first = active_goal(&file, true);
        let second = active_goal(&file, true);
        let advance = |goal_id: &str| {
            if goal_id == first.id {
                Err("boom".into())
            } else {
                Ok(())
            }
        };

        let report = run_once(file.path(), now(), AutopilotPolicy::default(), advance).unwrap();

        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].0, first.id);
        assert!(report.advanced.contains(&second.id));
    }

    #[test]
    fn paused_goals_are_not_scanned() {
        let file = db();
        let goal = active_goal(&file, true);
        service::pause_goal(file.path(), &goal.id).unwrap();
        let advance = |_: &str| Ok(());

        let report = run_once(file.path(), now(), AutopilotPolicy::default(), advance).unwrap();

        assert!(report.advanced.is_empty());
        assert!(report.skipped.is_empty());
    }
}
