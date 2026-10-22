//! Goal 驱动器：把「跑一轮」抽象成可注入的 runner + judge，循环直到停止。
//!
//! 这一层刻意不碰数据库、不上线程：真实实现（发消息、等 run 完成）由调用方注入，
//! 这样循环语义、预算熔断与审批闸门都能用纯单测锁死。

use std::fmt;

use super::{Goal, GoalStatus, RoundOutcome, RoundVerdict, StopReason, Timestamp};

/// 一轮执行产出的观察结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundObservation {
    pub summary: String,
    pub tokens_used: u64,
}

/// 执行一轮。真实实现负责发消息、等待 run 完成并把结果压缩成观察。
pub trait GoalRunner {
    fn run_round(&self, goal: &Goal, round_index: u32) -> Result<RoundObservation, String>;
}

/// 判定一轮是否推进了目标。
pub trait GoalJudge {
    fn judge(&self, goal: &Goal, observation: &RoundObservation) -> RoundVerdict;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepReport {
    pub round_index: u32,
    pub verdict: RoundVerdict,
    pub tokens_used: u64,
    pub summary: String,
    pub outcome: RoundOutcome,
}

/// 一轮驱动结束的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverStop {
    /// 目标自身停止（达成或触线）。
    Goal(StopReason),
    /// 等待人类审批。
    AwaitingApproval,
    /// 单次驱动的步数上限，防止无人值守时无限循环。
    StepBudgetReached,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverRun {
    pub steps: Vec<StepReport>,
    pub stop: DriverStop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverError {
    NotActive(GoalStatus),
    Runner(String),
    Goal(String),
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotActive(status) => write!(f, "目标未处于运行中：{status:?}"),
            Self::Runner(error) => write!(f, "执行轮次失败：{error}"),
            Self::Goal(error) => write!(f, "记录轮次失败：{error}"),
        }
    }
}

impl std::error::Error for DriverError {}

impl From<DriverError> for String {
    fn from(error: DriverError) -> Self {
        error.to_string()
    }
}

/// 驱动器：注入 runner 与 judge，按轮推进。
pub struct GoalDriver<'a> {
    runner: &'a dyn GoalRunner,
    judge: &'a dyn GoalJudge,
    /// 单次 `run` 最多执行的轮数；0 视为 1。
    max_steps: u32,
}

impl<'a> GoalDriver<'a> {
    pub fn new(runner: &'a dyn GoalRunner, judge: &'a dyn GoalJudge, max_steps: u32) -> Self {
        Self {
            runner,
            judge,
            max_steps: max_steps.max(1),
        }
    }

    /// 执行并记录一轮。
    pub fn step(&self, goal: &mut Goal, now: Timestamp) -> Result<StepReport, DriverError> {
        if goal.status != GoalStatus::Active {
            return Err(DriverError::NotActive(goal.status));
        }
        let round_index = goal.rounds.len() as u32;
        let observation = self
            .runner
            .run_round(goal, round_index)
            .map_err(DriverError::Runner)?;
        let verdict = self.judge.judge(goal, &observation);
        let outcome = goal
            .record_round(verdict, observation.tokens_used, now)
            .map_err(|error| DriverError::Goal(error.to_string()))?;
        if let Some(round) = goal.rounds.last_mut() {
            round.summary = observation.summary.clone();
        }
        Ok(StepReport {
            round_index,
            verdict,
            tokens_used: observation.tokens_used,
            summary: observation.summary,
            outcome,
        })
    }

    /// 连续推进，直到目标停止、等待审批，或触达单次驱动步数上限。
    pub fn run(
        &self,
        goal: &mut Goal,
        mut now: impl FnMut() -> Timestamp,
    ) -> Result<DriverRun, DriverError> {
        let mut steps = Vec::new();
        loop {
            if steps.len() >= self.max_steps as usize {
                return Ok(DriverRun {
                    steps,
                    stop: DriverStop::StepBudgetReached,
                });
            }
            let report = self.step(goal, now())?;
            let outcome = report.outcome;
            steps.push(report);
            match outcome {
                RoundOutcome::Continue => continue,
                RoundOutcome::AwaitApproval => {
                    return Ok(DriverRun {
                        steps,
                        stop: DriverStop::AwaitingApproval,
                    })
                }
                RoundOutcome::Stopped(reason) => {
                    return Ok(DriverRun {
                        steps,
                        stop: DriverStop::Goal(reason),
                    })
                }
            }
        }
    }
}

/// 组装一轮的 prompt。保持确定性，便于测试与排查。
pub fn build_round_prompt(goal: &Goal) -> String {
    let round = goal.rounds.len() as u32 + 1;
    let previous = goal
        .rounds
        .last()
        .map(|last| format!("上一轮判定：{:?}", last.verdict))
        .unwrap_or_else(|| "这是第一轮".to_string());
    format!(
        "[GOAL] {}\n[ROUND] 第 {round} 轮（上限 {}）\n[PREVIOUS] {previous}\n请先说明本轮计划，再执行，最后汇报：完成了什么、还差什么、下一轮建议。\n最后一行 MUST 只写一个判决标记：{VERDICT_MET}（目标已达成）或 {VERDICT_PROGRESS}（有进展但未完成）或 {VERDICT_FAILED}（受阻或失败）。",
        goal.objective,
        if goal.budget.max_rounds == 0 {
            "不限".to_string()
        } else {
            goal.budget.max_rounds.to_string()
        },
        VERDICT_MET = VERDICT_MET,
        VERDICT_PROGRESS = VERDICT_PROGRESS,
        VERDICT_FAILED = VERDICT_FAILED,
    )
}

pub const VERDICT_MET: &str = "[GOAL:MET]";
pub const VERDICT_PROGRESS: &str = "[GOAL:PROGRESS]";
pub const VERDICT_FAILED: &str = "[GOAL:FAILED]";

/// 解析 Agent 的显式判决标记；取最后一次出现的标记（Agent 可能先引用格式再给结论）。
pub fn parse_verdict_marker(text: &str) -> Option<RoundVerdict> {
    let upper = text.to_uppercase();
    let mut found: Option<(usize, RoundVerdict)> = None;
    for (marker, verdict) in [
        (VERDICT_MET, RoundVerdict::GoalMet),
        (VERDICT_PROGRESS, RoundVerdict::Progress),
        (VERDICT_FAILED, RoundVerdict::Failed),
    ] {
        if let Some(index) = upper.rfind(marker) {
            if found.is_none_or(|(best, _)| index > best) {
                found = Some((index, verdict));
            }
        }
    }
    found.map(|(_, verdict)| verdict)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goals::{GoalBudget, RoundVerdict};
    use std::cell::Cell;

    struct ScriptedRunner {
        tokens: u64,
        fail: bool,
        calls: Cell<u32>,
    }

    impl ScriptedRunner {
        fn new(tokens: u64) -> Self {
            Self {
                tokens,
                fail: false,
                calls: Cell::new(0),
            }
        }

        fn failing() -> Self {
            Self {
                tokens: 0,
                fail: true,
                calls: Cell::new(0),
            }
        }
    }

    impl GoalRunner for ScriptedRunner {
        fn run_round(&self, _goal: &Goal, round_index: u32) -> Result<RoundObservation, String> {
            self.calls.set(self.calls.get() + 1);
            if self.fail {
                return Err("runner exploded".into());
            }
            Ok(RoundObservation {
                summary: format!("第 {round_index} 轮完成"),
                tokens_used: self.tokens,
            })
        }
    }

    struct ScriptedJudge {
        met_after: u32,
        seen: Cell<u32>,
        always: Option<RoundVerdict>,
    }

    impl ScriptedJudge {
        fn meeting_after(rounds: u32) -> Self {
            Self {
                met_after: rounds,
                seen: Cell::new(0),
                always: None,
            }
        }

        fn always(verdict: RoundVerdict) -> Self {
            Self {
                met_after: u32::MAX,
                seen: Cell::new(0),
                always: Some(verdict),
            }
        }
    }

    impl GoalJudge for ScriptedJudge {
        fn judge(&self, _goal: &Goal, _observation: &RoundObservation) -> RoundVerdict {
            self.seen.set(self.seen.get() + 1);
            if let Some(verdict) = self.always {
                return verdict;
            }
            if self.seen.get() >= self.met_after {
                RoundVerdict::GoalMet
            } else {
                RoundVerdict::Progress
            }
        }
    }

    fn active_goal(budget: GoalBudget) -> Goal {
        let mut goal = Goal::new("g1", "grp", "把首页画出来", budget, 0);
        goal.start(0).unwrap();
        goal
    }

    fn clock() -> impl FnMut() -> Timestamp {
        let mut tick = 0;
        move || {
            tick += 1_000;
            tick
        }
    }

    #[test]
    fn stops_when_judge_reports_goal_met() {
        let goal = active_goal(GoalBudget::default());
        let runner = ScriptedRunner::new(10);
        let judge = ScriptedJudge::meeting_after(3);
        let driver = GoalDriver::new(&runner, &judge, 50);
        let mut goal = goal;

        let run = driver.run(&mut goal, clock()).unwrap();

        assert_eq!(run.steps.len(), 3);
        assert_eq!(run.stop, DriverStop::Goal(StopReason::GoalMet));
        assert!(goal.status.is_terminal());
    }

    #[test]
    fn step_budget_prevents_runaway_loop() {
        let mut goal = active_goal(GoalBudget {
            max_rounds: 0,
            max_tokens: 0,
            ..GoalBudget::default()
        });
        let runner = ScriptedRunner::new(1);
        let judge = ScriptedJudge::always(RoundVerdict::Progress);
        let driver = GoalDriver::new(&runner, &judge, 5);

        let run = driver.run(&mut goal, clock()).unwrap();

        assert_eq!(run.steps.len(), 5);
        assert_eq!(run.stop, DriverStop::StepBudgetReached);
        assert_eq!(goal.rounds.len(), 5);
    }

    #[test]
    fn token_budget_stops_the_loop() {
        let mut goal = active_goal(GoalBudget {
            max_tokens: 25,
            ..GoalBudget::default()
        });
        let runner = ScriptedRunner::new(10);
        let judge = ScriptedJudge::always(RoundVerdict::Progress);
        let driver = GoalDriver::new(&runner, &judge, 50);

        let run = driver.run(&mut goal, clock()).unwrap();

        assert_eq!(run.steps.len(), 3);
        assert_eq!(run.stop, DriverStop::Goal(StopReason::TokensExhausted));
        assert_eq!(goal.total_tokens(), 30);
    }

    #[test]
    fn approval_gate_stops_after_one_round() {
        let mut goal = active_goal(GoalBudget::default());
        goal.require_approval_each_round = true;
        let runner = ScriptedRunner::new(1);
        let judge = ScriptedJudge::always(RoundVerdict::Progress);
        let driver = GoalDriver::new(&runner, &judge, 50);

        let run = driver.run(&mut goal, clock()).unwrap();

        assert_eq!(run.steps.len(), 1);
        assert_eq!(run.stop, DriverStop::AwaitingApproval);
        assert_eq!(goal.status, GoalStatus::AwaitingApproval);
    }

    #[test]
    fn runner_failure_does_not_record_a_round() {
        let mut goal = active_goal(GoalBudget::default());
        let runner = ScriptedRunner::failing();
        let judge = ScriptedJudge::always(RoundVerdict::Progress);
        let driver = GoalDriver::new(&runner, &judge, 5);

        let error = driver.run(&mut goal, clock()).unwrap_err();

        assert!(matches!(error, DriverError::Runner(_)));
        assert!(goal.rounds.is_empty());
        assert_eq!(goal.status, GoalStatus::Active);
    }

    #[test]
    fn inactive_goal_is_rejected() {
        let mut goal = Goal::new("g1", "grp", "x", GoalBudget::default(), 0);
        let runner = ScriptedRunner::new(1);
        let judge = ScriptedJudge::always(RoundVerdict::Progress);
        let driver = GoalDriver::new(&runner, &judge, 5);

        let error = driver.step(&mut goal, 1).unwrap_err();

        assert_eq!(error, DriverError::NotActive(GoalStatus::Draft));
    }

    #[test]
    fn prompt_is_deterministic_and_mentions_progress() {
        let mut goal = active_goal(GoalBudget {
            max_rounds: 4,
            ..GoalBudget::default()
        });
        let first = build_round_prompt(&goal);
        assert!(first.contains("[ROUND] 第 1 轮（上限 4）"));
        assert!(first.contains("这是第一轮"));

        goal.record_round(RoundVerdict::Progress, 1, 10).unwrap();
        let second = build_round_prompt(&goal);
        assert!(second.contains("[ROUND] 第 2 轮"));
        assert!(second.contains("Progress"));
    }

    #[test]
    fn parses_explicit_verdict_markers() {
        assert_eq!(
            parse_verdict_marker("工作完成\n[GOAL:MET]"),
            Some(RoundVerdict::GoalMet)
        );
        assert_eq!(
            parse_verdict_marker("还在做 [GOAL:PROGRESS]"),
            Some(RoundVerdict::Progress)
        );
        assert_eq!(
            parse_verdict_marker("[goal:failed] 被阻塞"),
            Some(RoundVerdict::Failed)
        );
        assert_eq!(parse_verdict_marker("没有标记"), None);
    }

    #[test]
    fn last_verdict_marker_wins() {
        // Agent 可能先引用格式再给结论，以最后一次出现为准。
        let text = "格式示例 [GOAL:MET]，但本轮实际上：[GOAL:PROGRESS]";
        assert_eq!(parse_verdict_marker(text), Some(RoundVerdict::Progress));
    }

    #[test]
    fn round_prompt_requires_a_verdict_marker() {
        let goal = Goal::new("g1", "grp", "做点事", GoalBudget::default(), 0);
        let prompt = build_round_prompt(&goal);

        assert!(prompt.contains(VERDICT_MET));
        assert!(prompt.contains(VERDICT_PROGRESS));
        assert!(prompt.contains(VERDICT_FAILED));
        assert!(prompt.contains("最后一行"));
    }
}
