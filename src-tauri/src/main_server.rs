use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use linlis_work_panel_lib::db;
use linlis_work_panel_lib::web;

#[tokio::main]
async fn main() {
    // DB path: %APPDATA%/linlis-work-panel/linlis-work-panel.sqlite3
    let db_path: PathBuf = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("linlis-work-panel")
        .join("linlis-work-panel.sqlite3");

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("create db data dir");
    }

    db::init_db(&db_path).expect("init database");
    println!("DB path: {}", db_path.display());

    let (tx, _) = broadcast::channel::<String>(256);
    let state = Arc::new(web::AppState { db_path, tx });

    let app = web::build_router(state)
        .layer(CorsLayer::permissive())
        .nest_service("/", ServeDir::new("../dist"));

    let addr = "0.0.0.0:8080";
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind to {addr}");
    println!("LinlisWorkPanel Web Server → http://{addr}");
    axum::serve(listener, app).await.expect("serve");
}
