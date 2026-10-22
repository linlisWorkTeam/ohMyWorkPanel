//! Self-Marketing IPC 命令（P4）。
//!
//! 从 commands.rs 拆出；对外路径仍为 `commands::<name>`（由 mod.rs 重导出）。
//! 与 Web 一样只走插件服务，不再保留第二套自由函数实现。

use crate::db::AppResult;
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
    let sched = to_scheduler(&app_state, &app);
    crate::plugins::require_marketing_service()?.create(&app_state.db_path, sched, input)
}

#[tauri::command]
pub fn list_marketing_campaigns(
    group_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<crate::marketing::ContentCampaign>> {
    crate::plugins::require_marketing_service()?.list(&state.db_path, &group_id)
}

#[tauri::command]
pub fn get_marketing_campaign(
    campaign_id: String,
    state: State<'_, AppState>,
) -> AppResult<crate::marketing::ContentCampaign> {
    crate::plugins::require_marketing_service()?.get(&state.db_path, &campaign_id)
}

#[tauri::command]
pub async fn revise_marketing_campaign(
    campaign_id: String,
    input: crate::marketing::ReviseCampaignInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<crate::marketing::ContentCampaign> {
    let app_state = state.inner().clone();
    let sched = to_scheduler(&app_state, &app);
    crate::plugins::require_marketing_service()?.revise(
        &app_state.db_path,
        sched,
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
    crate::plugins::require_marketing_service()?.approve(&state.db_path, &campaign_id, input)
}

#[tauri::command]
pub fn export_marketing_campaign(
    campaign_id: String,
    state: State<'_, AppState>,
) -> AppResult<crate::marketing::CampaignExport> {
    crate::plugins::require_marketing_service()?.export(&state.db_path, &campaign_id)
}
