//! Goal IPC 命令（P3 自治循环）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use crate::db::AppResult;
use crate::scheduler::schedule_group;
use crate::AppState;
use tauri::{AppHandle, State};

use super::to_scheduler;

// === 群组级 Goal（P3 自治循环） ===

#[tauri::command]
pub fn list_goals(
    group_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<crate::goals::Goal>> {
    crate::goals::service::list_goals(&state.db_path, &group_id)
}

#[tauri::command]
pub fn get_goal(
    goal_id: String,
    state: State<'_, AppState>,
) -> AppResult<Option<crate::goals::Goal>> {
    crate::goals::service::get_goal(&state.db_path, &goal_id)
}

#[tauri::command]
pub fn create_goal(
    input: crate::goals::CreateGoalInput,
    state: State<'_, AppState>,
) -> AppResult<crate::goals::Goal> {
    let auto_advance = input.auto_advance;
    let goal = crate::goals::service::create_goal(
        &state.db_path,
        &input.group_id,
        &input.objective,
        input.budget.unwrap_or_default(),
        input.require_approval_each_round,
    )?;
    if auto_advance {
        return crate::goals::service::set_auto_advance(&state.db_path, &goal.id, true);
    }
    Ok(goal)
}

#[tauri::command]
pub fn start_goal(goal_id: String, state: State<'_, AppState>) -> AppResult<crate::goals::Goal> {
    crate::goals::service::start_goal(&state.db_path, &goal_id)
}

#[tauri::command]
pub fn pause_goal(goal_id: String, state: State<'_, AppState>) -> AppResult<crate::goals::Goal> {
    crate::goals::service::pause_goal(&state.db_path, &goal_id)
}

#[tauri::command]
pub fn approve_goal(goal_id: String, state: State<'_, AppState>) -> AppResult<crate::goals::Goal> {
    crate::goals::service::approve_goal(&state.db_path, &goal_id)
}

#[tauri::command]
pub fn abort_goal(goal_id: String, state: State<'_, AppState>) -> AppResult<crate::goals::Goal> {
    crate::goals::service::abort_goal(&state.db_path, &goal_id)
}

/// 推进一步：派发一轮给驱动 Agent，轮询 run 完成后记录。
///
/// 这一步会阻塞到 run 结束或超时（默认约 1 分钟），因此放到阻塞线程池执行。
#[tauri::command]
pub async fn advance_goal(
    goal_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<crate::goals::Goal> {
    let state = state.inner().clone();
    let goal = crate::goals::service::get_goal(&state.db_path, &goal_id)?
        .ok_or_else(|| format!("goal 不存在：{goal_id}"))?;
    if goal.status != crate::goals::GoalStatus::Active {
        return Err("目标未处于运行中".into());
    }
    let actor = crate::goals::service::pick_goal_actor(&state.db_path, &goal.group_id)?;

    let kick_state = state.clone();
    let kick_app = app.clone();
    let dispatcher = crate::goals::scheduler_adapter::SchedulerDispatcher::new(
        state.db_path.clone(),
        goal.group_id.clone(),
        actor.driver_member_id,
        actor.sender_member_id,
        Box::new(move |group_id| {
            schedule_group(to_scheduler(&kick_state, &kick_app), group_id.to_string());
        }),
    );
    let db_path = state.db_path.clone();
    let report = tokio::task::spawn_blocking(move || {
        crate::goals::scheduler_adapter::tick(
            &db_path,
            &goal_id,
            dispatcher,
            crate::goals::scheduler_adapter::DEFAULT_MAX_POLLS,
        )
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(report.goal)
}

/// 开启 / 关闭某个目标的自动推进。
#[tauri::command]
pub fn set_goal_auto_advance(
    goal_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> AppResult<crate::goals::Goal> {
    crate::goals::service::set_auto_advance(&state.db_path, &goal_id, enabled)
}
