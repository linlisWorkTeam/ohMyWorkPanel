//! Self-Marketing IPC 命令（P4）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use crate::db::{open_db, AppResult};
use crate::AppState;
use tauri::{AppHandle, State};

use super::to_scheduler;

// ============================================================================
// Self-Marketing（Desktop 桥——与 Web API 同一 domain service）
// ============================================================================

#[tauri::command]
pub async fn create_marketing_campaign(
    input: crate::marketing::CreateCampaignInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<crate::marketing::ContentCampaign> {
    let app_state = state.inner().clone();
    crate::marketing::create_campaign(&app_state.db_path, to_scheduler(&app_state, &app), input)
}

#[tauri::command]
pub fn list_marketing_campaigns(
    group_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<crate::marketing::ContentCampaign>> {
    crate::marketing::list_campaigns(&open_db(&state.db_path)?, &group_id)
}

#[tauri::command]
pub fn get_marketing_campaign(
    campaign_id: String,
    state: State<'_, AppState>,
) -> AppResult<crate::marketing::ContentCampaign> {
    crate::marketing::get_campaign(&open_db(&state.db_path)?, &campaign_id)
}

#[tauri::command]
pub async fn revise_marketing_campaign(
    campaign_id: String,
    input: crate::marketing::ReviseCampaignInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<crate::marketing::ContentCampaign> {
    let app_state = state.inner().clone();
    crate::marketing::revise_campaign(
        &app_state.db_path,
        to_scheduler(&app_state, &app),
        &campaign_id,
        input,
    )
}

#[tauri::command]
pub fn approve_marketing_campaign(
    campaign_id: String,
    input: crate::marketing::ApproveCampaignInput,
    state: State<'_, AppState>,
) -> AppResult<crate::marketing::ContentCampaign> {
    crate::marketing::approve_campaign(&state.db_path, &campaign_id, input)
}

#[tauri::command]
pub fn export_marketing_campaign(
    campaign_id: String,
    state: State<'_, AppState>,
) -> AppResult<crate::marketing::CampaignExport> {
    crate::marketing::export_campaign(&open_db(&state.db_path)?, &campaign_id)
}
