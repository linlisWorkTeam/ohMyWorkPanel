//! Roadmap / Feature / FeatureTask IPC 命令（P4 入口瘦身）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use super::*;

#[tauri::command]
pub fn list_roadmap_items(
    group_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<RoadmapItem>> {
    get_roadmap_items(&open_db(&state.db_path)?, &group_id)
}

#[tauri::command]
pub fn create_roadmap_item(
    input: CreateRoadmapItemInput,
    state: State<'_, AppState>,
) -> AppResult<RoadmapItem> {
    create_roadmap_item_db(&open_db(&state.db_path)?, &input)
}

#[tauri::command]
pub fn update_roadmap_item(
    id: String,
    input: UpdateRoadmapItemInput,
    state: State<'_, AppState>,
) -> AppResult<RoadmapItem> {
    update_roadmap_item_db(&open_db(&state.db_path)?, &id, &input)
}

#[tauri::command]
pub fn delete_roadmap_item(id: String, state: State<'_, AppState>) -> AppResult<()> {
    delete_roadmap_item_db(&open_db(&state.db_path)?, &id)
}

// === PM: Features ===

#[tauri::command]
pub fn list_features(group_id: String, state: State<'_, AppState>) -> AppResult<Vec<Feature>> {
    get_features(&open_db(&state.db_path)?, &group_id)
}

#[tauri::command]
pub fn create_feature(input: CreateFeatureInput, state: State<'_, AppState>) -> AppResult<Feature> {
    create_feature_db(&open_db(&state.db_path)?, &input)
}

#[tauri::command]
pub fn update_feature(
    id: String,
    input: UpdateFeatureInput,
    state: State<'_, AppState>,
) -> AppResult<Feature> {
    update_feature_db(&open_db(&state.db_path)?, &id, &input)
}

#[tauri::command]
pub fn delete_feature(id: String, state: State<'_, AppState>) -> AppResult<()> {
    delete_feature_db(&open_db(&state.db_path)?, &id)
}

// === PM: Feature Tasks ===

#[tauri::command]
pub fn list_feature_tasks(
    feature_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<FeatureTask>> {
    get_feature_tasks(&open_db(&state.db_path)?, &feature_id)
}

#[tauri::command]
pub fn create_feature_task(
    input: CreateFeatureTaskInput,
    state: State<'_, AppState>,
) -> AppResult<FeatureTask> {
    create_feature_task_db(&open_db(&state.db_path)?, &input)
}

#[tauri::command]
pub fn update_feature_task(
    id: String,
    input: UpdateFeatureTaskInput,
    state: State<'_, AppState>,
) -> AppResult<FeatureTask> {
    update_feature_task_db(&open_db(&state.db_path)?, &id, &input)
}

#[tauri::command]
pub fn delete_feature_task(id: String, state: State<'_, AppState>) -> AppResult<()> {
    delete_feature_task_db(&open_db(&state.db_path)?, &id)
}

// === PM: Aggregated State ===

#[tauri::command]
pub fn get_roadmap_state(group_id: String, state: State<'_, AppState>) -> AppResult<RoadmapState> {
    get_roadmap_state_db(&open_db(&state.db_path)?, &group_id)
}

// === Shared Memory: Experiences ===
