//! 日志 / 经验 IPC 命令（P4 入口瘦身）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use super::*;

#[tauri::command]
pub fn save_experience(
    group_id: String,
    source_member_id: String,
    title: String,
    content: String,
    tags: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<String> {
    save_experience_db(
        &open_db(&state.db_path)?,
        &group_id,
        &source_member_id,
        &title,
        &content,
        tags.as_deref().unwrap_or(""),
    )
}

#[tauri::command]
pub fn query_experiences(
    group_id: String,
    query: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<Vec<Experience>> {
    query_experiences_db(
        &open_db(&state.db_path)?,
        &group_id,
        query.as_deref().unwrap_or(""),
        limit.unwrap_or(20),
    )
}

#[tauri::command]
pub fn delete_experience(id: String, state: State<'_, AppState>) -> AppResult<bool> {
    delete_experience_db(&open_db(&state.db_path)?, &id)
}

// === Logs ===

#[tauri::command]
pub fn list_logs(
    limit: Option<i64>,
    offset: Option<i64>,
    level: Option<String>,
    source: Option<String>,
    since: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<Vec<LogEntry>> {
    logger::query_logs(
        &open_db(&state.db_path)?,
        &LogQuery {
            limit,
            offset,
            level,
            source,
            since,
        },
    )
}

#[tauri::command]
pub fn count_logs(level: Option<String>, state: State<'_, AppState>) -> AppResult<i64> {
    logger::count_logs(&open_db(&state.db_path)?, level.as_deref(), None)
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppState>) -> AppResult<()> {
    let conn = open_db(&state.db_path)?;
    logger::clear_logs(&conn)?;
    logger::info(&conn, "logs", "logs cleared by user", None);
    Ok(())
}
