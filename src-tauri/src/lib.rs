mod adapters;
mod commands;
mod db;
mod models;
mod scheduler;

use db::init_db;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
};
use tauri::Manager;

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    pub scheduling_groups: Arc<Mutex<HashSet<String>>>,
}

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
            commands::get_runtime_settings,
            commands::update_runtime_settings,
            commands::create_group,
            commands::add_member,
            commands::remove_member,
            commands::set_admin,
            commands::send_message,
            commands::cancel_run,
            commands::retry_run,
            commands::detect_agent
        ])
        .run(tauri::generate_context!())
        .expect("启动 LinlisWorkPanel 失败");
}
