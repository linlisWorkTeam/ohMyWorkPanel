//! 群组级 Goal：自治循环的领域模型与停止条件。
//!
//! 设计原则：**循环可以无限跑，但预算不是无限的**。任何一次 `record_round`
//! 之后，只要触碰轮次、时长、token、无进展、连续失败中的任意一条，就必须停。
//! 这个模块不碰数据库、不调度 Agent，纯粹决定「该不该继续」。

pub mod store;

use serde::{Deserialize, Serialize};

pub type Timestamp = i64;

/// 预算与护栏。`0` 表示该项不限制，其余为硬上限。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalBudget {
    pub max_rounds: u32,
    pub max_wall_time_ms: u64,
    pub max_tokens: u64,
    /// 连续多少轮「没有进展」就停。
    pub max_no_progress_rounds: u32,
    /// 连续多少次失败就停。
    pub max_consecutive_failures: u32,
}

impl Default for GoalBudget {
    fn default() -> Self {
        Self {
            max_rounds: 20,
            max_wall_time_ms: 6 * 60 * 60 * 1000,
            max_tokens: 2_000_000,
            max_no_progress_rounds: 3,
            max_consecutive_failures: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Draft,
    Active,
    Paused,
    AwaitingApproval,
    Completed,
    Aborted,
}

impl GoalStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Aborted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundVerdict {
    Progress,
    NoProgress,
    Failed,
    /// 判定目标已达成，立即终止。
    GoalMet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    GoalMet,
    RoundsExhausted,
    WallTimeExhausted,
    TokensExhausted,
    NoProgress,
    ConsecutiveFailures,
    AbortedByHuman,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalRound {
    pub index: u32,
    pub verdict: RoundVerdict,
    pub tokens_used: u64,
    pub finished_at: Timestamp,
}

/// 记录一轮之后引擎给出的下一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundOutcome {
    Continue,
    AwaitApproval,
    Stopped(StopReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalError {
    NotActive,
    NotAwaitingApproval,
    Terminal,
}

impl std::fmt::Display for GoalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotActive => write!(f, "目标未处于运行中"),
            Self::NotAwaitingApproval => write!(f, "目标不在等待审批状态"),
            Self::Terminal => write!(f, "目标已结束，不能再变更"),
        }
    }
}

impl std::error::Error for GoalError {}

impl From<GoalError> for String {
    fn from(error: GoalError) -> Self {
        error.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub group_id: String,
    pub objective: String,
    pub budget: GoalBudget,
    pub status: GoalStatus,
    pub created_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub rounds: Vec<GoalRound>,
    pub stop_reason: Option<StopReason>,
    /// 每轮结束后是否需要人类确认才能继续。
    pub require_approval_each_round: bool,
}

impl Goal {
    pub fn new(
        id: impl Into<String>,
        group_id: impl Into<String>,
        objective: impl Into<String>,
        budget: GoalBudget,
        now: Timestamp,
    ) -> Self {
        Self {
            id: id.into(),
            group_id: group_id.into(),
            objective: objective.into(),
            budget,
            status: GoalStatus::Draft,
            created_at: now,
            started_at: None,
            rounds: Vec::new(),
            stop_reason: None,
            require_approval_each_round: false,
        }
    }

    pub fn with_round_approval(mut self) -> Self {
        self.require_approval_each_round = true;
        self
    }

    pub fn total_tokens(&self) -> u64 {
        self.rounds.iter().map(|round| round.tokens_used).sum()
    }

    /// 启动或从暂停恢复。
    pub fn start(&mut self, now: Timestamp) -> Result<(), GoalError> {
        match self.status {
            GoalStatus::Draft => {
                self.status = GoalStatus::Active;
                self.started_at = Some(now);
                Ok(())
            }
            GoalStatus::Paused => {
                self.status = GoalStatus::Active;
                Ok(())
            }
            GoalStatus::Completed | GoalStatus::Aborted => Err(GoalError::Terminal),
            _ => Err(GoalError::NotActive),
        }
    }

    pub fn pause(&mut self) -> Result<(), GoalError> {
        if self.status != GoalStatus::Active {
            return Err(GoalError::NotActive);
        }
        self.status = GoalStatus::Paused;
        Ok(())
    }

    /// 人类确认后继续下一轮。
    pub fn approve(&mut self) -> Result<(), GoalError> {
        if self.status != GoalStatus::AwaitingApproval {
            return Err(GoalError::NotAwaitingApproval);
        }
        self.status = GoalStatus::Active;
        Ok(())
    }

    pub fn abort_by_human(&mut self) -> Result<(), GoalError> {
        if self.status.is_terminal() {
            return Err(GoalError::Terminal);
        }
        self.status = GoalStatus::Aborted;
        self.stop_reason = Some(StopReason::AbortedByHuman);
        Ok(())
    }

    /// 记录一轮并判断下一步。只有 `Active` 状态可以记录。
    pub fn record_round(
        &mut self,
        verdict: RoundVerdict,
        tokens_used: u64,
        now: Timestamp,
    ) -> Result<RoundOutcome, GoalError> {
        if self.status.is_terminal() {
            return Err(GoalError::Terminal);
        }
        if self.status != GoalStatus::Active {
            return Err(GoalError::NotActive);
        }

        self.rounds.push(GoalRound {
            index: self.rounds.len() as u32,
            verdict,
            tokens_used,
            finished_at: now,
        });

        if verdict == RoundVerdict::GoalMet {
            self.finish(GoalStatus::Completed, StopReason::GoalMet);
            return Ok(RoundOutcome::Stopped(StopReason::GoalMet));
        }
        if let Some(reason) = self.exhausted(now) {
            self.finish(GoalStatus::Aborted, reason);
            return Ok(RoundOutcome::Stopped(reason));
        }
        if self.require_approval_each_round {
            self.status = GoalStatus::AwaitingApproval;
            return Ok(RoundOutcome::AwaitApproval);
        }
        Ok(RoundOutcome::Continue)
    }

    /// 不修改状态，供 UI 或调度器提前展示「下一轮会被哪条护栏挡住」。
    pub fn exhausted(&self, now: Timestamp) -> Option<StopReason> {
        let budget = &self.budget;
        if budget.max_rounds > 0 && self.rounds.len() as u32 >= budget.max_rounds {
            return Some(StopReason::RoundsExhausted);
        }
        if budget.max_wall_time_ms > 0 {
            if let Some(started_at) = self.started_at {
                let elapsed = now.saturating_sub(started_at) as u64;
                if elapsed >= budget.max_wall_time_ms {
                    return Some(StopReason::WallTimeExhausted);
                }
            }
        }
        if budget.max_tokens > 0 && self.total_tokens() >= budget.max_tokens {
            return Some(StopReason::TokensExhausted);
        }
        if budget.max_no_progress_rounds > 0
            && self.trailing_no_progress() >= budget.max_no_progress_rounds
        {
            return Some(StopReason::NoProgress);
        }
        if budget.max_consecutive_failures > 0
            && self.trailing_failures() >= budget.max_consecutive_failures
        {
            return Some(StopReason::ConsecutiveFailures);
        }
        None
    }

    fn finish(&mut self, status: GoalStatus, reason: StopReason) {
        self.status = status;
        self.stop_reason = Some(reason);
    }

    fn trailing_no_progress(&self) -> u32 {
        self.rounds
            .iter()
            .rev()
            .take_while(|round| round.verdict == RoundVerdict::NoProgress)
            .count() as u32
    }

    fn trailing_failures(&self) -> u32 {
        self.rounds
            .iter()
            .rev()
            .take_while(|round| round.verdict == RoundVerdict::Failed)
            .count() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goal(budget: GoalBudget) -> Goal {
        Goal::new("g1", "grp", "把路修好", budget, 1_000)
    }

    fn active(budget: GoalBudget) -> Goal {
        let mut goal = goal(budget);
        goal.start(1_000).unwrap();
        goal
    }

    #[test]
    fn starts_as_draft_and_becomes_active() {
        let mut goal = goal(GoalBudget::default());
        assert_eq!(goal.status, GoalStatus::Draft);
        goal.start(1_000).unwrap();
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(goal.started_at, Some(1_000));
    }

    #[test]
    fn goal_met_finishes_immediately() {
        let mut goal = active(GoalBudget::default());
        let outcome = goal.record_round(RoundVerdict::GoalMet, 10, 2_000).unwrap();
        assert_eq!(outcome, RoundOutcome::Stopped(StopReason::GoalMet));
        assert_eq!(goal.status, GoalStatus::Completed);
        assert_eq!(goal.stop_reason, Some(StopReason::GoalMet));
    }

    #[test]
    fn round_budget_stops_the_loop() {
        let mut goal = active(GoalBudget {
            max_rounds: 2,
            ..GoalBudget::default()
        });
        assert_eq!(
            goal.record_round(RoundVerdict::Progress, 1, 2_000).unwrap(),
            RoundOutcome::Continue
        );
        assert_eq!(
            goal.record_round(RoundVerdict::Progress, 1, 3_000).unwrap(),
            RoundOutcome::Stopped(StopReason::RoundsExhausted)
        );
        assert_eq!(goal.status, GoalStatus::Aborted);
    }

    #[test]
    fn wall_time_budget_stops_the_loop() {
        let mut goal = active(GoalBudget {
            max_wall_time_ms: 1_000,
            ..GoalBudget::default()
        });
        let outcome = goal.record_round(RoundVerdict::Progress, 1, 2_000).unwrap();
        assert_eq!(
            outcome,
            RoundOutcome::Stopped(StopReason::WallTimeExhausted)
        );
    }

    #[test]
    fn token_budget_stops_the_loop() {
        let mut goal = active(GoalBudget {
            max_tokens: 100,
            ..GoalBudget::default()
        });
        let outcome = goal
            .record_round(RoundVerdict::Progress, 100, 2_000)
            .unwrap();
        assert_eq!(outcome, RoundOutcome::Stopped(StopReason::TokensExhausted));
        assert_eq!(goal.total_tokens(), 100);
    }

    #[test]
    fn repeated_no_progress_stops_the_loop() {
        let mut goal = active(GoalBudget {
            max_no_progress_rounds: 2,
            ..GoalBudget::default()
        });
        assert_eq!(
            goal.record_round(RoundVerdict::NoProgress, 1, 2_000)
                .unwrap(),
            RoundOutcome::Continue
        );
        assert_eq!(
            goal.record_round(RoundVerdict::NoProgress, 1, 3_000)
                .unwrap(),
            RoundOutcome::Stopped(StopReason::NoProgress)
        );
    }

    #[test]
    fn progress_resets_the_no_progress_streak() {
        let mut goal = active(GoalBudget {
            max_no_progress_rounds: 2,
            ..GoalBudget::default()
        });
        goal.record_round(RoundVerdict::NoProgress, 1, 2_000)
            .unwrap();
        goal.record_round(RoundVerdict::Progress, 1, 3_000).unwrap();
        let outcome = goal
            .record_round(RoundVerdict::NoProgress, 1, 4_000)
            .unwrap();
        assert_eq!(outcome, RoundOutcome::Continue);
    }

    #[test]
    fn consecutive_failures_stop_the_loop() {
        let mut goal = active(GoalBudget {
            max_consecutive_failures: 2,
            ..GoalBudget::default()
        });
        goal.record_round(RoundVerdict::Failed, 1, 2_000).unwrap();
        assert_eq!(
            goal.record_round(RoundVerdict::Failed, 1, 3_000).unwrap(),
            RoundOutcome::Stopped(StopReason::ConsecutiveFailures)
        );
    }

    #[test]
    fn approval_gate_blocks_until_human_approves() {
        let mut goal = active(GoalBudget::default()).with_round_approval();
        assert_eq!(
            goal.record_round(RoundVerdict::Progress, 1, 2_000).unwrap(),
            RoundOutcome::AwaitApproval
        );
        assert_eq!(goal.status, GoalStatus::AwaitingApproval);
        goal.approve().unwrap();
        assert_eq!(goal.status, GoalStatus::Active);
    }

    #[test]
    fn paused_goal_cannot_record_rounds() {
        let mut goal = active(GoalBudget::default());
        goal.pause().unwrap();
        assert_eq!(
            goal.record_round(RoundVerdict::Progress, 1, 2_000),
            Err(GoalError::NotActive)
        );
    }

    #[test]
    fn terminal_goal_rejects_further_changes() {
        let mut goal = active(GoalBudget::default());
        goal.record_round(RoundVerdict::GoalMet, 1, 2_000).unwrap();
        assert_eq!(
            goal.record_round(RoundVerdict::Progress, 1, 3_000),
            Err(GoalError::Terminal)
        );
        assert_eq!(goal.abort_by_human(), Err(GoalError::Terminal));
    }

    #[test]
    fn goal_round_trips_through_json() {
        let mut goal = active(GoalBudget::default());
        goal.record_round(RoundVerdict::Progress, 42, 2_000)
            .unwrap();
        let raw = serde_json::to_string(&goal).unwrap();
        let parsed: Goal = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed, goal);
    }
}
