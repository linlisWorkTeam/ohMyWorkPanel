use axum::{
    extract::{Path, State, WebSocketUpgrade},
    extract::ws::{Message as WsMessage, WebSocket},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::auth::Claims;
use crate::db::{
    active_agent_ids, create_feature_db, create_feature_task_db,
    create_roadmap_item_db, create_task_run, delete_feature_db,
    delete_feature_task_db, delete_roadmap_item_db, get_features,
    get_feature_tasks, get_groups,
    get_group as db_get_group,
    get_preset_roles, get_roadmap_items, get_roadmap_state_db, get_runs,
    get_settings_from, group_state, id, member_from_row, now, open_db,
    update_feature_db, update_feature_task_db, update_roadmap_item_db,
};
use crate::models::{
    CreateFeatureInput, CreateFeatureTaskInput, CreateRoadmapItemInput, Feature,
    FeatureTask, Group, GroupState, Member, Message, PresetRole, RoadmapItem,
    RoadmapState, RuntimeSettings, TaskRun, UpdateFeatureInput, UpdateFeatureTaskInput,
    UpdateRoadmapItemInput,
};
// Helper to emit events to WebSocket clients
fn web_emit(tx: &broadcast::Sender<String>, group_id: &str, kind: &str, message_id: Option<&str>, run_id: Option<&str>, status: Option<&str>, error: Option<&str>) {
    let mut obj = serde_json::Map::new();
    obj.insert("kind".into(), json!(kind));
    obj.insert("group_id".into(), json!(group_id));
    if let Some(v) = message_id { obj.insert("message_id".into(), json!(v)); }
    if let Some(v) = run_id { obj.insert("run_id".into(), json!(v)); }
    if let Some(v) = status { obj.insert("status".into(), json!(v)); }
    if let Some(v) = error { obj.insert("error".into(), json!(v)); }
    let _ = tx.send(serde_json::to_string(&obj).unwrap_or_default());
}

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
    preset_roles: Option<Vec<String>>,
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

    group_state(&conn, &group_id).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

// === Message Routes ===

#[derive(Debug, Deserialize)]
struct SendMessageInput {
    group_id: String,
    sender_member_id: String,
    content: String,
    mention_member_ids: Vec<String>,
}

async fn send_message_web(
    State(state): State<Arc<AppState>>,
    Json(input): Json<SendMessageInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if input.content.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty message".into()));
    }
    let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
 
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
 
     // Find target agents
     let mut target_agents = active_agent_ids(&conn, &input.group_id, &input.mention_member_ids)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     if target_agents.is_empty() {
         if let Some(admin) = group.admin_member_id {
             target_agents.push(admin);
         }
     }
 
     // Create task runs for target agents
     let mut run_ids: Vec<String> = Vec::new();
     for agent_id in target_agents {
         match create_task_run(&conn, &input.group_id, &msg.id, &agent_id, None, 0) {
             Ok(rid) => run_ids.push(rid),
             Err(_) => {}
         }
     }
 
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

     Ok(Json(json!({
         "message": msg,
         "runIds": run_ids,
     })))
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

 // === Members CRUD ===
 
 #[derive(Debug, Deserialize)]
 struct AddMemberInputWeb {
     group_id: String,
     kind: String,
     display_name: String,
     role_description: String,
     avatar_color: Option<String>,
     adapter: Option<String>,
     executable_path: Option<String>,
 }
 
 async fn add_member_web(
     State(state): State<Arc<AppState>>,
     Json(input): Json<AddMemberInputWeb>,
 ) -> Result<Json<Member>, (StatusCode, String)> {
     if !matches!(input.kind.as_str(), "user" | "agent") || input.display_name.trim().is_empty() {
         return Err((StatusCode::BAD_REQUEST, "invalid member kind or name".into()));
     }
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let _ = db_get_group(&conn, &input.group_id)
         .map_err(|e| (StatusCode::NOT_FOUND, e))?;
     let member_id = id();
     let created_at = now();
     let color = input.avatar_color.unwrap_or_else(|| {
         if input.kind == "agent" { "#17a673".into() } else { "#5167f6".into() }
     });
     conn.execute(
         "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES(?1,?2,?3,?4,?5,?6,1,?7)",
         params![member_id, input.group_id, input.kind, input.display_name.trim(), color, input.role_description.trim(), created_at],
     )
     .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     if input.kind == "agent" {
         let adapter = input.adapter.unwrap_or_else(|| "mock".into());
         conn.execute(
             "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at) VALUES(?1,?2,?3,'unknown',?4)",
             params![member_id, adapter, input.executable_path.filter(|p| !p.trim().is_empty()), created_at],
         )
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     }
     let member = conn
         .query_row(
             "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,m.created_at FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.id=?1",
             params![member_id],
             member_from_row,
         )
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     Ok(Json(member))
 }
 
 async fn remove_member_web(
     State(state): State<Arc<AppState>>,
     Path((group_id, member_id)): Path<(String, String)>,
 ) -> Result<Json<()>, (StatusCode, String)> {
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let group = db_get_group(&conn, &group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
     if group.owner_member_id == member_id {
         return Err((StatusCode::FORBIDDEN, "cannot remove owner".into()));
     }
     conn.execute("UPDATE members SET is_active=0 WHERE id=?1", params![member_id])
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     web_emit(&state.tx, &group_id, "member_removed", None, None, None, None);
     Ok(Json(()))
 }
 
 async fn set_admin_web(
     State(state): State<Arc<AppState>>,
     Path(group_id): Path<String>,
     Json(body): Json<serde_json::Value>,
 ) -> Result<Json<GroupState>, (StatusCode, String)> {
     let member_id = body.get("member_id").and_then(|v| v.as_str());
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     let _ = db_get_group(&conn, &group_id).map_err(|e| (StatusCode::NOT_FOUND, e))?;
     if let Some(id) = member_id {
         let valid = conn
             .query_row("SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND kind='agent' AND is_active=1", params![id, group_id], |r| r.get::<_, i64>(0))
             .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
         if valid != 1 {
             return Err((StatusCode::BAD_REQUEST, "admin must be active agent".into()));
         }
     }
     conn.execute("UPDATE groups SET admin_member_id=?1 WHERE id=?2", params![member_id, group_id])
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     group_state(&conn, &group_id).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
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
         .query_row("SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,created_at,started_at,completed_at FROM task_runs WHERE id=?1", params![run_id], crate::db::run_from_row)
         .map_err(|e| (StatusCode::NOT_FOUND, format!("run not found: {e}")))?;
     let new_id = create_task_run(&conn, &old.group_id, &old.root_message_id, &old.agent_member_id, old.parent_run_id.as_deref(), old.depth)
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
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
     if settings.max_concurrent_runs < 1 || settings.run_timeout_seconds < 30 || settings.context_message_limit < 5 || !(0..=4).contains(&settings.max_delegation_depth) {
         return Err((StatusCode::BAD_REQUEST, "settings out of range".into()));
     }
     let conn = open_db(&state.db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
     for (key, value) in [
         ("max_concurrent_runs", settings.max_concurrent_runs),
         ("run_timeout_seconds", settings.run_timeout_seconds),
         ("context_message_limit", settings.context_message_limit),
         ("max_delegation_depth", settings.max_delegation_depth),
     ] {
         conn.execute(
             "INSERT INTO app_settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
             params![key, value.to_string()],
         )
         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
     }
     get_settings_from(&conn).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }
 
 // === OCR ===
 
 async fn ocr_image_web(
     Json(input): Json<serde_json::Value>,
 ) -> Result<Json<String>, (StatusCode, String)> {
     let path = input.get("image_path").and_then(|v| v.as_str()).ok_or((StatusCode::BAD_REQUEST, "missing image_path".into()))?;
     crate::ocr::ocr_image(path).map(Json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
 }
 
 async fn ocr_base64_web(
     Json(input): Json<serde_json::Value>,
 ) -> Result<Json<String>, (StatusCode, String)> {
     let data = input.get("base64_data").and_then(|v| v.as_str()).ok_or((StatusCode::BAD_REQUEST, "missing base64_data".into()))?;
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
 
// === Router Builder ===

pub fn build_router(state: Arc<AppState>) -> Router {
    let auth_routes = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login));

    let protected = Router::new()
        .route("/api/auth/verify", get(verify))
         // Groups
        .route("/api/groups", get(list_groups))
         .route("/api/groups", post(create_group_web))
        .route("/api/groups/{id}", get(get_group))
         // Members
         .route("/api/groups/{group_id}/members", post(add_member_web))
         .route("/api/groups/{group_id}/members/{member_id}", delete(remove_member_web))
         .route("/api/groups/{group_id}/admin", put(set_admin_web))
         // Messages
        .route("/api/messages", post(send_message_web))
         // Runs
         .route("/api/groups/{group_id}/runs", get(list_runs_web))
         .route("/api/runs/{run_id}/cancel", post(cancel_run_web))
         .route("/api/runs/{run_id}/retry", post(retry_run_web))
         // Settings
         .route("/api/settings", get(get_settings_web))
         .route("/api/settings", put(update_settings_web))
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
         // WebSocket
        .route("/ws", get(ws_handler))
        .route_layer(middleware::from_fn(auth_middleware));

    Router::new().merge(auth_routes).merge(protected).with_state(state)
}
