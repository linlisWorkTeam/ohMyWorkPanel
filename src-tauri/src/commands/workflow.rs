//! 版本 / Wave 工作流 IPC 命令（P4）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use crate::db::{get_group, open_db, AppResult};
use crate::AppState;
use tauri::{AppHandle, State};

use super::{admin_display_name, kickoff_admin_run_desktop, require_desktop_version_manager};

#[tauri::command]
pub async fn get_version_board(
    group_id: String,
    state: State<'_, AppState>,
) -> AppResult<crate::workflow::VersionBoard> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let group = get_group(&conn, &group_id)?;
    crate::plugins::require_workflow_service()?.board(&state.db_path, &group)
}

#[tauri::command]
pub async fn create_project_version(
    input: crate::workflow::CreateVersionInput,
    state: State<'_, AppState>,
) -> AppResult<crate::workflow::ProjectVersion> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let group = get_group(&conn, &input.group_id)?;
    let sender = input.requester_member_id.as_deref().unwrap_or_default();
    require_desktop_version_manager(&conn, &group, sender)?;
    crate::plugins::require_workflow_service()?.create_version(&state.db_path, &group, &input)
}

#[tauri::command]
pub async fn update_version_roadmap(
    version_id: String,
    input: crate::workflow::UpdateRoadmapInput,
    state: State<'_, AppState>,
) -> AppResult<crate::workflow::ProjectVersion> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let v = crate::workflow::get_version(&conn, &version_id)?;
    let group = get_group(&conn, &v.group_id)?;
    let sender = input.requester_member_id.as_deref().unwrap_or_default();
    require_desktop_version_manager(&conn, &group, sender)?;
    crate::plugins::require_workflow_service()?.update_roadmap(&state.db_path, &version_id, &input)
}

#[tauri::command]
pub async fn start_version_ask(
    version_id: String,
    sender_member_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<serde_json::Value> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let v = crate::workflow::get_version(&conn, &version_id)?;
    let group = get_group(&conn, &v.group_id)?;
    require_desktop_version_manager(&conn, &group, &sender_member_id)?;
    let v = crate::plugins::require_workflow_service()?.start_ask(
        &state.db_path,
        &group,
        &version_id,
    )?;
    let prompt = crate::workflow::build_ask_kickoff_prompt(&v);
    let content = format!("@{} {}", admin_display_name(&conn, &group), prompt);
    let run_id =
        kickoff_admin_run_desktop(&conn, &state, &app, &group, &sender_member_id, content)?;
    Ok(serde_json::json!({ "version": v, "runId": run_id }))
}

#[tauri::command]
pub async fn cancel_version_ask(
    version_id: String,
    sender_member_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<crate::workflow::ProjectVersion> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let v = crate::workflow::get_version(&conn, &version_id)?;
    let group = get_group(&conn, &v.group_id)?;
    // 桌面单机：未显式传发送者时按群主处理
    let sender = sender_member_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| group.owner_member_id.clone());
    require_desktop_version_manager(&conn, &group, &sender)?;
    crate::plugins::require_workflow_service()?.cancel_ask(&state.db_path, &version_id)
}

#[tauri::command]
pub async fn approve_version_waves(
    version_id: String,
    waves: Option<Vec<crate::workflow::ApproveWaveItem>>,
    sender_member_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<serde_json::Value> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let v = crate::workflow::get_version(&conn, &version_id)?;
    let group = get_group(&conn, &v.group_id)?;
    // 桌面单机：未显式传发送者时按群主处理
    let sender = sender_member_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| group.owner_member_id.clone());
    require_desktop_version_manager(&conn, &group, &sender)?;
    let mut input = crate::workflow::ApproveWavesInput {
        waves: waves.unwrap_or_default(),
    };
    if input.waves.is_empty() {
        input.waves = crate::workflow::default_waves_from_roadmap(&v);
    }
    let (version, waves) = crate::workflow::approve_waves(&conn, &version_id, &input)?;
    Ok(serde_json::json!({ "version": version, "waves": waves }))
}

#[tauri::command]
pub async fn play_wave(
    wave_id: String,
    sender_member_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<serde_json::Value> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let w0 = crate::workflow::get_wave(&conn, &wave_id)?;
    let group = get_group(&conn, &w0.group_id)?;
    require_desktop_version_manager(&conn, &group, &sender_member_id)?;
    let w = crate::plugins::require_workflow_service()?.play_wave(&state.db_path, &wave_id)?;
    let v = crate::workflow::get_version(&conn, &w.version_id)?;
    let prompt = crate::workflow::build_wave_kickoff_prompt(&v, &w);
    let run_id = kickoff_admin_run_desktop(
        &conn,
        &state,
        &app,
        &group,
        &sender_member_id,
        format!("@{} {prompt}", admin_display_name(&conn, &group)),
    )?;
    Ok(serde_json::json!({ "wave": w, "runId": run_id }))
}

#[tauri::command]
pub async fn pause_wave(
    wave_id: String,
    state: State<'_, AppState>,
) -> AppResult<crate::workflow::Wave> {
    let state = state.inner().clone();
    crate::plugins::require_workflow_service()?.pause_wave(&state.db_path, &wave_id)
}

#[tauri::command]
pub async fn advance_wave(
    wave_id: String,
    state: State<'_, AppState>,
) -> AppResult<crate::workflow::Wave> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    crate::workflow::advance_wave_phase(&conn, &wave_id)
}

#[tauri::command]
pub async fn play_version(
    version_id: String,
    sender_member_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<serde_json::Value> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let v = crate::workflow::get_version(&conn, &version_id)?;
    let group = get_group(&conn, &v.group_id)?;
    require_desktop_version_manager(&conn, &group, &sender_member_id)?;
    let wave = crate::workflow::play_version_roadmap(&conn, &version_id)?;
    if let Some(w) = wave {
        let v = crate::workflow::get_version(&conn, &w.version_id)?;
        let prompt = crate::workflow::build_wave_kickoff_prompt(&v, &w);
        let run_id = kickoff_admin_run_desktop(
            &conn,
            &state,
            &app,
            &group,
            &sender_member_id,
            format!("@{} {prompt}", admin_display_name(&conn, &group)),
        )?;
        return Ok(serde_json::json!({ "wave": w, "runId": run_id }));
    }
    Ok(serde_json::json!({ "wave": null, "status": "awaiting_release" }))
}

#[tauri::command]
pub async fn pause_version(
    version_id: String,
    state: State<'_, AppState>,
) -> AppResult<serde_json::Value> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let v = crate::workflow::get_version(&conn, &version_id)?;
    let _group = get_group(&conn, &v.group_id)?;
    crate::workflow::pause_version_roadmap(&conn, &version_id)?;
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
pub async fn release_version(
    version_id: String,
    git_tag: Option<String>,
    sender_member_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<crate::workflow::ProjectVersion> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    let v = crate::workflow::get_version(&conn, &version_id)?;
    let group = get_group(&conn, &v.group_id)?;
    // 桌面单机：未显式传发送者时按群主处理
    let sender = sender_member_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| group.owner_member_id.clone());
    require_desktop_version_manager(&conn, &group, &sender)?;
    crate::plugins::require_workflow_service()?.mark_released(&state.db_path, &version_id, git_tag)
}
