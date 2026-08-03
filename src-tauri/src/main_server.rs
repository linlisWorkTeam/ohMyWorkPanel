use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use axum::http::{header, HeaderValue, Request};
use axum::middleware::{self, Next};
use axum::response::Response;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use linlis_work_panel_lib::db;
use linlis_work_panel_lib::event_sender::EventSender;
use linlis_work_panel_lib::logger;
use linlis_work_panel_lib::scheduler::SchedulerState;
use linlis_work_panel_lib::web;

/// Prevent browsers from caching index.html (stale shell → old JS without login page).
async fn html_no_cache(req: Request<axum::body::Body>, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let mut res = next.run(req).await;
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let is_html = path == "/" || path.ends_with(".html") || content_type.contains("text/html");
    if is_html {
        res.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate"),
        );
        res.headers_mut()
            .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    }
    res
}

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

    // Create scheduler state with Web event sender
    let sched = SchedulerState {
        db_path: db_path.clone(),
        event_sender: EventSender::Web(tx.clone()),
        cancellations: Arc::new(Mutex::new(HashMap::new())),
        scheduling_groups: Arc::new(Mutex::new(HashSet::new())),
    };

    let state = Arc::new(web::AppState {
        db_path: db_path.clone(),
        tx: tx.clone(),
        sched: sched.clone(),
    });

    // Start background scheduler for agent runs
    web::start_scheduler_background(sched);

    let dist_dir = env::var("LINLIS_WEB_DIST").unwrap_or_else(|_| "../dist".to_string());
    println!("Static: {}", dist_dir);

    let app = web::build_router(state)
        .layer(CorsLayer::permissive())
        .fallback_service(ServeDir::new(&dist_dir))
        .layer(middleware::from_fn(html_no_cache));

    let port = env::var("LINLIS_PORT").unwrap_or_else(|_| "8080".into());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind to {addr}: {e}"));
    println!("LinlisWorkPanel Web Server -> http://{addr}");
    // Log server start
    if let Ok(conn) = db::open_db(&db_path) {
        logger::info(&conn, "server", "LinlisWorkPanel Web Server started", Some(&format!("{{\"addr\":\"{}\"}}", addr)));
    }
    axum::serve(listener, app).await.expect("serve");
}
