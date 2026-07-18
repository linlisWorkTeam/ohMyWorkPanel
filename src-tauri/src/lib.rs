use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, BufReader}, process::Command, time::sleep};
use uuid::Uuid;

type AppResult<T> = Result<T, String>;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    scheduling_groups: Arc<Mutex<HashSet<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Group {
    id: String,
    name: String,
    workspace_path: String,
    owner_member_id: String,
    admin_member_id: Option<String>,
    created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Member {
    id: String,
    group_id: String,
    kind: String,
    display_name: String,
    avatar_color: String,
    role_description: String,
    is_active: bool,
    adapter: Option<String>,
    executable_path: Option<String>,
    runtime_status: Option<String>,
    created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Message {
    id: String,
    group_id: String,
    sender_member_id: String,
    parent_run_id: Option<String>,
    content: String,
    status: String,
    created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskRun {
    id: String,
    group_id: String,
    root_message_id: String,
    agent_member_id: String,
    parent_run_id: Option<String>,
    depth: i64,
    status: String,
    output_message_id: Option<String>,
    error_message: Option<String>,
    created_at: i64,
    started_at: Option<i64>,
    completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupState {
    group: Group,
    members: Vec<Member>,
    messages: Vec<Message>,
    runs: Vec<TaskRun>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    groups: Vec<Group>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateGroupInput {
    name: String,
    workspace_path: String,
    owner_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddMemberInput {
    group_id: String,
    kind: String,
    display_name: String,
    role_description: String,
    avatar_color: Option<String>,
    adapter: Option<String>,
    executable_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendResult {
    message: Message,
    run_ids: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChatEvent {
    kind: String,
    group_id: String,
    run_id: Option<String>,
    message_id: Option<String>,
    delta: Option<String>,
    status: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeSettings {
    max_concurrent_runs: i64,
    run_timeout_seconds: i64,
    context_message_limit: i64,
    max_delegation_depth: i64,
}

fn now() -> i64 { Utc::now().timestamp_millis() }
fn id() -> String { Uuid::new_v4().to_string() }

fn open_db(path: &Path) -> AppResult<Connection> {
    let connection = Connection::open(path).map_err(|e| e.to_string())?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
        .map_err(|e| e.to_string())?;
    Ok(connection)
}

fn init_db(path: &Path) -> AppResult<()> {
    let connection = open_db(path)?;
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS groups (
          id TEXT PRIMARY KEY, name TEXT NOT NULL, workspace_path TEXT NOT NULL,
          owner_member_id TEXT NOT NULL, admin_member_id TEXT, created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS members (
          id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
          kind TEXT NOT NULL CHECK(kind IN ('user','agent')), display_name TEXT NOT NULL,
          avatar_color TEXT NOT NULL, role_description TEXT NOT NULL, is_active INTEGER NOT NULL DEFAULT 1,
          created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS agent_profiles (
          member_id TEXT PRIMARY KEY REFERENCES members(id) ON DELETE CASCADE,
          adapter TEXT NOT NULL, executable_path TEXT, runtime_status TEXT NOT NULL DEFAULT 'unknown', updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS messages (
          id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
          sender_member_id TEXT NOT NULL REFERENCES members(id), parent_run_id TEXT,
          content TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS mentions (
          message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
          member_id TEXT NOT NULL REFERENCES members(id), PRIMARY KEY(message_id, member_id)
        );
        CREATE TABLE IF NOT EXISTS task_runs (
          id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
          root_message_id TEXT NOT NULL REFERENCES messages(id), agent_member_id TEXT NOT NULL REFERENCES members(id),
          parent_run_id TEXT, depth INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL,
          output_message_id TEXT, error_message TEXT, created_at INTEGER NOT NULL,
          started_at INTEGER, completed_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS run_events (
          id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES task_runs(id) ON DELETE CASCADE,
          kind TEXT NOT NULL, payload TEXT NOT NULL, created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS idx_messages_group_created ON messages(group_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_runs_group_status ON task_runs(group_id, status, created_at);
        "
    ).map_err(|e| e.to_string())?;
    for (key, value) in [
        ("max_concurrent_runs", "3"), ("run_timeout_seconds", "900"),
        ("context_message_limit", "40"), ("max_delegation_depth", "2"),
    ] {
        connection.execute("INSERT OR IGNORE INTO app_settings(key, value) VALUES (?1, ?2)", params![key, value])
            .map_err(|e| e.to_string())?;
    }
    connection.execute("UPDATE task_runs SET status='interrupted', completed_at=?1 WHERE status IN ('queued','running')", params![now()])
        .map_err(|e| e.to_string())?;
    connection.execute("UPDATE messages SET status='interrupted' WHERE status='streaming'", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn group_from_row(row: &Row<'_>) -> rusqlite::Result<Group> {
    Ok(Group { id: row.get(0)?, name: row.get(1)?, workspace_path: row.get(2)?, owner_member_id: row.get(3)?, admin_member_id: row.get(4)?, created_at: row.get(5)? })
}
fn member_from_row(row: &Row<'_>) -> rusqlite::Result<Member> {
    Ok(Member {
        id: row.get(0)?, group_id: row.get(1)?, kind: row.get(2)?, display_name: row.get(3)?, avatar_color: row.get(4)?,
        role_description: row.get(5)?, is_active: row.get::<_, i64>(6)? != 0, adapter: row.get(7)?, executable_path: row.get(8)?, runtime_status: row.get(9)?, created_at: row.get(10)?,
    })
}
fn message_from_row(row: &Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message { id: row.get(0)?, group_id: row.get(1)?, sender_member_id: row.get(2)?, parent_run_id: row.get(3)?, content: row.get(4)?, status: row.get(5)?, created_at: row.get(6)? })
}
fn run_from_row(row: &Row<'_>) -> rusqlite::Result<TaskRun> {
    Ok(TaskRun { id: row.get(0)?, group_id: row.get(1)?, root_message_id: row.get(2)?, agent_member_id: row.get(3)?, parent_run_id: row.get(4)?, depth: row.get(5)?, status: row.get(6)?, output_message_id: row.get(7)?, error_message: row.get(8)?, created_at: row.get(9)?, started_at: row.get(10)?, completed_at: row.get(11)? })
}

fn get_groups(connection: &Connection) -> AppResult<Vec<Group>> {
    let mut stmt = connection.prepare("SELECT id,name,workspace_path,owner_member_id,admin_member_id,created_at FROM groups ORDER BY created_at DESC").map_err(|e| e.to_string())?;
    stmt.query_map([], group_from_row).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
fn get_group(connection: &Connection, group_id: &str) -> AppResult<Group> {
    connection.query_row("SELECT id,name,workspace_path,owner_member_id,admin_member_id,created_at FROM groups WHERE id=?1", params![group_id], group_from_row)
        .optional().map_err(|e| e.to_string())?.ok_or_else(|| "找不到群聊。".to_string())
}
fn get_members(connection: &Connection, group_id: &str) -> AppResult<Vec<Member>> {
    let mut stmt = connection.prepare(
        "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,m.created_at
         FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.group_id=?1 ORDER BY m.created_at"
    ).map_err(|e| e.to_string())?;
    stmt.query_map(params![group_id], member_from_row).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
fn get_messages(connection: &Connection, group_id: &str) -> AppResult<Vec<Message>> {
    let mut stmt = connection.prepare("SELECT id,group_id,sender_member_id,parent_run_id,content,status,created_at FROM messages WHERE group_id=?1 ORDER BY created_at").map_err(|e| e.to_string())?;
    stmt.query_map(params![group_id], message_from_row).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
fn get_runs(connection: &Connection, group_id: &str) -> AppResult<Vec<TaskRun>> {
    let mut stmt = connection.prepare("SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,created_at,started_at,completed_at FROM task_runs WHERE group_id=?1 ORDER BY created_at").map_err(|e| e.to_string())?;
    stmt.query_map(params![group_id], run_from_row).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
fn group_state(connection: &Connection, group_id: &str) -> AppResult<GroupState> {
    Ok(GroupState { group: get_group(connection, group_id)?, members: get_members(connection, group_id)?, messages: get_messages(connection, group_id)?, runs: get_runs(connection, group_id)? })
}
fn get_settings_from(connection: &Connection) -> AppResult<RuntimeSettings> {
    let get = |key: &str| -> AppResult<i64> {
        connection.query_row("SELECT value FROM app_settings WHERE key=?1", params![key], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?.parse::<i64>().map_err(|_| format!("设置 {key} 不是有效整数"))
    };
    Ok(RuntimeSettings { max_concurrent_runs: get("max_concurrent_runs")?, run_timeout_seconds: get("run_timeout_seconds")?, context_message_limit: get("context_message_limit")?, max_delegation_depth: get("max_delegation_depth")? })
}
fn insert_run_event(connection: &Connection, run_id: &str, kind: &str, payload: &str) -> AppResult<()> {
    connection.execute("INSERT INTO run_events(id,run_id,kind,payload,created_at) VALUES(?1,?2,?3,?4,?5)", params![id(),run_id,kind,payload,now()]).map_err(|e| e.to_string())?;
    Ok(())
}
fn emit(app: &AppHandle, event: ChatEvent) { let _ = app.emit("chat-event", event); }

#[tauri::command]
fn bootstrap(state: State<'_, AppState>) -> AppResult<Bootstrap> {
    Ok(Bootstrap { groups: get_groups(&open_db(&state.db_path)?)? })
}
#[tauri::command]
fn get_group_state(group_id: String, state: State<'_, AppState>) -> AppResult<GroupState> { group_state(&open_db(&state.db_path)?, &group_id) }
#[tauri::command]
fn get_runtime_settings(state: State<'_, AppState>) -> AppResult<RuntimeSettings> { get_settings_from(&open_db(&state.db_path)?) }
#[tauri::command]
fn update_runtime_settings(settings: RuntimeSettings, state: State<'_, AppState>) -> AppResult<RuntimeSettings> {
    if settings.max_concurrent_runs < 1 || settings.run_timeout_seconds < 30 || settings.context_message_limit < 5 || !(0..=4).contains(&settings.max_delegation_depth) { return Err("运行设置超出允许范围。".into()); }
    let conn = open_db(&state.db_path)?;
    for (key, value) in [("max_concurrent_runs",settings.max_concurrent_runs),("run_timeout_seconds",settings.run_timeout_seconds),("context_message_limit",settings.context_message_limit),("max_delegation_depth",settings.max_delegation_depth)] {
        conn.execute("INSERT INTO app_settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value.to_string()]).map_err(|e| e.to_string())?;
    }
    get_settings_from(&conn)
}

#[tauri::command]
fn create_group(input: CreateGroupInput, state: State<'_, AppState>) -> AppResult<GroupState> {
    let name = input.name.trim(); let owner_name = input.owner_name.trim(); let workspace = Path::new(input.workspace_path.trim());
    if name.is_empty() || owner_name.is_empty() { return Err("群名称和群主名称不能为空。".into()); }
    if !workspace.is_dir() { return Err("工作目录不存在或不可访问。".into()); }
    let group_id = id(); let owner_id = id(); let created_at = now(); let conn = open_db(&state.db_path)?;
    conn.execute("INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES(?1,?2,?3,?4,NULL,?5)", params![group_id,name,workspace.to_string_lossy(),owner_id,created_at]).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES(?1,?2,'user',?3,'#5167f6','群主',1,?4)", params![owner_id,group_id,owner_name,created_at]).map_err(|e| e.to_string())?;
    group_state(&conn, &group_id)
}

#[tauri::command]
fn add_member(input: AddMemberInput, state: State<'_, AppState>) -> AppResult<Member> {
    if !matches!(input.kind.as_str(), "user" | "agent") || input.display_name.trim().is_empty() { return Err("成员类型或名称无效。".into()); }
    let conn = open_db(&state.db_path)?; let _ = get_group(&conn, &input.group_id)?;
    let member_id = id(); let created_at = now(); let color = input.avatar_color.unwrap_or_else(|| if input.kind == "agent" { "#17a673".into() } else { "#5167f6".into() });
    conn.execute("INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES(?1,?2,?3,?4,?5,?6,1,?7)", params![member_id,input.group_id,input.kind,input.display_name.trim(),color,input.role_description.trim(),created_at]).map_err(|e| e.to_string())?;
    if input.kind == "agent" {
        let adapter = input.adapter.unwrap_or_else(|| "mock".into());
        if !matches!(adapter.as_str(), "mock" | "codex" | "claude-code" | "opencode" | "cursor") { return Err("不支持的 Agent 适配器。".into()); }
        conn.execute("INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at) VALUES(?1,?2,?3,'unknown',?4)", params![member_id,adapter,input.executable_path.filter(|p| !p.trim().is_empty()),created_at]).map_err(|e| e.to_string())?;
    }
    conn.query_row(
        "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,m.created_at FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.id=?1", params![member_id], member_from_row
    ).map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_member(group_id: String, member_id: String, state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    let state = state.inner().clone(); let conn = open_db(&state.db_path)?; let group = get_group(&conn, &group_id)?;
    if group.owner_member_id == member_id { return Err("不能移除群主。".into()); }
    let member: Member = conn.query_row("SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,m.created_at FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.id=?1 AND m.group_id=?2", params![member_id,group_id], member_from_row).optional().map_err(|e| e.to_string())?.ok_or_else(|| "找不到成员。".to_string())?;
    if member.kind == "agent" {
        let mut stmt = conn.prepare("SELECT id FROM task_runs WHERE agent_member_id=?1 AND status='running'").map_err(|e|e.to_string())?;
        let run_ids = stmt.query_map(params![member_id], |row| row.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        for run_id in run_ids { if let Some(token) = state.cancellations.lock().map_err(|_|"取消锁不可用".to_string())?.get(&run_id) { token.store(true, Ordering::SeqCst); } }
        conn.execute("UPDATE task_runs SET status='cancelled',completed_at=?1 WHERE agent_member_id=?2 AND status IN ('queued','running')",params![now(),member_id]).map_err(|e|e.to_string())?;
    }
    conn.execute("UPDATE members SET is_active=0 WHERE id=?1",params![member_id]).map_err(|e|e.to_string())?;
    if group.admin_member_id.as_deref()==Some(member_id.as_str()) { conn.execute("UPDATE groups SET admin_member_id=NULL WHERE id=?1",params![group_id]).map_err(|e|e.to_string())?; }
    emit(&app, ChatEvent { kind:"member_removed".into(),group_id,run_id:None,message_id:None,delta:None,status:None,error:None });
    Ok(())
}

#[tauri::command]
fn set_admin(group_id: String, member_id: Option<String>, state: State<'_, AppState>) -> AppResult<GroupState> {
    let conn = open_db(&state.db_path)?; let _ = get_group(&conn, &group_id)?;
    if let Some(id) = &member_id {
        let valid = conn.query_row("SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND kind='agent' AND is_active=1",params![id,group_id],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string())?;
        if valid != 1 { return Err("管理员必须是本群的活跃 Agent。".into()); }
    }
    conn.execute("UPDATE groups SET admin_member_id=?1 WHERE id=?2",params![member_id,group_id]).map_err(|e|e.to_string())?;
    group_state(&conn,&group_id)
}

fn active_agent_ids(conn: &Connection, group_id: &str, mentions: &[String]) -> AppResult<Vec<String>> {
    let mut unique = HashSet::new(); let mut agents = Vec::new();
    for member_id in mentions {
        if !unique.insert(member_id.clone()) { continue; }
        let kind = conn.query_row("SELECT kind || ':' || is_active FROM members WHERE id=?1 AND group_id=?2",params![member_id,group_id],|r|r.get::<_,String>(0)).optional().map_err(|e|e.to_string())?;
        if kind.as_deref()==Some("agent:1") { agents.push(member_id.clone()); }
    }
    Ok(agents)
}
fn create_task_run(conn: &Connection, group_id: &str, root_message_id: &str, agent_member_id: &str, parent_run_id: Option<&str>, depth: i64) -> AppResult<String> {
    let run_id=id(); conn.execute("INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,created_at) VALUES(?1,?2,?3,?4,?5,?6,'queued',?7)",params![run_id,group_id,root_message_id,agent_member_id,parent_run_id,depth,now()]).map_err(|e|e.to_string())?; insert_run_event(conn,&run_id,"queued","{}")?; Ok(run_id)
}

#[tauri::command]
async fn send_message(group_id: String, sender_member_id: String, content: String, mention_member_ids: Vec<String>, state: State<'_, AppState>, app: AppHandle) -> AppResult<SendResult> {
    let content = content.trim().to_string(); if content.is_empty() { return Err("消息不能为空。".into()); }
    let state = state.inner().clone(); let conn = open_db(&state.db_path)?; let group = get_group(&conn,&group_id)?;
    let sender_count = conn.query_row("SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND is_active=1",params![sender_member_id,group_id],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string())?;
    if sender_count != 1 { return Err("发送者不属于该群或已被移除。".into()); }
    let message = Message { id:id(),group_id:group_id.clone(),sender_member_id:sender_member_id.clone(),parent_run_id:None,content,status:"completed".into(),created_at:now() };
    conn.execute("INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,NULL,?4,?5,?6)",params![message.id,message.group_id,message.sender_member_id,message.content,message.status,message.created_at]).map_err(|e|e.to_string())?;
    for mentioned in &mention_member_ids { let _=conn.execute("INSERT OR IGNORE INTO mentions(message_id,member_id) SELECT ?1,id FROM members WHERE id=?2 AND group_id=?3",params![message.id,mentioned,group_id]); }
    let mut target_agents = active_agent_ids(&conn,&group_id,&mention_member_ids)?;
    if target_agents.is_empty() { if let Some(admin) = group.admin_member_id { target_agents.push(admin); } }
    let mut run_ids=Vec::new(); for agent_id in target_agents { run_ids.push(create_task_run(&conn,&group_id,&message.id,&agent_id,None,0)?); }
    drop(conn);
    emit(&app, ChatEvent { kind:"message_created".into(),group_id:group_id.clone(),run_id:None,message_id:Some(message.id.clone()),delta:None,status:Some("completed".into()),error:None });
    schedule_group(state, app, group_id.clone()).await;
    Ok(SendResult { message,run_ids })
}

#[tauri::command]
async fn cancel_run(run_id: String, state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    let state=state.inner().clone(); let conn=open_db(&state.db_path)?; let run:TaskRun=conn.query_row("SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,created_at,started_at,completed_at FROM task_runs WHERE id=?1",params![run_id],run_from_row).optional().map_err(|e|e.to_string())?.ok_or_else(||"找不到任务。".to_string())?;
    if !matches!(run.status.as_str(),"queued"|"running") { return Ok(()); }
    if let Some(token)=state.cancellations.lock().map_err(|_|"取消锁不可用".to_string())?.get(&run_id){token.store(true,Ordering::SeqCst);}
    conn.execute("UPDATE task_runs SET status='cancelled',completed_at=?1 WHERE id=?2",params![now(),run_id]).map_err(|e|e.to_string())?;
    if let Some(message_id)=run.output_message_id { conn.execute("UPDATE messages SET status='cancelled' WHERE id=?1",params![message_id]).map_err(|e|e.to_string())?; }
    insert_run_event(&conn,&run_id,"cancelled","{}")?;
    emit(&app,ChatEvent {kind:"run_status".into(),group_id:run.group_id,run_id:Some(run_id),message_id:run.output_message_id,delta:None,status:Some("cancelled".into()),error:None});
    Ok(())
}

#[tauri::command]
async fn retry_run(run_id: String, state: State<'_, AppState>, app: AppHandle) -> AppResult<String> {
    let state=state.inner().clone();let conn=open_db(&state.db_path)?;let old:TaskRun=conn.query_row("SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,created_at,started_at,completed_at FROM task_runs WHERE id=?1",params![run_id],run_from_row).optional().map_err(|e|e.to_string())?.ok_or_else(||"找不到任务。".to_string())?;
    let new_id=create_task_run(&conn,&old.group_id,&old.root_message_id,&old.agent_member_id,old.parent_run_id.as_deref(),old.depth)?;drop(conn);schedule_group(state,app,old.group_id).await;Ok(new_id)
}

#[tauri::command]
async fn detect_agent(member_id: String, state: State<'_, AppState>) -> AppResult<String> {
    let conn=open_db(&state.db_path)?; let record=conn.query_row("SELECT p.adapter,COALESCE(p.executable_path,''),m.group_id FROM agent_profiles p JOIN members m ON m.id=p.member_id WHERE p.member_id=?1",params![member_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).optional().map_err(|e|e.to_string())?.ok_or_else(||"找不到 Agent 配置。".to_string())?;
    if record.0=="mock" { conn.execute("UPDATE agent_profiles SET runtime_status='ready',updated_at=?1 WHERE member_id=?2",params![now(),member_id]).map_err(|e|e.to_string())?;return Ok("ready".into()); }
    let executable=if record.1.trim().is_empty(){default_executable(&record.0).ok_or_else(||"此适配器暂未提供运行器。".to_string())?.to_string()}else{record.1};
    let status=match tokio::time::timeout(Duration::from_secs(5),Command::new(executable).arg("--version").output()).await { Ok(Ok(output)) if output.status.success()=>"ready", _=>"unavailable" };
    conn.execute("UPDATE agent_profiles SET runtime_status=?1,updated_at=?2 WHERE member_id=?3",params![status,now(),member_id]).map_err(|e|e.to_string())?;Ok(status.into())
}

fn default_executable(adapter:&str)->Option<&'static str>{match adapter{"codex"=>Some("codex"),"claude-code"=>Some("claude"),"opencode"=>Some("opencode"),"cursor"=>Some("cursor-agent"),_=>None}}

async fn schedule_group(state: AppState, app: AppHandle, group_id: String) {
    let inserted = match state.scheduling_groups.lock() { Ok(mut guard) => guard.insert(group_id.clone()), Err(_) => false };
    if !inserted { return; }
    let scheduled = (|| -> AppResult<Vec<(String, Option<String>)>> {
        let conn=open_db(&state.db_path)?;let settings=get_settings_from(&conn)?;
        let running=conn.query_row("SELECT COUNT(*) FROM task_runs WHERE group_id=?1 AND status='running'",params![group_id],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string())?;
        let available=(settings.max_concurrent_runs-running).max(0);
        let mut stmt=conn.prepare("SELECT id,agent_member_id FROM task_runs WHERE group_id=?1 AND status='queued' ORDER BY created_at LIMIT ?2").map_err(|e|e.to_string())?;
        let queued=stmt.query_map(params![group_id,available],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        let mut starts=Vec::new();for(run_id,agent_id)in queued{let message_id=id();conn.execute("INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,?2,?3,?4,'','streaming',?5)",params![message_id,group_id,agent_id,run_id,now()]).map_err(|e|e.to_string())?;conn.execute("UPDATE task_runs SET status='running',output_message_id=?1,started_at=?2 WHERE id=?3 AND status='queued'",params![message_id,now(),run_id]).map_err(|e|e.to_string())?;insert_run_event(&conn,&run_id,"started","{}")?;starts.push((run_id,Some(message_id)));}Ok(starts)
    })();
    if let Ok(mut guard)=state.scheduling_groups.lock(){guard.remove(&group_id);}
    match scheduled { Ok(starts)=>for(run_id,message_id)in starts{emit(&app,ChatEvent{kind:"run_status".into(),group_id:group_id.clone(),run_id:Some(run_id.clone()),message_id,status:Some("running".into()),delta:None,error:None});let child_state=state.clone();let child_app=app.clone();tokio::spawn(async move{execute_run(child_state,child_app,run_id).await;});},Err(error)=>emit(&app,ChatEvent{kind:"scheduler_error".into(),group_id,run_id:None,message_id:None,delta:None,status:None,error:Some(error)}) }
}

#[derive(Clone)]
struct ExecutionContext { run: TaskRun, group: Group, agent: Member, prompt: String, settings: RuntimeSettings }
fn get_execution_context(state:&AppState,run_id:&str)->AppResult<ExecutionContext>{
    let conn=open_db(&state.db_path)?;let run:TaskRun=conn.query_row("SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,created_at,started_at,completed_at FROM task_runs WHERE id=?1",params![run_id],run_from_row).map_err(|e|e.to_string())?;let group=get_group(&conn,&run.group_id)?;let agent:Member=conn.query_row("SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,m.created_at FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.id=?1",params![run.agent_member_id],member_from_row).map_err(|e|e.to_string())?;if !agent.is_active{return Err("该 Agent 已被移除。".into());}let mut stmt=conn.prepare("SELECT m.id,m.group_id,m.sender_member_id,m.parent_run_id,m.content,m.status,m.created_at FROM messages m WHERE m.group_id=?1 ORDER BY m.created_at DESC LIMIT ?2").map_err(|e|e.to_string())?;let mut history=stmt.query_map(params![group.id,settings_or(&conn,"context_message_limit",40)?],message_from_row).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;history.reverse();let members=get_members(&conn,&group.id)?;let display:HashMap<_,_>=members.iter().map(|m|(m.id.clone(),m.display_name.clone())).collect();let lines=history.iter().map(|m|format!("{}: {}",display.get(&m.sender_member_id).cloned().unwrap_or_else(||"成员".into()),m.content)).collect::<Vec<_>>().join("\n");let root=history.iter().find(|m|m.id==run.root_message_id).map(|m|m.content.clone()).unwrap_or_default();let prompt=format!("你是群聊中的 Agent「{}」。职责：{}。\n工作目录：{}\n请只完成当前任务，明确说明结果与风险。需要其他 Agent 协作时，仅在你是管理员时使用 @成员名 提及。\n任务根消息：{}\n最近群聊：\n{}",agent.display_name,agent.role_description,group.workspace_path,root,lines);Ok(ExecutionContext{run,group,agent,prompt,settings:get_settings_from(&conn)?})
}
fn settings_or(conn:&Connection,key:&str,default:i64)->AppResult<i64>{conn.query_row("SELECT value FROM app_settings WHERE key=?1",params![key],|r|r.get::<_,String>(0)).optional().map_err(|e|e.to_string())?.and_then(|x|x.parse().ok()).or(Some(default)).ok_or_else(||"设置读取失败".into())}

async fn execute_run(state:AppState,app:AppHandle,run_id:String){
    let context=match get_execution_context(&state,&run_id){Ok(v)=>v,Err(error)=>{finish_failed(&state,&app,&run_id,&error).await;return;}};let token=Arc::new(AtomicBool::new(false));if let Ok(mut tokens)=state.cancellations.lock(){tokens.insert(run_id.clone(),token.clone());}
    let outcome=run_agent(&state,&app,&context,&token).await;if token.load(Ordering::SeqCst){ }else if let Err(error)=outcome{finish_failed(&state,&app,&run_id,&error).await;}else{finish_completed(&state,&app,&context).await;}
    if let Ok(mut tokens)=state.cancellations.lock(){tokens.remove(&run_id);}schedule_group(state,app,context.group.id).await;
}
async fn run_agent(state:&AppState,app:&AppHandle,context:&ExecutionContext,token:&Arc<AtomicBool>)->AppResult<()> {
    let adapter=context.agent.adapter.as_deref().unwrap_or("mock");if adapter=="mock"{for chunk in ["已收到任务。", "我会根据当前群聊上下文进行处理。", "（这是本地模拟 Agent 的流式回复。）"]{if token.load(Ordering::SeqCst){return Ok(());}append_delta(state,app,&context.run,chunk).await?;sleep(Duration::from_millis(280)).await;}return Ok(());}
    if !matches!(adapter,"codex"|"claude-code"){return Err(format!("{} 适配器已预留配置，但 v1 尚未实现真实运行器。",adapter));}
    let executable=context.agent.executable_path.as_deref().filter(|p|!p.trim().is_empty()).unwrap_or_else(||default_executable(adapter).unwrap());let mut command=Command::new(executable);command.current_dir(&context.group.workspace_path).stdout(Stdio::piped()).stderr(Stdio::piped());match adapter{"codex"=>{command.args(["exec","--json","--skip-git-repo-check",&context.prompt]);},"claude-code"=>{command.args(["-p","--output-format","stream-json","--verbose",&context.prompt]);},_=>{}};
    let mut child=command.spawn().map_err(|e|format!("无法启动 {adapter}：{e}"))?;let stdout=child.stdout.take().ok_or_else(||"无法读取 Agent 输出。".to_string())?;let mut stderr=child.stderr.take().ok_or_else(||"无法读取 Agent 诊断。".to_string())?;let stderr_task=tokio::spawn(async move{let mut result=String::new();let _=stderr.read_to_string(&mut result).await;result});let mut lines=BufReader::new(stdout).lines();let started=Instant::now();loop{tokio::select!{result=lines.next_line()=>match result{Ok(Some(line))=>{let output=parse_agent_line(&line);if !output.is_empty(){append_delta(state,app,&context.run,&output).await?;}},Ok(None)=>break,Err(error)=>return Err(format!("读取 Agent 输出失败：{error}"))},_=sleep(Duration::from_millis(200))=>{if token.load(Ordering::SeqCst){let _=child.kill().await;return Ok(());}if started.elapsed()>Duration::from_secs(context.settings.run_timeout_seconds as u64){let _=child.kill().await;return Err("Agent 任务超时，已停止。".into());}}}}
    let status=child.wait().await.map_err(|e|format!("等待 Agent 结束失败：{e}"))?;let stderr=stderr_task.await.unwrap_or_default();if !status.success(){return Err(if stderr.trim().is_empty(){format!("{adapter} 异常退出（{status}）。")}else{format!("{adapter} 异常退出：{}",stderr.trim())});}Ok(())
}
fn parse_agent_line(line:&str)->String{if let Ok(value)=serde_json::from_str::<Value>(line){return extract_text(&value).unwrap_or_default();}line.to_string()}
fn extract_text(value:&Value)->Option<String>{match value{Value::Object(map)=>{for key in ["delta","text","content"]{if let Some(Value::String(text))=map.get(key){if !text.trim().is_empty(){return Some(text.clone());}}}for value in map.values(){if let Some(text)=extract_text(value){return Some(text);}}None},Value::Array(values)=>values.iter().find_map(extract_text),_=>None}}
async fn append_delta(state:&AppState,app:&AppHandle,run:&TaskRun,delta:&str)->AppResult<()>{let conn=open_db(&state.db_path)?;let output_id=run.output_message_id.as_ref().ok_or_else(||"任务缺少输出消息。".to_string())?;let changed=conn.execute("UPDATE messages SET content=content || ?1 WHERE id=?2 AND status='streaming'",params![delta,output_id]).map_err(|e|e.to_string())?;if changed==0{return Ok(());}insert_run_event(&conn,&run.id,"delta",delta)?;emit(app,ChatEvent{kind:"message_delta".into(),group_id:run.group_id.clone(),run_id:Some(run.id.clone()),message_id:Some(output_id.clone()),delta:Some(delta.into()),status:Some("streaming".into()),error:None});Ok(())}
async fn finish_failed(state:&AppState,app:&AppHandle,run_id:&str,error:&str){let result=(||->AppResult<(String,Option<String>)>{let conn=open_db(&state.db_path)?;let run:TaskRun=conn.query_row("SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,created_at,started_at,completed_at FROM task_runs WHERE id=?1",params![run_id],run_from_row).map_err(|e|e.to_string())?;let changed=conn.execute("UPDATE task_runs SET status='failed',error_message=?1,completed_at=?2 WHERE id=?3 AND status='running'",params![error,now(),run_id]).map_err(|e|e.to_string())?;if changed>0{if let Some(message_id)=&run.output_message_id{conn.execute("UPDATE messages SET status='failed' WHERE id=?1",params![message_id]).map_err(|e|e.to_string())?;}insert_run_event(&conn,run_id,"failed",error)?;}Ok((run.group_id,run.output_message_id))})();if let Ok((group_id,message_id))=result{emit(app,ChatEvent{kind:"run_status".into(),group_id,run_id:Some(run_id.into()),message_id,delta:None,status:Some("failed".into()),error:Some(error.into())});}}
async fn finish_completed(state:&AppState,app:&AppHandle,context:&ExecutionContext){let result=(||->AppResult<Option<String>>{let conn=open_db(&state.db_path)?;let changed=conn.execute("UPDATE task_runs SET status='completed',completed_at=?1 WHERE id=?2 AND status='running'",params![now(),context.run.id]).map_err(|e|e.to_string())?;if changed==0{return Ok(None);}let output_id=context.run.output_message_id.as_ref().ok_or_else(||"任务缺少输出消息。".to_string())?;conn.execute("UPDATE messages SET content=CASE WHEN length(content)=0 THEN '已完成。' ELSE content END,status='completed' WHERE id=?1",params![output_id]).map_err(|e|e.to_string())?;insert_run_event(&conn,&context.run.id,"completed","{}")?;Ok(Some(output_id.clone()))})();if let Ok(Some(message_id))=result{emit(app,ChatEvent{kind:"run_status".into(),group_id:context.group.id.clone(),run_id:Some(context.run.id.clone()),message_id:Some(message_id.clone()),delta:None,status:Some("completed".into()),error:None});delegate_from_admin(state,app,context,&message_id).await;}}
async fn delegate_from_admin(state:&AppState,app:&AppHandle,context:&ExecutionContext,output_message_id:&str){let created=(||->AppResult<Vec<String>>{let conn=open_db(&state.db_path)?;let group=get_group(&conn,&context.group.id)?;if group.admin_member_id.as_deref()!=Some(context.agent.id.as_str())||context.run.depth>=context.settings.max_delegation_depth{return Ok(vec![]);}let content=conn.query_row("SELECT content FROM messages WHERE id=?1",params![output_message_id],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;let members=get_members(&conn,&group.id)?;let target_ids=members.iter().filter(|m|m.kind=="agent"&&m.is_active&&m.id!=context.agent.id&&content.contains(&format!("@{}",m.display_name))).map(|m|m.id.clone()).collect::<Vec<_>>();let mut runs=Vec::new();for target in target_ids{let _=conn.execute("INSERT OR IGNORE INTO mentions(message_id,member_id) VALUES(?1,?2)",params![output_message_id,target]);runs.push(create_task_run(&conn,&group.id,output_message_id,&target,Some(&context.run.id),context.run.depth+1)?);}Ok(runs)})();if let Ok(run_ids)=created{if !run_ids.is_empty(){schedule_group(state.clone(),app.clone(),context.group.id.clone()).await;}}}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir=app.path().app_data_dir().map_err(|e|std::io::Error::other(e.to_string()))?;fs::create_dir_all(&dir)?;let db_path=dir.join("linlis-work-panel.sqlite3");init_db(&db_path).map_err(std::io::Error::other)?;app.manage(AppState{db_path,cancellations:Arc::new(Mutex::new(HashMap::new())),scheduling_groups:Arc::new(Mutex::new(HashSet::new()))});Ok(())
        })
        .invoke_handler(tauri::generate_handler![bootstrap,get_group_state,get_runtime_settings,update_runtime_settings,create_group,add_member,remove_member,set_admin,send_message,cancel_run,retry_run,detect_agent])
        .run(tauri::generate_context!())
        .expect("启动 LinlisWorkPanel 失败");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_stream_text_from_nested_json() {
        let value: Value=serde_json::from_str(r#"{"type":"item","delta":{"text":"hello"}}"#).unwrap();
        assert_eq!(extract_text(&value).as_deref(),Some("hello"));
    }
    #[test]
    fn database_marks_incomplete_runs_interrupted() {
        let file=tempfile::NamedTempFile::new().unwrap();init_db(file.path()).unwrap();let conn=open_db(file.path()).unwrap();conn.execute("INSERT INTO groups VALUES('g','g','.', 'u',NULL,1)",[]).unwrap();conn.execute("INSERT INTO members VALUES('u','g','user','u','#000','',1,1)",[]).unwrap();conn.execute("INSERT INTO members VALUES('a','g','agent','a','#000','',1,1)",[]).unwrap();conn.execute("INSERT INTO agent_profiles VALUES('a','mock',NULL,'unknown',1)",[]).unwrap();conn.execute("INSERT INTO messages VALUES('m','g','u',NULL,'x','completed',1)",[]).unwrap();conn.execute("INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,depth,status,created_at) VALUES('r','g','m','a',0,'running',1)",[]).unwrap();drop(conn);init_db(file.path()).unwrap();let conn=open_db(file.path()).unwrap();let status:String=conn.query_row("SELECT status FROM task_runs WHERE id='r'",[],|r|r.get(0)).unwrap();assert_eq!(status,"interrupted");
    }
}
