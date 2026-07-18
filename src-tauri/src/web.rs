use axum::{
    extract::{Path, State, WebSocketUpgrade},
    extract::ws::{Message as WsMessage, WebSocket},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::auth::Claims;
use crate::db::{open_db, group_state, get_groups, id, now};
use crate::models::{Group, GroupState, Message};

// === Shared State ===
pub struct AppState {
    pub db_path: std::path::PathBuf,
    pub tx: broadcast::Sender<String>,
}

// === Claims Extractor ===
pub struct ClaimsExtractor(pub Claims);

impl<S> axum::extract::FromRequestParts<S> for ClaimsExtractor
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let auth = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or((StatusCode::UNAUTHORIZED, "Missing token"))?;
        let claims =
            crate::auth::validate_jwt(auth).map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token"))?;
        Ok(ClaimsExtractor(claims))
    }
}

// === Auth Middleware ===
async fn auth_middleware(req: Request<axum::body::Body>, next: Next) -> Response {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match auth_header {
        Some(token) => match crate::auth::validate_jwt(token) {
            Ok(_) => next.run(req).await,
            Err(e) => (StatusCode::UNAUTHORIZED, e).into_response(),
        },
        None => (StatusCode::UNAUTHORIZED, "Missing token".to_string()).into_response(),
    }
}

// === Auth Routes ===

#[derive(Debug, Deserialize)]
struct RegisterInput {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    token: String,
    user_id: String,
    username: String,
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RegisterInput>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    if input.username.len() < 2 || input.password.len() < 4 {
        return Err((StatusCode::BAD_REQUEST, "username >=2, password >=4".into()));
    }
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if conn
        .query_row("SELECT id FROM users WHERE username=?1", params![input.username], |_| Ok(()))
        .is_ok()
    {
        return Err((StatusCode::CONFLICT, "username taken".into()));
    }

    let user_id = id();
    let password_hash =
        crate::auth::hash_password(&input.password).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    conn.execute(
        "INSERT INTO users(id,username,password_hash,created_at) VALUES(?1,?2,?3,?4)",
        params![user_id, input.username, password_hash, now()],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let token = crate::auth::create_jwt(&user_id, &input.username)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(AuthResponse { token, user_id, username: input.username }))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RegisterInput>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let (uid, hash): (String, String) = conn
        .query_row(
            "SELECT id, password_hash FROM users WHERE username=?1",
            params![input.username],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| (StatusCode::UNAUTHORIZED, "bad credentials".into()))?;

    if !crate::auth::verify_password(&input.password, &hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        return Err((StatusCode::UNAUTHORIZED, "bad credentials".into()));
    }

    let token = crate::auth::create_jwt(&uid, &input.username)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(AuthResponse { token, user_id: uid, username: input.username }))
}

async fn verify(ClaimsExtractor(claims): ClaimsExtractor) -> Json<Claims> {
    Json(claims)
}

// === Group Routes ===

async fn list_groups(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Group>>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    get_groups(&conn).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn get_group(
    State(state): State<Arc<AppState>>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupState>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    group_state(&conn, &group_id).map(Json).map_err(|e| (StatusCode::NOT_FOUND, e))
}

#[derive(Debug, Deserialize)]
struct CreateGroupInputWeb {
    name: String,
    workspace_path: String,
    owner_name: String,
    #[allow(dead_code)]
    owner_id: String,
}

async fn create_group_web(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateGroupInputWeb>,
) -> Result<Json<GroupState>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let group_id = id();
    let owner_member_id = id();
    let created_at = now();

    conn.execute(
        "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES(?1,?2,?3,?4,NULL,?5)",
        params![group_id, input.name, input.workspace_path, owner_member_id, created_at],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    conn.execute(
        "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES(?1,?2,'user',?3,'#5167f6','owner',1,?4)",
        params![owner_member_id, group_id, input.owner_name, created_at],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    group_state(&conn, &group_id).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

// === Message Routes ===

#[derive(Debug, Deserialize)]
struct SendMessageInput {
    group_id: String,
    sender_member_id: String,
    content: String,
    #[allow(dead_code)]
    mention_member_ids: Vec<String>,
}

async fn send_message_web(
    State(state): State<Arc<AppState>>,
    Json(input): Json<SendMessageInput>,
) -> Result<Json<Message>, (StatusCode, String)> {
    if input.content.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty message".into()));
    }
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let msg = Message {
        id: id(),
        group_id: input.group_id.clone(),
        sender_member_id: input.sender_member_id,
        parent_run_id: None,
        content: input.content,
        status: "completed".into(),
        created_at: now(),
    };
    conn.execute(
        "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,?5,?6)",
        params![msg.id, msg.group_id, msg.sender_member_id, msg.content, msg.status, msg.created_at],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let _ = state.tx.send(format!(
        "{{\"kind\":\"message_created\",\"group_id\":\"{}\",\"message_id\":\"{}\"}}",
        msg.group_id, msg.id
    ));

    Ok(Json(msg))
}

// === WebSocket ===

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if socket.send(WsMessage::Text(text.into())).await.is_err() { break; }
                    }
                    Err(_) => break,
                }
            }
            ws_msg = socket.recv() => {
                match ws_msg {
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

// === Router Builder ===

pub fn build_router(state: Arc<AppState>) -> Router {
    let auth_routes = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login));

    let protected = Router::new()
        .route("/api/auth/verify", get(verify))
        .route("/api/groups", get(list_groups))
        .route("/api/groups/{id}", get(get_group))
        .route("/api/groups", post(create_group_web))
        .route("/api/messages", post(send_message_web))
        .route("/ws", get(ws_handler))
        .route_layer(middleware::from_fn(auth_middleware));

    Router::new().merge(auth_routes).merge(protected).with_state(state)
}
