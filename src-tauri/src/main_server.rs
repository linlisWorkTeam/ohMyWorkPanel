use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use linlis_work_panel_lib::db;
use linlis_work_panel_lib::web;

#[tokio::main]
async fn main() {
    // Data directory: LINLIS_DATA_DIR > XDG/platform default
    let data_dir: PathBuf = if let Ok(dir) = env::var("LINLIS_DATA_DIR") {
        PathBuf::from(dir)
    } else if cfg!(target_os = "windows") {
        env::var("APPDATA")
            .map(|p| PathBuf::from(p).join("linlis-work-panel"))
            .unwrap_or_else(|_| PathBuf::from("data"))
    } else {
        let home = env::var("HOME").unwrap_or_else(|_| "/root".into());
        PathBuf::from(home).join(".local/share/linlis-work-panel")
    };
    let db_path = data_dir.join("linlis-work-panel.sqlite3");

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("create db data dir");
    }

    db::init_db(&db_path).expect("init database");
    println!("DB: {}", db_path.display());

    let (tx, _) = broadcast::channel::<String>(256);
    let state = Arc::new(web::AppState { db_path, tx });

    let dist_dir = env::var("LINLIS_WEB_DIST").unwrap_or_else(|_| "../dist".to_string());
    println!("Static: {}", dist_dir);

    let app = web::build_router(state)
        .layer(CorsLayer::permissive())
        .nest_service("/", ServeDir::new(&dist_dir));

    let addr = "0.0.0.0:8080";
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind to {addr}");
    println!("LinlisWorkPanel Web Server -> http://{addr}");
    axum::serve(listener, app).await.expect("serve");
}
