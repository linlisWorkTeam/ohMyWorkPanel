//! Goal 运行时：把一次驱动收敛成「加锁 → 执行一轮 → 事务写回」。
//!
//! 真实调度器（发消息、等 run）由 `GoalRunner` 注入；这里解决的是并发与一致性：
//! - 同一个 goal 同一时刻只允许一个驱动器（进程内锁）；
//! - runner 失败时不写回任何轮次；
//! - 状态与轮次用单个事务写回，避免半套数据。

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::db::{now, AppResult};

use super::driver::{GoalDriver, StepReport};
use super::store;
use super::Goal;

fn driving() -> &'static Mutex<HashSet<String>> {
    static DRIVING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    DRIVING.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 驱动锁；离开作用域自动释放。
pub struct DriveGuard {
    goal_id: String,
}

impl Drop for DriveGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = driving().lock() {
            set.remove(&self.goal_id);
        }
    }
}

/// 尝试取得某个 goal 的驱动权；已被占用时返回错误。
pub fn try_begin(goal_id: &str) -> AppResult<DriveGuard> {
    let mut set = driving().lock().map_err(|_| "驱动锁不可用".to_string())?;
    if !set.insert(goal_id.to_string()) {
        return Err(format!("目标正在被驱动：{goal_id}"));
    }
    Ok(DriveGuard {
        goal_id: goal_id.to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct TickReport {
    /// 执行后的目标快照。
    pub goal: Goal,
    /// 本轮详情。
    pub step: StepReport,
}

/// 执行一轮并落库。runner/judge 由调用方注入。
///
/// 注意：runner 可能会写库（发消息、建 run），因此不能把 `step` 包在目标事务里，
/// 否则会与 SQLite 写锁冲突。这里先在内存推进，再用一个事务把状态与轮次写回；
/// 若 runner 成功后写回失败，会留下消息/run 但没有轮次记录，属于可接受的降级。
pub fn tick(db_path: &Path, goal_id: &str, driver: &GoalDriver<'_>) -> AppResult<TickReport> {
    let guard = try_begin(goal_id)?;
    let mut goal =
        store::load_goal(db_path, goal_id)?.ok_or_else(|| format!("goal 不存在：{goal_id}"))?;
    let step = driver.step(&mut goal, now()).map_err(String::from)?;
    store::persist_goal(db_path, &goal)?;
    drop(guard);
    Ok(TickReport { goal, step })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_db, open_db};
    use crate::goals::driver::{GoalJudge, GoalRunner, RoundObservation};
    use crate::goals::service;
    use crate::goals::{GoalBudget, GoalStatus, RoundVerdict, StopReason};
    use tempfile::NamedTempFile;

    struct FixedRunner {
        tokens: u64,
        fail: bool,
    }

    impl GoalRunner for FixedRunner {
        fn run_round(&self, _goal: &Goal, _round_index: u32) -> Result<RoundObservation, String> {
            if self.fail {
                return Err("boom".into());
            }
            Ok(RoundObservation {
                summary: "done".into(),
                tokens_used: self.tokens,
            })
        }
    }

    struct FixedJudge(RoundVerdict);

    impl GoalJudge for FixedJudge {
        fn judge(&self, _goal: &Goal, _observation: &RoundObservation) -> RoundVerdict {
            self.0
        }
    }

    fn db_with_group() -> NamedTempFile {
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

    fn started_goal(file: &NamedTempFile) -> Goal {
        let goal = service::create_goal(
            file.path(),
            "grp-1",
            "把首页画出来",
            GoalBudget::default(),
            false,
        )
        .unwrap();
        service::start_goal(file.path(), &goal.id).unwrap()
    }

    #[test]
    fn tick_persists_round_and_terminal_state() {
        let file = db_with_group();
        let goal = started_goal(&file);
        let runner = FixedRunner {
            tokens: 12,
            fail: false,
        };
        let judge = FixedJudge(RoundVerdict::GoalMet);
        let driver = GoalDriver::new(&runner, &judge, 5);

        let report = tick(file.path(), &goal.id, &driver).unwrap();

        assert_eq!(report.step.round_index, 0);
        assert_eq!(report.goal.status, GoalStatus::Completed);
        assert_eq!(report.goal.stop_reason, Some(StopReason::GoalMet));
        let loaded = service::get_goal(file.path(), &goal.id).unwrap().unwrap();
        assert_eq!(loaded.rounds.len(), 1);
        assert_eq!(loaded.rounds[0].tokens_used, 12);
        assert_eq!(loaded.rounds[0].summary, "done");
    }

    #[test]
    fn second_tick_is_rejected_while_driving() {
        let file = db_with_group();
        let goal = started_goal(&file);
        let _guard = try_begin(&goal.id).unwrap();

        let error = try_begin(&goal.id).err().unwrap();

        assert!(error.contains("正在被驱动"));
    }

    #[test]
    fn runner_failure_rolls_back_round() {
        let file = db_with_group();
        let goal = started_goal(&file);
        let runner = FixedRunner {
            tokens: 0,
            fail: true,
        };
        let judge = FixedJudge(RoundVerdict::Progress);
        let driver = GoalDriver::new(&runner, &judge, 5);

        assert!(tick(file.path(), &goal.id, &driver).is_err());

        let loaded = service::get_goal(file.path(), &goal.id).unwrap().unwrap();
        assert!(loaded.rounds.is_empty());
        assert_eq!(loaded.status, GoalStatus::Active);
    }

    #[test]
    fn missing_goal_is_reported() {
        let file = db_with_group();
        let runner = FixedRunner {
            tokens: 1,
            fail: false,
        };
        let judge = FixedJudge(RoundVerdict::Progress);
        let driver = GoalDriver::new(&runner, &judge, 5);

        let error = tick(file.path(), "nope", &driver).err().unwrap();

        assert!(error.contains("goal 不存在"));
    }

    #[test]
    fn guard_is_released_after_tick() {
        let file = db_with_group();
        let goal = started_goal(&file);
        let runner = FixedRunner {
            tokens: 1,
            fail: false,
        };
        let judge = FixedJudge(RoundVerdict::Progress);
        let driver = GoalDriver::new(&runner, &judge, 5);

        tick(file.path(), &goal.id, &driver).unwrap();
        let again = try_begin(&goal.id);

        assert!(again.is_ok());
    }
}
