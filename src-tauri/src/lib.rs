pub mod auth;
mod adapters;
pub mod a2a;

#[cfg(feature = "gui")]
mod commands;
mod ocr;
pub mod codex_proxy;
mod context_policy;
pub mod db;
pub mod presence;
pub mod extensions;
pub mod fs_browse;
pub mod keepalive;
pub mod memory;
mod message_content;
mod models;
pub mod logger;
pub mod metrics;
pub mod event_sender;
pub mod ops;
pub mod orchestrator;
pub mod scheduler;
pub mod web;

use db::init_db;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

#[cfg(feature = "gui")]
use tauri::Manager;

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    pub scheduling_groups: Arc<Mutex<HashSet<String>>>,
}

#[cfg(feature = "gui")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app
                .path()
                .app_data_dir()
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            fs::create_dir_all(&dir)?;
            let db_path = dir.join("linlis-work-panel.sqlite3");
            init_db(&db_path).map_err(std::io::Error::other)?;
            app.manage(AppState {
                db_path,
                cancellations: Arc::new(Mutex::new(HashMap::new())),
                scheduling_groups: Arc::new(Mutex::new(HashSet::new())),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::get_group_state,
            commands::list_messages_before,
            commands::get_message_channel_part,
            commands::get_runtime_settings,
            commands::update_runtime_settings,
            commands::create_group,
            commands::add_member,
            commands::list_joinable_users,
            commands::remove_member,
            commands::set_admin,
            commands::send_message,
            commands::cancel_run,
            commands::retry_run,
            commands::detect_agent,
            commands::ocr_image,
            commands::ocr_image_base64,
            commands::get_preset_roles_command,
            // PM: Roadmap Items
            commands::list_roadmap_items,
            commands::create_roadmap_item,
            commands::update_roadmap_item,
            commands::delete_roadmap_item,
            // PM: Features
            commands::list_features,
            commands::create_feature,
            commands::update_feature,
            commands::delete_feature,
            // PM: Feature Tasks
            commands::list_feature_tasks,
            commands::create_feature_task,
            commands::update_feature_task,
            commands::delete_feature_task,
            // PM: Aggregated
            commands::get_roadmap_state,
            // Shared Memory: Experiences
            commands::save_experience,
            commands::query_experiences,
            commands::delete_experience,
            // Logs
            commands::list_logs,
            commands::count_logs,
            commands::clear_logs,
            commands::list_server_dir,
            commands::create_server_dir,
            commands::update_group_workspace_cmd,
            commands::update_member_workspace_cmd,
            commands::get_group_announcement,
            commands::set_group_announcement_cmd,
            commands::ops_release_status,
            commands::ops_job_status,
            commands::ops_run_test_gate,
            commands::ops_deploy_canary,
            commands::set_group_archived_cmd,
            commands::update_member_model_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to launch LinlisWorkPanel");
}

#[cfg(not(feature = "gui"))]
pub fn run() {}
