//! 派发式 runner：把「发一轮请求 → 等结果」抽象成可替换的 dispatcher。
//!
//! 真实的 dispatcher 会写消息并触发现有调度；这里只负责轮询语义、超时与
//! 观察结果压缩，保证这些边界能在没有 Agent 的情况下被测到。

use std::time::Duration;

use super::driver::{build_round_prompt, GoalRunner, RoundObservation};
use super::Goal;

/// 派发后拿到的句柄：一条消息 + 若干 run。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchHandle {
    pub message_id: String,
    pub run_ids: Vec<String>,
}

/// 一次派发的当前状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchStatus {
    Pending,
    Completed { output: String, tokens_used: u64 },
    Failed { error: String },
    Canceled,
}

/// 发消息并查询 run 结果。
pub trait GoalDispatcher {
    fn dispatch(&self, goal: &Goal, prompt: &str) -> Result<DispatchHandle, String>;
    fn poll(&self, handle: &DispatchHandle) -> Result<DispatchStatus, String>;
}

/// 观察摘要上限：证据账本里只留可读结论，不塞整段输出。
pub const MAX_SUMMARY_CHARS: usize = 2000;

fn summarize(output: &str) -> String {
    let trimmed = output.trim();
    let count = trimmed.chars().count();
    if count <= MAX_SUMMARY_CHARS {
        return trimmed.to_string();
    }
    let mut summary: String = trimmed.chars().take(MAX_SUMMARY_CHARS).collect();
    summary.push('…');
    summary
}

/// 轮询式 runner。
pub struct DispatchingRunner<D: GoalDispatcher> {
    dispatcher: D,
    max_polls: u32,
    poll_interval: Duration,
}

impl<D: GoalDispatcher> DispatchingRunner<D> {
    pub fn new(dispatcher: D, max_polls: u32, poll_interval: Duration) -> Self {
        Self {
            dispatcher,
            max_polls: max_polls.max(1),
            poll_interval,
        }
    }
}

/// 让 `&D` 也能当 dispatcher 用，便于测试里保留所有权。
impl<T: GoalDispatcher + ?Sized> GoalDispatcher for &T {
    fn dispatch(&self, goal: &Goal, prompt: &str) -> Result<DispatchHandle, String> {
        (**self).dispatch(goal, prompt)
    }

    fn poll(&self, handle: &DispatchHandle) -> Result<DispatchStatus, String> {
        (**self).poll(handle)
    }
}

impl<D: GoalDispatcher> GoalRunner for DispatchingRunner<D> {
    fn run_round(&self, goal: &Goal, _round_index: u32) -> Result<RoundObservation, String> {
        let prompt = build_round_prompt(goal);
        let handle = self.dispatcher.dispatch(goal, &prompt)?;

        for _ in 0..self.max_polls {
            match self.dispatcher.poll(&handle)? {
                DispatchStatus::Pending => {
                    if !self.poll_interval.is_zero() {
                        std::thread::sleep(self.poll_interval);
                    }
                }
                DispatchStatus::Completed {
                    output,
                    tokens_used,
                } => {
                    return Ok(RoundObservation {
                        summary: summarize(&output),
                        tokens_used,
                    })
                }
                DispatchStatus::Failed { error } => {
                    return Err(format!("run 失败：{error}"));
                }
                DispatchStatus::Canceled => return Err("run 已取消".into()),
            }
        }
        Err(format!(
            "等待 run 超时（轮询 {} 次，消息 {}）",
            self.max_polls, handle.message_id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goals::{GoalBudget, RoundVerdict};
    use std::cell::RefCell;

    struct ScriptedDispatcher {
        statuses: RefCell<Vec<DispatchStatus>>,
        dispatched_prompt: RefCell<Option<String>>,
        dispatch_error: bool,
    }

    impl ScriptedDispatcher {
        fn new(statuses: Vec<DispatchStatus>) -> Self {
            Self {
                statuses: RefCell::new(statuses),
                dispatched_prompt: RefCell::new(None),
                dispatch_error: false,
            }
        }

        fn failing_dispatch() -> Self {
            Self {
                statuses: RefCell::new(Vec::new()),
                dispatched_prompt: RefCell::new(None),
                dispatch_error: true,
            }
        }
    }

    impl GoalDispatcher for ScriptedDispatcher {
        fn dispatch(&self, _goal: &Goal, prompt: &str) -> Result<DispatchHandle, String> {
            if self.dispatch_error {
                return Err("dispatch failed".into());
            }
            *self.dispatched_prompt.borrow_mut() = Some(prompt.to_string());
            Ok(DispatchHandle {
                message_id: "msg-1".into(),
                run_ids: vec!["run-1".into()],
            })
        }

        fn poll(&self, _handle: &DispatchHandle) -> Result<DispatchStatus, String> {
            let mut statuses = self.statuses.borrow_mut();
            if statuses.is_empty() {
                return Ok(DispatchStatus::Pending);
            }
            Ok(statuses.remove(0))
        }
    }

    fn goal() -> Goal {
        let mut goal = Goal::new("g1", "grp", "把首页画出来", GoalBudget::default(), 0);
        goal.start(0).unwrap();
        goal
    }

    #[test]
    fn completes_after_polling() {
        let dispatcher = ScriptedDispatcher::new(vec![
            DispatchStatus::Pending,
            DispatchStatus::Completed {
                output: "已经完成三栏布局".into(),
                tokens_used: 120,
            },
        ]);
        let runner = DispatchingRunner::new(dispatcher, 5, Duration::ZERO);

        let observation = runner.run_round(&goal(), 0).unwrap();

        assert_eq!(observation.summary, "已经完成三栏布局");
        assert_eq!(observation.tokens_used, 120);
    }

    #[test]
    fn dispatch_receives_the_prompt() {
        let dispatcher = ScriptedDispatcher::new(vec![DispatchStatus::Completed {
            output: "ok".into(),
            tokens_used: 1,
        }]);
        let runner = DispatchingRunner::new(&dispatcher, 1, Duration::ZERO);

        runner.run_round(&goal(), 0).unwrap();

        let prompt = dispatcher.dispatched_prompt.borrow().clone().unwrap();
        assert!(prompt.contains("[GOAL] 把首页画出来"));
    }

    #[test]
    fn failed_run_is_an_error() {
        let dispatcher = ScriptedDispatcher::new(vec![DispatchStatus::Failed {
            error: "agent crashed".into(),
        }]);
        let runner = DispatchingRunner::new(dispatcher, 3, Duration::ZERO);

        let error = runner.run_round(&goal(), 0).err().unwrap();

        assert!(error.contains("run 失败"));
        assert!(error.contains("agent crashed"));
    }

    #[test]
    fn canceled_run_is_an_error() {
        let dispatcher = ScriptedDispatcher::new(vec![DispatchStatus::Canceled]);
        let runner = DispatchingRunner::new(dispatcher, 3, Duration::ZERO);

        let error = runner.run_round(&goal(), 0).err().unwrap();

        assert!(error.contains("已取消"));
    }

    #[test]
    fn pending_forever_times_out() {
        let dispatcher = ScriptedDispatcher::new(vec![]);
        let runner = DispatchingRunner::new(dispatcher, 3, Duration::ZERO);

        let error = runner.run_round(&goal(), 0).err().unwrap();

        assert!(error.contains("超时"));
        assert!(error.contains("msg-1"));
    }

    #[test]
    fn dispatch_failure_propagates() {
        let runner =
            DispatchingRunner::new(ScriptedDispatcher::failing_dispatch(), 3, Duration::ZERO);

        let error = runner.run_round(&goal(), 0).err().unwrap();

        assert!(error.contains("dispatch failed"));
    }

    #[test]
    fn long_output_is_truncated() {
        let dispatcher = ScriptedDispatcher::new(vec![DispatchStatus::Completed {
            output: "x".repeat(MAX_SUMMARY_CHARS + 50),
            tokens_used: 1,
        }]);
        let runner = DispatchingRunner::new(dispatcher, 1, Duration::ZERO);

        let observation = runner.run_round(&goal(), 0).unwrap();

        assert_eq!(observation.summary.chars().count(), MAX_SUMMARY_CHARS + 1);
        assert!(observation.summary.ends_with('…'));
        let _ = RoundVerdict::Progress;
    }
}
