use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    extract::ws::{Message as WsMessage, WebSocket},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, delete, get, post, put},
    Json, Router,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::auth::Claims;
use crate::logger::{self, LogEntry, LogQuery};
use crate::presence::PresenceRegistry;
use crate::scheduler::{self, SchedulerState};
    use crate::db::{
    create_feature_db, create_feature_task_db,
    create_roadmap_item_db, create_task_run, delete_feature_db,
    delete_feature_task_db, delete_roadmap_item_db, get_features,
    get_feature_tasks, get_groups_for_user,
    get_group as db_get_group,
    get_preset_roles, get_roadmap_items, get_roadmap_state_db, get_runs,
    get_messages_before, get_settings_from, group_state, id, is_admin_user, mark_group_read,
    member_from_row, now, open_db, require_admin, require_group_access, resolve_target_agent_ids,
    set_group_announcement, update_feature_db, update_feature_task_db, update_group_workspace,
    update_roadmap_item_db,
};
use crate::fs_browse::{self, DirListing};
use crate::ops;
use crate::models::{
    CreateFeatureInput, CreateFeatureTaskInput, CreateRoadmapItemInput, Experience,
    Feature, FeatureTask, Group, GroupState, Member, Message, PresetRole, RoadmapItem,
    RoadmapState, RuntimeSettings, TaskRun, UpdateFeatureInput, UpdateFeatureTaskInput,
    UpdateRoadmapItemInput,
};
// Helper to emit events to WebSocket clients
fn web_emit(tx: &broadcast::Sender<String>, group_id: &str, kind: &str, message_id: Option<&str>, run_id: Option<&str>, status: Option<&str>, error: Option<&str>) {
    web_emit_delta(tx, group_id, kind, message_id, run_id, status, error, None);
}

fn web_emit_delta(
    tx: &broadcast::Sender<String>,
    group_id: &str,
    kind: &str,
    message_id: Option<&str>,
    run_id: Option<&str>,
    status: Option<&str>,
    error: Option<&str>,
    delta: Option<&str>,
) {
    let event = crate::models::ChatEvent {
        kind: kind.into(),
        group_id: group_id.into(),
        run_id: run_id.map(str::to_string),
        message_id: message_id.map(str::to_string),
        delta: delta.map(str::to_string),
        status: status.map(str::to_string),
        error: error.map(str::to_string),
        channel: None,
        replace: None,
        phase: None,
        elapsed_ms: None,
        total_ms: None,
            seq: None,
            delta_count: None,
            rss_mib: None,
        };
    if let Ok(payload) = serde_json::to_string(&event) {
        let _ = tx.send(payload);
    }
}

// === Shared State ===
pub struct AppState {
    pub db_path: std::path::PathBuf,
    pub tx: broadcast::Sender<String>,
    pub sched: SchedulerState,
    pub presence: Arc<PresenceRegistry>,
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
    let header_token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());
    let query_token = req.uri().query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some("token"), Some(v)) if !v.is_empty() => Some(v.to_string()),
                _ => None,
            }
        })
    });
    let token = header_token.or(query_token);

    match token {
        Some(token) => match crate::auth::validate_jwt(&token) {
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
    #[serde(rename = "isAdmin")]
    is_admin: bool,
}

fn map_acl_err(e: String) -> (StatusCode, String) {
    if e.contains("无权") || e.contains("管理员") {
        (StatusCode::FORBIDDEN, e)
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, e)
    }
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RegisterInput>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    if input.username.trim().is_empty() || input.password.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "username and password required".into()));
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
        "INSERT INTO users(id,username,password_hash,created_at,is_admin) VALUES(?1,?2,?3,?4,0)",
        params![user_id, input.username.trim(), password_hash, now()],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let token = crate::auth::create_jwt(&user_id, input.username.trim())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    logger::info(&conn, "auth", &format!("user registered: {}", input.username), None);
    Ok(Json(AuthResponse {
        token,
        user_id,
        username: input.username.trim().to_string(),
        is_admin: false,
    }))
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
        .map_err(|_| {
            logger::warn(&conn, "auth", &format!("failed login attempt for user: {}", input.username), None);
            (StatusCode::UNAUTHORIZED, "bad credentials".into())
        })?;

    if !crate::auth::verify_password(&input.password, &hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        logger::warn(&conn, "auth", &format!("wrong password for user: {}", input.username), None);
        return Err((StatusCode::UNAUTHORIZED, "bad credentials".into()));
    }

    let token = crate::auth::create_jwt(&uid, &input.username)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let is_admin = is_admin_user(&conn, &uid).unwrap_or(false);
    logger::info(&conn, "auth", &format!("user logged in: {}", input.username), None);
    Ok(Json(AuthResponse {
        token,
        user_id: uid,
        username: input.username,
        is_admin,
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifyResponse {
    sub: String,
    username: String,
    is_admin: bool,
}

async fn verify(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
) -> Result<Json<VerifyResponse>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let is_admin = is_admin_user(&conn, &claims.sub).map_err(map_acl_err)?;
    Ok(Json(VerifyResponse {
        sub: claims.sub,
        username: claims.username,
        is_admin,
    }))
}

// === Group Routes ===

async fn list_groups(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
) -> Result<Json<Vec<Group>>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    get_groups_for_user(&conn, &claims.sub)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn get_group(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Path(group_id): Path<String>,
) -> Result<Json<GroupState>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_group_access(&conn, &claims.sub, &group_id).map_err(map_acl_err)?;
    // Entering a group clears unread for this user.
    let _ = mark_group_read(&conn, &claims.sub, &group_id);
    group_state(&conn, &group_id).map(Json).map_err(|e| (StatusCode::NOT_FOUND, e))
}

async fn mark_group_read_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Path(group_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_group_access(&conn, &claims.sub, &group_id).map_err(map_acl_err)?;
    mark_group_read(&conn, &claims.sub, &group_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "ok": true, "groupId": group_id })))
}

async fn list_presence_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(_claims): ClaimsExtractor,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(json!({
        "onlineUserIds": state.presence.online_user_ids(),
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateGroupInputWeb {
    name: String,
    workspace_path: String,
    owner_name: String,
    preset_roles: Option<Vec<String>>,
    group_kind: Option<String>,
}

fn normalize_group_kind(raw: Option<&str>) -> String {
    match raw.map(str::trim).unwrap_or("project") {
        "chat" => "chat".into(),
        _ => "project".into(),
    }
}

async fn create_group_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Json(input): Json<CreateGroupInputWeb>,
) -> Result<Json<GroupState>, (StatusCode, String)> {
    let conn_acl = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_admin(&conn_acl, &claims.sub).map_err(map_acl_err)?;
    let group_kind = normalize_group_kind(input.group_kind.as_deref());
    let workspace_path = if group_kind == "chat" {
        String::new()
    } else {
        fs_browse::resolve_server_dir(&input.workspace_path)
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?
            .to_string_lossy()
            .into_owned()
    };
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let group_id = id();
    let owner_member_id = id();
    let created_at = now();

    conn.execute(
        "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,group_kind,archived) VALUES(?1,?2,?3,?4,NULL,?5,?6,0)",
        params![group_id, input.name, workspace_path, owner_member_id, created_at, group_kind],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    conn.execute(
        "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES(?1,?2,'user',?3,'#5167f6','owner',1,?4)",
        params![owner_member_id, group_id, input.owner_name, created_at],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

     // Auto-create agent members from selected preset roles
     if let Some(role_names) = &input.preset_roles {
         let json_val: String = conn
             .query_row("SELECT value FROM app_settings WHERE key='preset_roles'", [], |r| r.get(0))
             .unwrap_or_else(|_| "[]".into());
         if let Ok(all_roles) = serde_json::from_str::<Vec<serde_json::Value>>(&json_val) {
             for role in all_roles {
                 let rname = role["name"].as_str().unwrap_or("");
                 if !role_names.iter().any(|n| n == rname) { continue; }
                 let mid = id();
                 let adapter = role["adapter"].as_str().unwrap_or("mock");
                 let avatar = role["avatarColor"].as_str().unwrap_or("#17a673");
                 let desc = role["roleDescription"].as_str().unwrap_or("");
                 conn.execute(
                     "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES(?1,?2,'agent',?3,?4,?5,1,?6)",
                     params![mid, group_id, rname, avatar, desc, created_at],
                 ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                 conn.execute(
                     "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at) VALUES(?1,?2,NULL,'unknown',?3)",
                     params![mid, adapter, created_at],
                 ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
             }
         }
     }

    logger::info(&conn, "group", &format!("group created: {} (id: {})", input.name, group_id), None);
    group_state(&conn, &group_id).map(Json).map_err(|e| {
        logger::error(&conn, "group", &format!("failed to get group state after creation: {e}"), None);
        (StatusCode::INTERNAL_SERVER_ERROR, e)
    })
}

// === Message Routes ===

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendMessageInput {
    group_id: String,
    sender_member_id: String,
    content: String,
    mention_member_ids: Vec<String>,
}

async fn send_message_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Json(input): Json<SendMessageInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if input.content.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty message".into()));
    }
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_group_access(&conn, &claims.sub, &input.group_id).map_err(map_acl_err)?;

     // Validate sender is active member in this group
     let sender_count = conn
         .query_row(
             "SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND is_active=1",
             params![input.sender_member_id, input.group_id],
             |r| r.get::<_, i64>(0),
         )
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     if sender_count != 1 {
         return Err((StatusCode::FORBIDDEN, "sender not in group".into()));
     }
     // Scoped users may only send as their linked member.
     if !is_admin_user(&conn, &claims.sub).unwrap_or(false) {
         let owned: i64 = conn
             .query_row(
                 "SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND auth_user_id=?3 AND is_active=1",
                 params![input.sender_member_id, input.group_id, claims.sub],
                 |r| r.get(0),
             )
             .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
         if owned != 1 {
             return Err((StatusCode::FORBIDDEN, "只能以本人成员身份发言".into()));
         }
     }

     // Validate group exists
     let group = db_get_group(&conn, &input.group_id)
         .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    let msg = Message {
        id: id(),
        group_id: input.group_id.clone(),
        sender_member_id: input.sender_member_id,
        parent_run_id: None,
        content: input.content.clone(),
        status: "completed".into(),
        created_at: now(),
        has_thinking: false,
        has_artifact: false,
    };
    conn.execute(
        "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,?5,?6)",
        params![msg.id, msg.group_id, msg.sender_member_id, msg.content, msg.status, msg.created_at],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

     // Insert mentions
     for mentioned in &input.mention_member_ids {
         let _ = conn.execute(
             "INSERT OR IGNORE INTO mentions(message_id,member_id) SELECT ?1,id FROM members WHERE id=?2 AND group_id=?3",
             params![msg.id, mentioned, input.group_id],
         );
     }

     // Find target agents: only @agent/@chatbot run.
     // If the message @mentioned someone but no agent was tagged (e.g. only @user),
     // do NOT fall back to the group admin agent.
     let target_agents = resolve_target_agent_ids(
         &conn,
         &input.group_id,
         group.admin_member_id.as_deref(),
         &input.mention_member_ids,
     )
     .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

     // Create task runs for target agents
     let mut run_ids: Vec<String> = Vec::new();
     for agent_id in target_agents {
         match create_task_run(&conn, &input.group_id, &msg.id, &agent_id, None, 0) {
             Ok(rid) => run_ids.push(rid),
             Err(_) => {}
         }
     }

     logger::info(&conn, "message", &format!("message sent to group {}: {} chars, {} agents triggered", &input.group_id, input.content.len(), run_ids.len()), None);

     // Emit via WebSocket broadcast
     web_emit(
         &state.tx,
         &input.group_id,
         "message_created",
         Some(&msg.id),
         None,
         Some("completed"),
         None,
     );

     if !run_ids.is_empty() {
         scheduler::schedule_group(state.sched.clone(), input.group_id.clone());
     }

     Ok(Json(json!({
         "message": msg,
         "runIds": run_ids,
     })))
}

// === WebSocket ===

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let user_id = query
        .get("token")
        .and_then(|t| crate::auth::validate_jwt(t).ok())
        .map(|c| c.sub);
    ws.on_upgrade(move |socket| handle_socket(socket, state, user_id))
}

fn emit_presence(tx: &broadcast::Sender<String>, user_id: &str, online: bool) {
    let payload = json!({
        "kind": "presence",
        "userId": user_id,
        "online": online,
        "ts": now(),
    });
    let _ = tx.send(payload.to_string());
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>, user_id: Option<String>) {
    let mut rx = state.tx.subscribe();
    if let Some(ref uid) = user_id {
        if state.presence.connect(uid) {
            emit_presence(&state.tx, uid, true);
        }
        // Snapshot for this client.
        let snap = json!({
            "kind": "presence_snapshot",
            "onlineUserIds": state.presence.online_user_ids(),
            "ts": now(),
        });
        let _ = socket
            .send(WsMessage::Text(snap.to_string().into()))
            .await;
    }
    // Keep proxies from idle-closing the socket; browsers deliver Text frames to JS.
    let mut beat = tokio::time::interval(std::time::Duration::from_secs(25));
    beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = beat.tick() => {
                let payload = json!({
                    "kind": "heartbeat",
                    "ts": now(),
                });
                if socket
                    .send(WsMessage::Text(payload.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if socket.send(WsMessage::Text(text.into())).await.is_err() { break; }
                    }
                    // Client fell behind: drop lagged messages but keep the socket alive.
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let hint = json!({ "kind": "ws_reconnected", "ts": now() });
                        if socket
                            .send(WsMessage::Text(hint.to_string().into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            ws_msg = socket.recv() => {
                match ws_msg {
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(WsMessage::Ping(payload))) => {
                        if socket.send(WsMessage::Pong(payload)).await.is_err() { break; }
                    }
                    Some(Ok(WsMessage::Text(text))) => {
                        // Client heartbeat / ignore; optional ack keeps NAT mappings warm.
                        if text.contains("heartbeat") {
                            let ack = json!({ "kind": "heartbeat", "ts": now() });
                            if socket
                                .send(WsMessage::Text(ack.to_string().into()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(ref uid) = user_id {
        if state.presence.disconnect(uid) {
            emit_presence(&state.tx, uid, false);
        }
    }
}

 // === Members CRUD ===

 #[derive(Debug, Deserialize)]
 #[serde(rename_all = "camelCase")]
 struct AddMemberInputWeb {
     group_id: String,
     kind: String,
     display_name: String,
     role_description: String,
     avatar_color: Option<String>,
     adapter: Option<String>,
     executable_path: Option<String>,
     chatbot_provider: Option<String>,
     api_key: Option<String>,
     model: Option<String>,
     login_username: Option<String>,
     login_password: Option<String>,
     existing_auth_user_id: Option<String>,
     /// kind=user：创建待接受邀请占位成员 + 24h 链接
     invite: Option<bool>,
 }

 #[derive(Debug, Serialize)]
 #[serde(rename_all = "camelCase")]
 struct MemberWithInvite {
     #[serde(flatten)]
     member: Member,
     #[serde(skip_serializing_if = "Option::is_none")]
     invite_token: Option<String>,
     #[serde(skip_serializing_if = "Option::is_none")]
     invite_url: Option<String>,
     #[serde(skip_serializing_if = "Option::is_none")]
     invite_expires_at: Option<i64>,
 }

 async fn list_joinable_users_web(
     State(state): State<Arc<AppState>>,
     ClaimsExtractor(claims): ClaimsExtractor,
     axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
 ) -> Result<Json<Vec<crate::models::JoinableUser>>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
     let group_id = q
         .get("groupId")
         .map(|s| s.as_str())
         .filter(|s| !s.is_empty())
         .ok_or_else(|| (StatusCode::BAD_REQUEST, "缺少 groupId".into()))?;
     db_get_group(&conn, group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
     crate::db::list_joinable_users(&conn, group_id)
         .map(Json)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn add_member_web(
     State(state): State<Arc<AppState>>,
     ClaimsExtractor(claims): ClaimsExtractor,
     Json(input): Json<AddMemberInputWeb>,
 ) -> Result<Json<MemberWithInvite>, (StatusCode, String)> {
     if !matches!(input.kind.as_str(), "user" | "agent" | "chatbot") || input.display_name.trim().is_empty() {
         return Err((StatusCode::BAD_REQUEST, "invalid member kind or name".into()));
     }
     let invite_mode = input.invite.unwrap_or(false);
     if invite_mode && input.kind != "user" {
         return Err((StatusCode::BAD_REQUEST, "仅用户成员支持邀请链接".into()));
     }
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
     let group = db_get_group(&conn, &input.group_id)
         .map_err(|e| (StatusCode::NOT_FOUND, e))?;
     if input.kind == "chatbot" && group.group_kind != "chat" {
         let exists: i64 = conn
             .query_row(
                 "SELECT COUNT(*) FROM members WHERE group_id=?1 AND kind='chatbot' AND is_active=1",
                 params![input.group_id],
                 |r| r.get(0),
             )
             .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
         if exists > 0 {
             return Err((StatusCode::CONFLICT, "项目群只能添加一个聊天机器人；聊天群可添加多个。".into()));
         }
     }
     let member_id = id();
     let created_at = now();
     let color = input.avatar_color.unwrap_or_else(|| match input.kind.as_str() {
         "agent" => "#17a673".into(),
         "chatbot" => "#0ea5a0".into(),
         _ => "#5167f6".into(),
     });
     let mut auth_user_id: Option<String> = None;
     if input.kind == "user" && !invite_mode {
         auth_user_id = Some(
             crate::db::resolve_user_member_auth_id(
                 &conn,
                 &input.group_id,
                 input.existing_auth_user_id.as_deref(),
                 input.login_username.as_deref(),
                 input.login_password.as_deref(),
             )
             .map_err(|e| {
                 let status = if e.contains("已被占用") || e.contains("已是本群") {
                     StatusCode::CONFLICT
                 } else if e.contains("不存在") {
                     StatusCode::NOT_FOUND
                 } else {
                     StatusCode::BAD_REQUEST
                 };
                 (status, e)
             })?,
         );
     }
     conn.execute(
         "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,auth_user_id) VALUES(?1,?2,?3,?4,?5,?6,1,?7,?8)",
         params![
             member_id,
             input.group_id,
             input.kind,
             input.display_name.trim(),
             color,
             input.role_description.trim(),
             created_at,
             auth_user_id
         ],
     )
     .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     if input.kind == "agent" {
         let adapter = input.adapter.unwrap_or_else(|| "mock".into());
         let default_ws = if group.workspace_path.trim().is_empty() {
             None
         } else {
             let _ = crate::memory::ensure_linlis_layout(
                 std::path::Path::new(&group.workspace_path),
                 Some(&member_id),
             );
             Some(
                 crate::memory::default_agent_workspace(
                     std::path::Path::new(&group.workspace_path),
                     &member_id,
                 )
                 .to_string_lossy()
                 .into_owned(),
             )
         };
         let model = input
             .model
             .as_deref()
             .map(str::trim)
             .filter(|s| !s.is_empty());
         conn.execute(
             "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at,workspace_path,warm_status,model) VALUES(?1,?2,?3,'unknown',?4,?5,'cold',?6)",
             params![
                 member_id,
                 adapter,
                 input.executable_path.filter(|p| !p.trim().is_empty()),
                 created_at,
                 default_ws,
                 model
             ],
         )
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     } else if input.kind == "chatbot" {
         let provider = input
             .chatbot_provider
             .as_deref()
             .unwrap_or("opencode-go");
         let adapter = crate::adapters::chatbot::normalize_adapter(provider)
             .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
         let api_key = input
             .api_key
             .as_deref()
             .map(str::trim)
             .filter(|s| !s.is_empty())
             .ok_or_else(|| (StatusCode::BAD_REQUEST, "聊天机器人必须填写 API Key".into()))?;
         let model = input
             .model
             .as_deref()
             .map(str::trim)
             .filter(|s| !s.is_empty())
             .unwrap_or("deepseek-v4-flash");
         conn.execute(
             "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at,api_key,warm_status,model) VALUES(?1,?2,NULL,'ready',?3,?4,'cold',?5)",
             params![member_id, adapter, created_at, api_key, model],
         )
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     }
     let member = crate::db::get_member(&conn, &member_id)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let mut invite_token = None;
     let mut invite_url = None;
     let mut invite_expires_at = None;
     if invite_mode {
         let inv = crate::db::create_group_invite(&conn, &input.group_id, &member_id, Some(&claims.sub))
             .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
         invite_url = Some(format!("/invite/{}", inv.token));
         invite_token = Some(inv.token);
         invite_expires_at = Some(inv.expires_at);
     }
     logger::info(&conn, "member", &format!("member added: {} ({}) in group {}", input.display_name, input.kind, input.group_id), None);
     web_emit(&state.tx, &input.group_id, "member_added", None, None, None, None);
     Ok(Json(MemberWithInvite {
         member,
         invite_token,
         invite_url,
         invite_expires_at,
     }))
 }

 async fn remove_member_web(
     State(state): State<Arc<AppState>>,
     ClaimsExtractor(claims): ClaimsExtractor,
     Path((group_id, member_id)): Path<(String, String)>,
 ) -> Result<Json<()>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
     let group = db_get_group(&conn, &group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
     if group.owner_member_id == member_id {
         return Err((StatusCode::FORBIDDEN, "cannot remove owner".into()));
     }
     conn.execute("UPDATE members SET is_active=0 WHERE id=?1", params![member_id])
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     logger::warn(&conn, "member", &format!("member {} removed from group {}", member_id, group_id), None);
     web_emit(&state.tx, &group_id, "member_removed", None, None, None, None);
     Ok(Json(()))
 }

 async fn purge_member_web(
     State(state): State<Arc<AppState>>,
     ClaimsExtractor(claims): ClaimsExtractor,
     Path((group_id, member_id)): Path<(String, String)>,
 ) -> Result<Json<()>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
     crate::db::hard_delete_member(&conn, &group_id, &member_id).map_err(|e| {
         let status = if e.contains("群主") {
             StatusCode::FORBIDDEN
         } else if e.contains("找不到") {
             StatusCode::NOT_FOUND
         } else {
             StatusCode::BAD_REQUEST
         };
         (status, e)
     })?;
     logger::warn(
         &conn,
         "member",
         &format!("member {} purged from group {}", member_id, group_id),
         None,
     );
     web_emit(&state.tx, &group_id, "member_removed", None, None, None, None);
     Ok(Json(()))
 }

 async fn preview_invite_web(
     State(state): State<Arc<AppState>>,
     Path(token): Path<String>,
 ) -> Result<Json<crate::db::InvitePreview>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     crate::db::preview_invite(&conn, &token)
         .map(Json)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn accept_invite_web(
     State(state): State<Arc<AppState>>,
     ClaimsExtractor(claims): ClaimsExtractor,
     Path(token): Path<String>,
 ) -> Result<Json<Member>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let member = crate::db::accept_invite(&conn, &token, &claims.sub).map_err(|e| {
         let status = if e.contains("已是本群") {
             StatusCode::CONFLICT
         } else if e.contains("过期") || e.contains("无效") || e.contains("已使用") || e.contains("已移除") {
             StatusCode::GONE
         } else {
             StatusCode::BAD_REQUEST
         };
         (status, e)
     })?;
     web_emit(&state.tx, &member.group_id, "member_updated", None, None, None, None);
     Ok(Json(member))
 }

 async fn set_admin_web(
     State(state): State<Arc<AppState>>,
     ClaimsExtractor(claims): ClaimsExtractor,
     Path(group_id): Path<String>,
     Json(body): Json<serde_json::Value>,
 ) -> Result<Json<GroupState>, (StatusCode, String)> {
     let member_id = body
         .get("memberId")
         .or_else(|| body.get("member_id"))
         .and_then(|v| v.as_str());
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
     let _ = db_get_group(&conn, &group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
     if let Some(id) = member_id {
         let ok = crate::db::member_is_default_responder_candidate(&conn, &group_id, id)
             .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
         if !ok {
             return Err((
                 StatusCode::BAD_REQUEST,
                 "默认响应者必须是本群活跃的 Agent 或聊天机器人".into(),
             ));
         }
     }
     conn.execute("UPDATE groups SET admin_member_id=?1 WHERE id=?2", params![member_id, group_id])
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     // Only admin *agent* gets keep-alive; chatbots stay cold / fast HTTP.
     conn.execute(
         "UPDATE agent_profiles SET keep_alive=0, updated_at=?1 WHERE member_id IN (SELECT id FROM members WHERE group_id=?2 AND kind='agent')",
         params![now(), group_id],
     )
     .ok();
     if let Some(id) = member_id {
         let kind: String = conn
             .query_row("SELECT kind FROM members WHERE id=?1", params![id], |r| r.get(0))
             .unwrap_or_default();
         if kind == "agent" {
             conn.execute(
                 "UPDATE agent_profiles SET keep_alive=1, warm_status='warming', updated_at=?1 WHERE member_id=?2",
                 params![now(), id],
             )
             .ok();
         }
     }
     group_state(&conn, &group_id).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveBody {
    archived: bool,
}

async fn put_group_archive_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Path(group_id): Path<String>,
    Json(body): Json<ArchiveBody>,
) -> Result<Json<crate::models::Group>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
    crate::db::set_group_archived(&conn, &group_id, body.archived)
        .map(Json)
        .map_err(|e| (StatusCode::NOT_FOUND, e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberModelBody {
    model: Option<String>,
}

async fn put_member_model_web(
    State(state): State<Arc<AppState>>,
    Path(member_id): Path<String>,
    Json(body): Json<MemberModelBody>,
) -> Result<Json<Member>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    crate::db::set_member_model(&conn, &member_id, body.model.as_deref())
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;
    conn.query_row(
        &format!("{} WHERE m.id=?1", crate::db::MEMBER_SELECT),
        params![member_id],
        member_from_row,
    )
    .map(Json)
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberWorkspaceBody {
    workspace_path: String,
}

async fn put_member_workspace_web(
    State(state): State<Arc<AppState>>,
    Path(member_id): Path<String>,
    Json(body): Json<MemberWorkspaceBody>,
) -> Result<Json<Member>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let member = conn
        .query_row(
            "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,COALESCE(m.tags,''),m.created_at,p.workspace_path,p.api_key,COALESCE(p.keep_alive,0),p.warm_status,p.model,m.auth_user_id FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.id=?1",
            params![member_id],
            member_from_row,
        )
        .map_err(|_| (StatusCode::NOT_FOUND, "member not found".into()))?;
    if member.kind != "agent" {
        return Err((StatusCode::BAD_REQUEST, "仅 Agent 可设置工作区".into()));
    }
    let group = db_get_group(&conn, &member.group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
    let resolved = crate::memory::resolve_agent_workspace(
        std::path::Path::new(&group.workspace_path),
        &member_id,
        Some(&body.workspace_path),
        group.is_system,
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    crate::db::set_member_workspace(&conn, &member_id, resolved.to_string_lossy().as_ref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let member = conn
        .query_row(
            "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,COALESCE(m.tags,''),m.created_at,p.workspace_path,p.api_key,COALESCE(p.keep_alive,0),p.warm_status,p.model,m.auth_user_id FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.id=?1",
            params![member_id],
            member_from_row,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(member))
}

// === Messages (history pages) ===

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListMessagesQuery {
    before_created_at: Option<i64>,
    before_id: Option<String>,
    limit: Option<i64>,
}

async fn list_messages_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Path(group_id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<ListMessagesQuery>,
) -> Result<Json<crate::models::MessagePage>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_group_access(&conn, &claims.sub, &group_id).map_err(map_acl_err)?;
    let _ = db_get_group(&conn, &group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let (before_created_at, before_id) = match (q.before_created_at, q.before_id.as_deref()) {
        (Some(ts), Some(id)) if !id.is_empty() => (ts, id.to_string()),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "beforeCreatedAt and beforeId are required for older pages".into(),
            ));
        }
    };
    let messages = get_messages_before(&conn, &group_id, before_created_at, &before_id, limit)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let has_more = messages.len() as i64 >= limit;
    Ok(Json(crate::models::MessagePage {
        messages: crate::db::project_messages_for_client(messages),
        has_more,
    }))
}

async fn get_message_channel_part_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Path((group_id, message_id, channel)): Path<(String, String, String)>,
) -> Result<Json<crate::models::MessageChannelPart>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_group_access(&conn, &claims.sub, &group_id).map_err(map_acl_err)?;
    let text = crate::db::get_message_channel_text(&conn, &group_id, &message_id, &channel)
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;
    Ok(Json(crate::models::MessageChannelPart {
        message_id,
        channel: crate::message_content::normalize_channel(&channel),
        text,
    }))
}

 // === Runs ===

 async fn list_runs_web(
     State(state): State<Arc<AppState>>,
     Path(group_id): Path<String>,
 ) -> Result<Json<Vec<TaskRun>>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     get_runs(&conn, &group_id).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn cancel_run_web(
     State(state): State<Arc<AppState>>,
     Path(run_id): Path<String>,
 ) -> Result<Json<()>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let changed = conn
         .execute("UPDATE task_runs SET status='cancelled',completed_at=?1 WHERE id=?2 AND status IN ('queued','running')", params![now(), run_id])
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     if changed > 0 {
         conn.execute("UPDATE messages SET status='cancelled' WHERE id=(SELECT output_message_id FROM task_runs WHERE id=?1) AND status='streaming'", params![run_id])
             .ok();
         logger::warn(&conn, "run", &format!("run {} cancelled", run_id), None);
         web_emit(&state.tx, "", "run_status", None, Some(&run_id), Some("cancelled"), None);
     }
     Ok(Json(()))
 }

 async fn retry_run_web(
     State(state): State<Arc<AppState>>,
     Path(run_id): Path<String>,
 ) -> Result<Json<String>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let old: TaskRun = conn
         .query_row("SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at FROM task_runs WHERE id=?1", params![run_id], crate::db::run_from_row)
         .map_err(|e| (StatusCode::NOT_FOUND, format!("run not found: {e}")))?;
     let new_id = create_task_run(&conn, &old.group_id, &old.root_message_id, &old.agent_member_id, old.parent_run_id.as_deref(), old.depth)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     logger::info(&conn, "run", &format!("run {} retried as {}", run_id, new_id), None);
     Ok(Json(new_id))
 }

 // === Settings ===

 async fn get_settings_web(
     State(state): State<Arc<AppState>>,
 ) -> Result<Json<RuntimeSettings>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     get_settings_from(&conn).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn update_settings_web(
     State(state): State<Arc<AppState>>,
     Json(settings): Json<RuntimeSettings>,
 ) -> Result<Json<RuntimeSettings>, (StatusCode, String)> {
     if settings.max_concurrent_runs < 1
         || settings.run_timeout_seconds < 30
         || settings.context_message_limit < 5
         || !(5..=40).contains(&settings.chat_context_message_limit)
         || !(0..=4).contains(&settings.max_delegation_depth)
     {
         return Err((StatusCode::BAD_REQUEST, "settings out of range".into()));
     }
     if !(1..=30).contains(&settings.heartbeat_focus_seconds)
         || !(1..=60).contains(&settings.heartbeat_background_seconds)
     {
         return Err((StatusCode::BAD_REQUEST, "heartbeat settings out of range".into()));
     }
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     for (key, value) in [
         ("max_concurrent_runs", settings.max_concurrent_runs),
         ("run_timeout_seconds", settings.run_timeout_seconds),
         ("context_message_limit", settings.context_message_limit),
         (
             "chat_context_message_limit",
             settings.chat_context_message_limit,
         ),
         ("max_delegation_depth", settings.max_delegation_depth),
         (
             "heartbeat_auto",
             if settings.heartbeat_auto { 1 } else { 0 },
         ),
         ("heartbeat_focus_seconds", settings.heartbeat_focus_seconds),
         (
             "heartbeat_background_seconds",
             settings.heartbeat_background_seconds,
         ),
     ] {
         conn.execute(
             "INSERT INTO app_settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
             params![key, value.to_string()],
         )
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     }
     get_settings_from(&conn).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn health_web() -> Json<serde_json::Value> {
     Json(serde_json::json!({ "ok": true, "service": "linlis-work-panel" }))
 }

 async fn metrics_latest_web(
     State(state): State<Arc<AppState>>,
 ) -> Result<Json<crate::metrics::Sample>, (StatusCode, String)> {
     crate::metrics::latest_or_from_db(&state.db_path)
         .map(Json)
         .ok_or((StatusCode::NOT_FOUND, "no metrics sample yet".into()))
 }

 async fn list_active_runs_web(
     State(state): State<Arc<AppState>>,
     ClaimsExtractor(claims): ClaimsExtractor,
     Path(group_id): Path<String>,
 ) -> Result<Json<Vec<TaskRun>>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     require_group_access(&conn, &claims.sub, &group_id).map_err(map_acl_err)?;
     let runs = get_runs(&conn, &group_id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     Ok(Json(
         runs.into_iter()
             .filter(|r| r.status == "queued" || r.status == "running")
             .collect(),
     ))
 }

 // === OCR ===

 async fn ocr_image_web(
     Json(input): Json<serde_json::Value>,
 ) -> Result<Json<String>, (StatusCode, String)> {
     let path = input
         .get("imagePath")
         .or_else(|| input.get("image_path"))
         .and_then(|v| v.as_str())
         .ok_or((StatusCode::BAD_REQUEST, "missing imagePath".into()))?;
     crate::ocr::ocr_image(path).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn ocr_base64_web(
     Json(input): Json<serde_json::Value>,
 ) -> Result<Json<String>, (StatusCode, String)> {
     let data = input
         .get("base64Data")
         .or_else(|| input.get("base64_data"))
         .and_then(|v| v.as_str())
         .ok_or((StatusCode::BAD_REQUEST, "missing base64Data".into()))?;
     crate::ocr::ocr_image_base64(data).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 // === Preset Roles ===

 async fn get_preset_roles_web(
     State(state): State<Arc<AppState>>,
 ) -> Result<Json<Vec<PresetRole>>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    get_preset_roles(&conn).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

 // === PM: Roadmap Items ===

 async fn list_roadmap_items_web(
     State(state): State<Arc<AppState>>,
     Path(group_id): Path<String>,
 ) -> Result<Json<Vec<RoadmapItem>>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     get_roadmap_items(&conn, &group_id).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn create_roadmap_item_web(
     State(state): State<Arc<AppState>>,
     Json(input): Json<CreateRoadmapItemInput>,
 ) -> Result<Json<RoadmapItem>, (StatusCode, String)> {
     if input.title.trim().is_empty() { return Err((StatusCode::BAD_REQUEST, "title required".into())); }
     create_roadmap_item_db(&open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?, &input)
         .map(Json)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn update_roadmap_item_web(
     State(state): State<Arc<AppState>>,
     Path(id): Path<String>,
     Json(input): Json<UpdateRoadmapItemInput>,
 ) -> Result<Json<RoadmapItem>, (StatusCode, String)> {
     update_roadmap_item_db(&open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?, &id, &input)
         .map(Json)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn delete_roadmap_item_web(
     State(state): State<Arc<AppState>>,
     Path(id): Path<String>,
 ) -> Result<Json<()>, (StatusCode, String)> {
     delete_roadmap_item_db(&open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?, &id)
         .map(|_| Json(()))
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 // === PM: Features ===

 async fn list_features_web(
     State(state): State<Arc<AppState>>,
     Path(group_id): Path<String>,
 ) -> Result<Json<Vec<Feature>>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     get_features(&conn, &group_id).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn create_feature_web(
     State(state): State<Arc<AppState>>,
     Json(input): Json<CreateFeatureInput>,
 ) -> Result<Json<Feature>, (StatusCode, String)> {
     if input.title.trim().is_empty() { return Err((StatusCode::BAD_REQUEST, "title required".into())); }
     create_feature_db(&open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?, &input)
         .map(Json)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn update_feature_web(
     State(state): State<Arc<AppState>>,
     Path(id): Path<String>,
     Json(input): Json<UpdateFeatureInput>,
 ) -> Result<Json<Feature>, (StatusCode, String)> {
     update_feature_db(&open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?, &id, &input)
         .map(Json)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn delete_feature_web(
     State(state): State<Arc<AppState>>,
     Path(id): Path<String>,
 ) -> Result<Json<()>, (StatusCode, String)> {
     delete_feature_db(&open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?, &id)
         .map(|_| Json(()))
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 // === PM: Feature Tasks ===

 async fn list_feature_tasks_web(
     State(state): State<Arc<AppState>>,
     Path(feature_id): Path<String>,
 ) -> Result<Json<Vec<FeatureTask>>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     get_feature_tasks(&conn, &feature_id).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn create_feature_task_web(
     State(state): State<Arc<AppState>>,
     Json(input): Json<CreateFeatureTaskInput>,
 ) -> Result<Json<FeatureTask>, (StatusCode, String)> {
     if input.title.trim().is_empty() { return Err((StatusCode::BAD_REQUEST, "title required".into())); }
     create_feature_task_db(&open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?, &input)
         .map(Json)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn update_feature_task_web(
     State(state): State<Arc<AppState>>,
     Path(id): Path<String>,
     Json(input): Json<UpdateFeatureTaskInput>,
 ) -> Result<Json<FeatureTask>, (StatusCode, String)> {
     update_feature_task_db(&open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?, &id, &input)
         .map(Json)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 async fn delete_feature_task_web(
     State(state): State<Arc<AppState>>,
     Path(id): Path<String>,
 ) -> Result<Json<()>, (StatusCode, String)> {
     delete_feature_task_db(&open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?, &id)
         .map(|_| Json(()))
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 // === PM: Aggregated State ===

 async fn get_roadmap_state_web(
     State(state): State<Arc<AppState>>,
     Path(group_id): Path<String>,
 ) -> Result<Json<RoadmapState>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     get_roadmap_state_db(&conn, &group_id).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }

 // === Logs ===

 async fn list_logs_web(
     State(state): State<Arc<AppState>>,
     axum::extract::Query(q): axum::extract::Query<LogQuery>,
 ) -> Result<Json<Vec<LogEntry>>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     logger::query_logs(&conn, &q).map(Json).map_err(|e| {
         logger::error(&conn, "logs", &format!("failed to query logs: {e}"), None);
         (StatusCode::INTERNAL_SERVER_ERROR, e)
     })
 }

 async fn count_logs_web(
     State(state): State<Arc<AppState>>,
     axum::extract::Query(q): axum::extract::Query<LogQuery>,
 ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let count = logger::count_logs(&conn, q.level.as_deref(), q.source.as_deref()).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     Ok(Json(serde_json::json!({"count": count})))
 }

 async fn clear_logs_web(
     State(state): State<Arc<AppState>>,
 ) -> Result<Json<()>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     logger::clear_logs(&conn).map_err(|e| {
         logger::error(&conn, "logs", &format!("failed to clear logs: {e}"), None);
         (StatusCode::INTERNAL_SERVER_ERROR, e)
     })?;
     logger::info(&conn, "logs", "logs cleared by user", None);
     Ok(Json(()))
 }

 // === Shared Memory (Experiences) ===

 #[derive(Deserialize)]
 #[serde(rename_all = "camelCase")]
 struct SaveExperienceInput {
     group_id: String,
     title: String,
     content: String,
     #[serde(default)]
     tags: String,
     /// Optional group member id as the recorded source; falls back to the logged-in user.
     source_member_id: Option<String>,
 }

 #[derive(Deserialize)]
 #[serde(rename_all = "camelCase")]
 struct QueryExperienceInput {
     #[serde(default = "default_group")]
     group_id: String,
     #[serde(default)]
     query: String,
     #[serde(default = "default_limit")]
     limit: i64,
 }

 fn default_group() -> String { String::new() }
 fn default_limit() -> i64 { 20 }

 async fn save_experience_web(
     ClaimsExtractor(claims): ClaimsExtractor,
     State(state): State<Arc<AppState>>,
     Json(input): Json<SaveExperienceInput>,
 ) -> Result<Json<String>, (StatusCode, String)> {
     let conn = crate::db::open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let source_id = input
         .source_member_id
         .as_deref()
         .filter(|value| !value.trim().is_empty())
         .unwrap_or(&claims.sub);
     let eid = crate::db::save_experience(&conn, &input.group_id, source_id, &input.title, &input.content, &input.tags)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     Ok(Json(eid))
 }

 async fn query_experiences_web(
     State(state): State<Arc<AppState>>,
     axum::extract::Query(input): axum::extract::Query<QueryExperienceInput>,
 ) -> Result<Json<Vec<Experience>>, (StatusCode, String)> {
     let conn = crate::db::open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let results = crate::db::query_experiences(&conn, &input.group_id, &input.query, input.limit)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     Ok(Json(results))
 }

 async fn delete_experience_web(
     Path(id): Path<String>,
     State(state): State<Arc<AppState>>,
 ) -> Result<Json<bool>, (StatusCode, String)> {
     let conn = crate::db::open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let ok = crate::db::delete_experience(&conn, &id)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     Ok(Json(ok))
 }

 // === Router Builder ===

/// Start a background task that polls for queued runs periodically
pub fn start_scheduler_background(state: SchedulerState) {
    tokio::spawn(async move {
        // Give server time to start
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        // Get all group IDs
        let groups: Vec<String> = match (|| -> Result<Vec<String>, String> {
            let conn = crate::db::open_db(&state.db_path)?;
            let mut stmt = conn.prepare("SELECT id FROM groups").map_err(|e| e.to_string())?;
            let ids = stmt.query_map([], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();
            Ok(ids)
        })() {
            Ok(ids) => ids,
            Err(_) => return,
        };
        for gid in &groups {
            scheduler::schedule_group(state.clone(), gid.clone());
        }
        // Periodic poll every 5 seconds
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let groups: Vec<String> = match (|| -> Result<Vec<String>, String> {
                let conn = crate::db::open_db(&state.db_path)?;
                let mut stmt = conn.prepare("SELECT DISTINCT group_id FROM task_runs WHERE status='queued'").map_err(|e| e.to_string())?;
                let ids = stmt.query_map([], |r| r.get::<_, String>(0))
                    .map_err(|e| e.to_string())?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(ids)
            })() {
                Ok(ids) => ids,
                Err(_) => continue,
            };
            for gid in &groups {
                scheduler::schedule_group(state.clone(), gid.clone());
            }
        }
    });
}

async fn list_server_dir_web(
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<DirListing>, (StatusCode, String)> {
    let path = q.get("path").map(|s| s.as_str()).unwrap_or("/");
    fs_browse::list_server_dir(path)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MkdirBody {
    parent: String,
    name: String,
}

async fn create_server_dir_web(
    Json(body): Json<MkdirBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = fs_browse::create_server_dir(&body.parent, &body.name)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(json!({
        "path": path.to_string_lossy(),
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnnouncementBody {
    announcement: String,
}

async fn get_announcement_web(
    State(state): State<Arc<AppState>>,
    Path(group_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let g = db_get_group(&conn, &group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
    Ok(Json(json!({
        "groupId": g.id,
        "announcement": g.announcement,
        "announcementUpdatedAt": g.announcement_updated_at,
    })))
}

async fn put_announcement_web(
    State(state): State<Arc<AppState>>,
    Path(group_id): Path<String>,
    Json(body): Json<AnnouncementBody>,
) -> Result<Json<Group>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let g = set_group_announcement(&conn, &group_id, body.announcement.trim())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let _ = fs_browse::sync_announcement_rule(
        std::path::Path::new(&g.workspace_path),
        &g.announcement,
    );
    Ok(Json(g))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceBody {
    workspace_path: String,
}

async fn put_workspace_web(
    State(state): State<Arc<AppState>>,
    Path(group_id): Path<String>,
    Json(body): Json<WorkspaceBody>,
) -> Result<Json<Group>, (StatusCode, String)> {
    let workspace = fs_browse::resolve_server_dir(&body.workspace_path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let g = update_group_workspace(&conn, &group_id, workspace.to_string_lossy().as_ref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(g))
}

async fn list_group_extensions_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Path(group_id): Path<String>,
) -> Result<Json<Vec<crate::extensions::ExtensionStatus>>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_group_access(&conn, &claims.sub, &group_id).map_err(map_acl_err)?;
    db_get_group(&conn, &group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
    crate::extensions::list_group_extensions(&conn, &group_id)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionToggleBody {
    enabled: bool,
}

async fn put_panellive_extension_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Path(group_id): Path<String>,
    Json(body): Json<ExtensionToggleBody>,
) -> Result<Json<crate::extensions::ExtensionStatus>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
    db_get_group(&conn, &group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
    crate::extensions::set_panellive_enabled(&conn, &group_id, body.enabled)
        .map(Json)
        .map_err(|e| {
            if e.starts_with("CONFLICT:") {
                (StatusCode::CONFLICT, e.trim_start_matches("CONFLICT:").to_string())
            } else {
                (StatusCode::BAD_REQUEST, e)
            }
        })
}

async fn proxy_panellive(
    method: axum::http::Method,
    Path(path): Path<String>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, (StatusCode, String)> {
    let upstream_path = crate::extensions::sanitize_proxy_path(&path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let root = crate::extensions::panellive_root();
    let manifest = crate::extensions::load_panellive_manifest(&root)
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    let port = crate::extensions::panellive_upstream_port(&manifest);
    let ct = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let method_s = method.as_str();
    if method_s != "GET" && method_s != "POST" && method_s != "OPTIONS" {
        return Err((StatusCode::METHOD_NOT_ALLOWED, "仅支持 GET/POST".into()));
    }
    if method_s == "OPTIONS" {
        return Ok((
            StatusCode::NO_CONTENT,
            [
                (axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                (axum::http::header::ACCESS_CONTROL_ALLOW_METHODS, "GET,POST,OPTIONS"),
                (axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type,X-Panellive-Token"),
            ],
            Vec::new(),
        )
            .into_response());
    }
    let body_opt = if method_s == "GET" {
        None
    } else {
        Some(body.as_ref())
    };
    let ex = crate::extensions::http_exchange_local(
        method_s,
        "127.0.0.1",
        port,
        &upstream_path,
        body_opt,
        ct.as_deref(),
    )
    .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    let status = StatusCode::from_u16(ex.status).unwrap_or(StatusCode::BAD_GATEWAY);
    Ok((
        status,
        [(axum::http::header::CONTENT_TYPE, ex.content_type)],
        ex.body,
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PanelliveEventBody {
    #[serde(default)]
    task_id: Option<String>,
    skill: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    group_id: Option<String>,
    #[serde(default)]
    payload: serde_json::Value,
    #[serde(default)]
    ts: Option<i64>,
}

async fn panellive_events_web(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<PanelliveEventBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let expected = crate::extensions::panellive_token();
    let got = headers
        .get("X-Panellive-Token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if got != expected {
        return Err((StatusCode::UNAUTHORIZED, "invalid X-Panellive-Token".into()));
    }
    crate::a2a::validate_live_skill(&body.skill).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    // Events whitelist: transcribe + session signals (A1: no chat persistence).
    if !matches!(
        body.skill.as_str(),
        "live.transcribe.result" | "live.session.start" | "live.session.stop" | "live.session.cancel"
    ) {
        return Err((StatusCode::BAD_REQUEST, "events 入口不接受此 skill".into()));
    }
    crate::a2a::validate_events_payload(&body.payload).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let group_id = body.group_id.unwrap_or_default();
    let text = body
        .payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    web_emit_delta(
        &state.tx,
        &group_id,
        "live_event",
        body.task_id.as_deref(),
        body.session_id.as_deref(),
        Some(&body.skill),
        None,
        if text.is_empty() { None } else { Some(text) },
    );
    Ok(Json(json!({
        "ok": true,
        "skill": body.skill,
        "sessionId": body.session_id,
        "note": "broadcast via WS; not written to group messages",
        "ts": body.ts,
    })))
}

async fn dispatch_a2a_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
    Json(mut envelope): Json<crate::a2a::A2aEnvelope>,
) -> Result<Json<crate::a2a::A2aDispatchResult>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_group_access(&conn, &claims.sub, &envelope.group_id).map_err(map_acl_err)?;
    if envelope.group_id.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "groupId 必填".into()));
    }
    // Live skills require extension loaded for the group.
    if envelope.skill.starts_with("live.") {
        let enabled = crate::extensions::is_extension_enabled(
            &conn,
            &envelope.group_id,
            crate::extensions::PANELLIVE_ID,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        if !enabled {
            return Err((
                StatusCode::CONFLICT,
                "PanelLive 未 load：请在运行设置中开启 Live 扩展".into(),
            ));
        }
    }
    if envelope.source.is_none() {
        envelope.source = Some(claims.username.clone());
    }
    crate::a2a::dispatch_live_skill(&envelope, Some(&state.sched))
        .map(Json)
        .map_err(|e| {
            let status = if e.contains("禁止") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, e)
        })
}

async fn ops_release_status_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
) -> Result<Json<ops::ReleaseStatus>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
    Ok(Json(ops::release_status().await))
}

async fn ops_job_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
) -> Result<Json<ops::OpsJobState>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
    Ok(Json(ops::ops_job_snapshot()))
}

async fn ops_test_gate_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
    ops::kickoff_test_gate().map_err(|e| (StatusCode::CONFLICT, e))?;
    Ok(Json(json!({ "ok": true, "job": ops::ops_job_snapshot() })))
}

async fn ops_deploy_canary_web(
    State(state): State<Arc<AppState>>,
    ClaimsExtractor(claims): ClaimsExtractor,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    require_admin(&conn, &claims.sub).map_err(map_acl_err)?;
    ops::kickoff_deploy_canary().map_err(|e| (StatusCode::CONFLICT, e))?;
    Ok(Json(json!({ "ok": true, "job": ops::ops_job_snapshot() })))
}

// === Roadmap orchestration ===

async fn start_roadmap_item_web(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::RoadmapOrchestration>, (StatusCode, String)> {
    crate::orchestrator::start_roadmap_item(&state.db_path, &id, &state.tx, state.sched.clone())
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn pause_orchestration_web(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::RoadmapOrchestration>, (StatusCode, String)> {
    crate::orchestrator::pause_orchestration(&state.db_path, &id, &state.tx)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn resume_orchestration_web(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::RoadmapOrchestration>, (StatusCode, String)> {
    crate::orchestrator::resume_orchestration(&state.db_path, &id, &state.tx, state.sched.clone())
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn cancel_orchestration_web(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::RoadmapOrchestration>, (StatusCode, String)> {
    crate::orchestrator::cancel_orchestration(&state.db_path, &id, &state.tx)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn list_orchestrations_web(
    State(state): State<Arc<AppState>>,
    Path(group_id): Path<String>,
) -> Result<Json<Vec<crate::models::RoadmapOrchestration>>, (StatusCode, String)> {
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    crate::orchestrator::ensure_orchestrations_table(&conn)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    crate::orchestrator::list_orchestrations(&conn, &group_id)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let auth_routes = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/health", get(health_web))
        .route("/api/invites/{token}", get(preview_invite_web));

    let protected = Router::new()
        .route("/api/auth/verify", get(verify))
         // Groups
        .route("/api/groups", get(list_groups))
         .route("/api/groups", post(create_group_web))
        .route("/api/groups/{id}", get(get_group))
        .route("/api/groups/{id}/read", put(mark_group_read_web))
        .route("/api/presence", get(list_presence_web))
        .route("/api/groups/{id}/announcement", get(get_announcement_web).put(put_announcement_web))
        .route("/api/groups/{id}/workspace", put(put_workspace_web))
        .route("/api/groups/{id}/archive", put(put_group_archive_web))
        .route("/api/groups/{id}/extensions", get(list_group_extensions_web))
        .route(
            "/api/groups/{id}/extensions/panellive",
            put(put_panellive_extension_web),
        )
        .route("/api/a2a/dispatch", post(dispatch_a2a_web))
        .route("/api/members/{member_id}/model", put(put_member_model_web))
        .route("/api/fs/list", get(list_server_dir_web))
        .route("/api/fs/mkdir", post(create_server_dir_web))
        .route("/api/ops/release-status", get(ops_release_status_web))
        .route("/api/ops/job", get(ops_job_web))
        .route("/api/ops/test-gate", post(ops_test_gate_web))
        .route("/api/ops/deploy-canary", post(ops_deploy_canary_web))
         // Members
         .route("/api/groups/{group_id}/members", post(add_member_web))
         .route("/api/groups/{group_id}/members/{member_id}", delete(remove_member_web))
         .route(
             "/api/groups/{group_id}/members/{member_id}/purge",
             delete(purge_member_web),
         )
         .route("/api/invites/{token}/accept", post(accept_invite_web))
         .route("/api/users/joinable", get(list_joinable_users_web))
         .route("/api/members/{member_id}/workspace", put(put_member_workspace_web))
         .route("/api/groups/{group_id}/admin", put(set_admin_web))
         // Messages
        .route("/api/messages", post(send_message_web))
        .route("/api/groups/{group_id}/messages", get(list_messages_web))
        .route(
            "/api/groups/{group_id}/messages/{message_id}/parts/{channel}",
            get(get_message_channel_part_web),
        )
         // Runs
         .route("/api/groups/{group_id}/runs", get(list_runs_web))
         .route("/api/groups/{group_id}/runs/active", get(list_active_runs_web))
         .route("/api/runs/{run_id}/cancel", post(cancel_run_web))
         .route("/api/runs/{run_id}/retry", post(retry_run_web))
         // Settings / metrics
         .route("/api/settings", get(get_settings_web))
         .route("/api/settings", put(update_settings_web))
         .route("/api/metrics/latest", get(metrics_latest_web))
         // OCR
         .route("/api/ocr", post(ocr_image_web))
         .route("/api/ocr/base64", post(ocr_base64_web))
         // Preset Roles
         .route("/api/preset-roles", get(get_preset_roles_web))
         // PM: Roadmap Items
         .route("/api/groups/{group_id}/roadmap", get(list_roadmap_items_web))
         .route("/api/roadmap-items", post(create_roadmap_item_web))
         .route("/api/roadmap-items/{id}", put(update_roadmap_item_web))
         .route("/api/roadmap-items/{id}", delete(delete_roadmap_item_web))
         .route("/api/roadmap-items/{id}/start", post(start_roadmap_item_web))
         .route("/api/roadmap-orchestrations/{id}/pause", post(pause_orchestration_web))
         .route("/api/roadmap-orchestrations/{id}/resume", post(resume_orchestration_web))
         .route("/api/roadmap-orchestrations/{id}/cancel", post(cancel_orchestration_web))
         .route("/api/groups/{group_id}/roadmap-orchestrations", get(list_orchestrations_web))
         // PM: Features
         .route("/api/groups/{group_id}/features", get(list_features_web))
         .route("/api/features", post(create_feature_web))
         .route("/api/features/{id}", put(update_feature_web))
         .route("/api/features/{id}", delete(delete_feature_web))
         // PM: Feature Tasks
         .route("/api/features/{feature_id}/tasks", get(list_feature_tasks_web))
         .route("/api/feature-tasks", post(create_feature_task_web))
         .route("/api/feature-tasks/{id}", put(update_feature_task_web))
         .route("/api/feature-tasks/{id}", delete(delete_feature_task_web))
         // PM: Aggregated State
         .route("/api/groups/{group_id}/roadmap-state", get(get_roadmap_state_web))
         // Logs
         .route("/api/logs", get(list_logs_web))
         .route("/api/logs/count", get(count_logs_web))
         .route("/api/logs", delete(clear_logs_web))
         // Shared Memory (Experiences)
         .route("/api/experiences", post(save_experience_web))
         .route("/api/experiences", get(query_experiences_web))
         .route("/api/experiences/{id}", delete(delete_experience_web))
         // WebSocket
        .route("/ws", get(ws_handler))
        .route_layer(middleware::from_fn(auth_middleware));

    // Same-origin PanelLive proxy + events (no JWT — iframe cannot send Bearer).
    // Events use X-Panellive-Token. Media/API proxied to 127.0.0.1:8790 server-side.
    let panellive_public = Router::new()
        .route("/api/extensions/panellive/events", post(panellive_events_web))
        .route("/api/extensions/panellive/{*path}", any(proxy_panellive));

    Router::new()
        .merge(auth_routes)
        .merge(panellive_public)
        .merge(protected)
        .with_state(state)
}
