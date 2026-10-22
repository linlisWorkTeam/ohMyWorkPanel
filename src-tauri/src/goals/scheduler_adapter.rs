//! 真实调度适配：把一轮 goal 写成群消息、建 run、触发调度，再轮询状态。
//!
//! 触发调度的方式通过闭包注入（真实实现传 `schedule_group`，测试传空操作），
//! 这样适配层本身可以在不启动任何 Agent 的情况下验证消息/run/状态映射。

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::OptionalExtension;

use crate::db::{create_task_run, get_group, id, now, open_db, AppResult};
use crate::models::Message;

use super::dispatch::{DispatchHandle, DispatchStatus, DispatchingRunner, GoalDispatcher};
use super::driver::{parse_verdict_marker, GoalDriver, GoalJudge, RoundObservation};
use super::{Goal, RoundVerdict};

/// 默认轮询间隔与上限：500ms × 600 ≈ 5 分钟（真实 CLI 首轮常常更慢）。
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);
pub const DEFAULT_MAX_POLLS: u32 = 600;

/// 触发一次调度；真实实现通常是 `schedule_group`。
pub type KickFn = Box<dyn Fn(&str) + Send + Sync>;

pub struct SchedulerDispatcher {
    db_path: PathBuf,
    group_id: String,
    /// 由哪个成员执行这一轮（通常是群里的主 Agent）。
    driver_member_id: String,
    /// 消息的发送者（通常是群主；调度器依赖成员存在性）。
    sender_member_id: String,
    kick: KickFn,
}

impl SchedulerDispatcher {
    pub fn new(
        db_path: PathBuf,
        group_id: impl Into<String>,
        driver_member_id: impl Into<String>,
        sender_member_id: impl Into<String>,
        kick: KickFn,
    ) -> Self {
        Self {
            db_path,
            group_id: group_id.into(),
            driver_member_id: driver_member_id.into(),
            sender_member_id: sender_member_id.into(),
            kick,
        }
    }
}

impl GoalDispatcher for SchedulerDispatcher {
    fn dispatch(&self, _goal: &Goal, prompt: &str) -> Result<DispatchHandle, String> {
        let content = prompt.trim();
        if content.is_empty() {
            return Err("prompt 为空".into());
        }
        let conn = open_db(&self.db_path)?;
        let group = get_group(&conn, &self.group_id)?;

        let sender_active: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND is_active=1",
                rusqlite::params![self.sender_member_id, self.group_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if sender_active != 1 {
            return Err("发送者不属于该群或已被移除".into());
        }

        let message = Message {
            id: id(),
            group_id: self.group_id.clone(),
            sender_member_id: self.sender_member_id.clone(),
            parent_run_id: None,
            content: content.to_string(),
            status: "completed".into(),
            created_at: now(),
            has_thinking: false,
            has_artifact: false,
        };
        conn.execute(
            "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) \
             VALUES(?1,?2,?3,NULL,?4,'completed',?5)",
            rusqlite::params![
                message.id,
                message.group_id,
                message.sender_member_id,
                message.content,
                message.created_at
            ],
        )
        .map_err(|error| error.to_string())?;

        let run_id = create_task_run(
            &conn,
            &group.id,
            &message.id,
            &self.driver_member_id,
            None,
            0,
        )?;
        drop(conn);

        (self.kick)(&self.group_id);

        Ok(DispatchHandle {
            message_id: message.id,
            run_ids: vec![run_id],
        })
    }

    fn poll(&self, handle: &DispatchHandle) -> Result<DispatchStatus, String> {
        let conn = open_db(&self.db_path)?;
        // v0 一次只派发一个 run；多 run 聚合留到后续切片。
        let Some(run_id) = handle.run_ids.first() else {
            return Ok(DispatchStatus::Pending);
        };
        let row = conn
            .query_row(
                "SELECT status, output_message_id, error_message FROM task_runs WHERE id=?1",
                [run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?;

        let Some((status, output_message_id, error_message)) = row else {
            return Err(format!("run 不存在：{run_id}"));
        };
        match status.as_str() {
            "completed" => {
                let output = match output_message_id {
                    Some(message_id) => conn
                        .query_row(
                            "SELECT content FROM messages WHERE id=?1",
                            [message_id],
                            |row| row.get::<_, String>(0),
                        )
                        .optional()
                        .map_err(|error| error.to_string())?
                        .unwrap_or_default(),
                    None => String::new(),
                };
                // 流式消息的内容是 channel parts JSON；证据账本只存最终文本。
                let output = final_text_from_content(&output);
                Ok(DispatchStatus::Completed {
                    output,
                    // 真实 token 计量后续接入；当前不虚报。
                    tokens_used: 0,
                })
            }
            "failed" => Ok(DispatchStatus::Failed {
                error: error_message.unwrap_or_else(|| "未提供错误信息".into()),
            }),
            "canceled" => Ok(DispatchStatus::Canceled),
            _ => Ok(DispatchStatus::Pending),
        }
    }
}

/// 把消息内容投影成最终文本：优先 final 频道，其次纯文本，最后原样返回。
fn final_text_from_content(raw: &str) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    let final_text = crate::message_content::extract_channel_text(raw, "final");
    if !final_text.trim().is_empty() {
        return final_text;
    }
    let plain = crate::message_content::parts_to_plain_text(raw);
    if plain.trim().is_empty() {
        raw.to_string()
    } else {
        plain
    }
}

/// MVP 的确定性判定：先按关键词，后续替换为 Agent 判定。
///
/// 关键词只覆盖「明确完成 / 明确失败 / 空输出」三类，其余按有进展处理。
pub struct HeuristicJudge;

impl GoalJudge for HeuristicJudge {
    fn judge(&self, _goal: &Goal, observation: &RoundObservation) -> RoundVerdict {
        let text = observation.summary.trim();
        if text.is_empty() {
            return RoundVerdict::NoProgress;
        }
        // 显式标记优先：Agent 被要求在最后一行给出判决。
        if let Some(verdict) = parse_verdict_marker(text) {
            return verdict;
        }
        let lower = text.to_lowercase();
        let done = text.contains("目标达成")
            || text.contains("已全部完成")
            || text.contains("全部完成")
            || lower.contains("goal met");
        if done {
            return RoundVerdict::GoalMet;
        }
        if text.contains("执行失败") || lower.contains("error:") || lower.contains("failed") {
            return RoundVerdict::Failed;
        }
        RoundVerdict::Progress
    }
}

/// 跑一轮：派发消息 → 轮询 run → 判定 → 落库。
pub fn tick(
    db_path: &Path,
    goal_id: &str,
    dispatcher: SchedulerDispatcher,
    max_polls: u32,
) -> AppResult<super::runtime::TickReport> {
    let runner = DispatchingRunner::new(dispatcher, max_polls, DEFAULT_POLL_INTERVAL);
    let driver = GoalDriver::new(&runner, &HeuristicJudge, 1);
    super::runtime::tick(db_path, goal_id, &driver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::goals::{service, GoalBudget, GoalStatus, StopReason};
    use std::sync::{Arc, Mutex};
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
        conn.execute(
            "INSERT INTO members (id, group_id, kind, display_name, avatar_color, role_description, is_active, created_at)
             VALUES ('m-owner', 'g', 'user', 'Owner', 'c', '', 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members (id, group_id, kind, display_name, avatar_color, role_description, is_active, created_at)
             VALUES ('m-agent', 'g', 'agent', 'Agent', 'c', '', 1, 1)",
            [],
        )
        .unwrap();
        file
    }

    fn dispatcher(file: &NamedTempFile, kicked: Arc<Mutex<Vec<String>>>) -> SchedulerDispatcher {
        SchedulerDispatcher::new(
            file.path().to_path_buf(),
            "g",
            "m-agent",
            "m-owner",
            Box::new(move |group_id| {
                kicked.lock().unwrap().push(group_id.to_string());
            }),
        )
    }

    fn goal() -> Goal {
        Goal::new("goal-1", "g", "把首页画出来", Default::default(), 0)
    }

    #[test]
    fn dispatch_writes_message_and_run_then_kicks() {
        let file = db();
        let kicked = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = dispatcher(&file, Arc::clone(&kicked));

        let handle = dispatcher.dispatch(&goal(), "[GOAL] demo").unwrap();

        assert_eq!(handle.run_ids.len(), 1);
        assert_eq!(*kicked.lock().unwrap(), vec!["g"]);
        let conn = open_db(file.path()).unwrap();
        let content: String = conn
            .query_row(
                "SELECT content FROM messages WHERE id=?1",
                [&handle.message_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(content, "[GOAL] demo");
        let status: String = conn
            .query_row(
                "SELECT status FROM task_runs WHERE id=?1",
                [&handle.run_ids[0]],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "queued");
    }

    #[test]
    fn pending_until_the_run_completes() {
        let file = db();
        let dispatcher = dispatcher(&file, Arc::new(Mutex::new(Vec::new())));
        let handle = dispatcher.dispatch(&goal(), "prompt").unwrap();

        assert_eq!(dispatcher.poll(&handle).unwrap(), DispatchStatus::Pending);

        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO messages (id, group_id, sender_member_id, content, status, created_at) \
             VALUES ('m-out', 'g', 'm-agent', '三栏布局完成', 'completed', 2)",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE task_runs SET status='completed', output_message_id='m-out' WHERE id=?1",
            [&handle.run_ids[0]],
        )
        .unwrap();

        assert_eq!(
            dispatcher.poll(&handle).unwrap(),
            DispatchStatus::Completed {
                output: "三栏布局完成".into(),
                tokens_used: 0
            }
        );
    }

    #[test]
    fn completed_output_prefers_final_channel_text() {
        let file = db();
        let dispatcher = dispatcher(&file, Arc::new(Mutex::new(Vec::new())));
        let handle = dispatcher.dispatch(&goal(), "prompt").unwrap();
        let conn = open_db(file.path()).unwrap();
        let content = r#"{"v":1,"parts":[{"channel":"thinking","text":"先想想"},{"channel":"final","text":"最终结论"}]}"#;
        conn.execute(
            "INSERT INTO messages (id, group_id, sender_member_id, content, status, created_at) \
             VALUES ('m-out', 'g', 'm-agent', ?1, 'completed', 2)",
            [content],
        )
        .unwrap();
        conn.execute(
            "UPDATE task_runs SET status='completed', output_message_id='m-out' WHERE id=?1",
            [&handle.run_ids[0]],
        )
        .unwrap();

        assert_eq!(
            dispatcher.poll(&handle).unwrap(),
            DispatchStatus::Completed {
                output: "最终结论".into(),
                tokens_used: 0
            }
        );
    }

    #[test]
    fn plain_output_is_passed_through() {
        assert_eq!(final_text_from_content("普通文本"), "普通文本");
        assert_eq!(final_text_from_content(""), "");
    }

    #[test]
    fn failed_run_maps_to_failed_status() {
        let file = db();
        let dispatcher = dispatcher(&file, Arc::new(Mutex::new(Vec::new())));
        let handle = dispatcher.dispatch(&goal(), "prompt").unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "UPDATE task_runs SET status='failed', error_message='boom' WHERE id=?1",
            [&handle.run_ids[0]],
        )
        .unwrap();

        assert_eq!(
            dispatcher.poll(&handle).unwrap(),
            DispatchStatus::Failed {
                error: "boom".into()
            }
        );
    }

    #[test]
    fn unknown_run_is_an_error() {
        let file = db();
        let dispatcher = dispatcher(&file, Arc::new(Mutex::new(Vec::new())));
        let handle = DispatchHandle {
            message_id: "x".into(),
            run_ids: vec!["missing".into()],
        };

        assert!(dispatcher.poll(&handle).is_err());
    }

    #[test]
    fn sender_must_belong_to_the_group() {
        let file = db();
        let bad = SchedulerDispatcher::new(
            file.path().to_path_buf(),
            "g",
            "m-agent",
            "m-stranger",
            Box::new(|_| {}),
        );

        let error = bad.dispatch(&goal(), "prompt").err().unwrap();

        assert!(error.contains("发送者不属于该群"));
    }

    #[test]
    fn heuristic_judge_maps_keywords() {
        let judge = HeuristicJudge;
        let done = RoundObservation {
            summary: "全部完成".into(),
            tokens_used: 1,
        };
        let failed = RoundObservation {
            summary: "执行失败：timeout".into(),
            tokens_used: 1,
        };
        let empty = RoundObservation {
            summary: "  ".into(),
            tokens_used: 0,
        };
        let progress = RoundObservation {
            summary: "完成了一半".into(),
            tokens_used: 1,
        };
        let goal = goal();
        assert_eq!(judge.judge(&goal, &done), RoundVerdict::GoalMet);
        assert_eq!(judge.judge(&goal, &failed), RoundVerdict::Failed);
        assert_eq!(judge.judge(&goal, &empty), RoundVerdict::NoProgress);
        assert_eq!(judge.judge(&goal, &progress), RoundVerdict::Progress);
    }

    #[test]
    fn end_to_end_tick_completes_the_goal() {
        let file = db();
        let goal = service::create_goal(
            file.path(),
            "g",
            "把首页画出来",
            GoalBudget::default(),
            false,
        )
        .unwrap();
        service::start_goal(file.path(), &goal.id).unwrap();

        let path = file.path().to_path_buf();
        let dispatcher = SchedulerDispatcher::new(
            path.clone(),
            "g",
            "m-agent",
            "m-owner",
            Box::new(move |_group_id| {
                // 模拟调度器跑完：把最新 queued run 标成 completed 并挂上输出消息。
                let conn = open_db(&path).unwrap();
                let run_id: String = conn
                    .query_row(
                        "SELECT id FROM task_runs WHERE status='queued' ORDER BY created_at DESC LIMIT 1",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                conn.execute(
                    "INSERT INTO messages (id, group_id, sender_member_id, content, status, created_at) \
                     VALUES ('m-out', 'g', 'm-agent', '全部完成', 'completed', 2)",
                    [],
                )
                .unwrap();
                conn.execute(
                    "UPDATE task_runs SET status='completed', output_message_id='m-out' WHERE id=?1",
                    [&run_id],
                )
                .unwrap();
            }),
        );

        let report = tick(file.path(), &goal.id, dispatcher, 3).unwrap();

        assert_eq!(report.goal.status, GoalStatus::Completed);
        assert_eq!(report.step.summary, "全部完成");
        let loaded = service::get_goal(file.path(), &goal.id).unwrap().unwrap();
        assert_eq!(loaded.stop_reason, Some(StopReason::GoalMet));
        assert_eq!(loaded.rounds.len(), 1);
    }

    #[test]
    fn judge_prefers_explicit_marker_over_keywords() {
        let judge = HeuristicJudge;
        let goal = goal();

        let observation = RoundObservation {
            summary: "全部完成，但其实还要继续 [GOAL:PROGRESS]".into(),
            tokens_used: 10,
        };
        assert_eq!(judge.judge(&goal, &observation), RoundVerdict::Progress);

        let observation = RoundObservation {
            summary: "什么都没做 [GOAL:MET]".into(),
            tokens_used: 10,
        };
        assert_eq!(judge.judge(&goal, &observation), RoundVerdict::GoalMet);
    }

    #[test]
    fn judge_falls_back_to_keywords_without_marker() {
        let judge = HeuristicJudge;
        let goal = goal();

        let observation = RoundObservation {
            summary: "全部完成".into(),
            tokens_used: 10,
        };
        assert_eq!(judge.judge(&goal, &observation), RoundVerdict::GoalMet);
    }
}
