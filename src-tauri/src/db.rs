use crate::models::{
    Experience, Feature, FeatureTask, Group, GroupState, JoinableUser, Member, Message, RoadmapItem,
    RuntimeSettings, TaskRun,
};
use chrono::Utc;
use rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use std::sync::OnceLock;
use uuid::Uuid;

pub type AppResult<T> = Result<T, String>;

pub fn now() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn id() -> String {
    Uuid::new_v4().to_string()
}

pub fn open_db(path: &Path) -> AppResult<Connection> {
    let _ = KEY_FILE.set(path.with_extension("key"));
    let connection = Connection::open(path).map_err(|e| e.to_string())?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
        .map_err(|e| e.to_string())?;
    Ok(connection)
}

/// 本机密钥文件：DB 同目录、<db 文件名>.key（首次启动生成 32 随机字节）。
static KEY_FILE: OnceLock<std::path::PathBuf> = OnceLock::new();
/// 进程内只计算/加载一次：避免并行测试或并发路径间互相覆写 key 文件。
static MACHINE_KEY: OnceLock<[u8; 32]> = OnceLock::new();

fn machine_key() -> [u8; 32] {
    *MACHINE_KEY.get_or_init(|| {
        let path = KEY_FILE
            .get()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("linlis.key"));
        if let Ok(data) = std::fs::read(&path) {
            if data.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&data);
                return key;
            }
        }
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        let _ = std::fs::write(&path, key);
        key
    })
}

/// 加密落库（AES-256-GCM）：`v1:<nonce_b64>:<ct_b64>`；空串原样返回。
pub fn encrypt_secret(plain: &str) -> AppResult<String> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use base64::Engine;
    let cipher = Aes256Gcm::new_from_slice(&machine_key()).map_err(|e| e.to_string())?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plain.as_bytes())
        .map_err(|e| format!("secret encrypt failed: {e}"))?;
    let enc = base64::engine::general_purpose::STANDARD;
    Ok(format!("v1:{}:{}", enc.encode(nonce_bytes), enc.encode(ct)))
}

/// 解密读取；非 `v1:` 前缀的旧明文直通（存量兼容，本机密钥文件不变则始终可解）。
pub fn decrypt_secret(stored: &str) -> AppResult<String> {
    if !stored.starts_with("v1:") {
        return Ok(stored.to_string());
    }
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use base64::Engine;
    let mut parts = stored.splitn(3, ':');
    let _tag = parts.next();
    let nonce_b64 = parts.next().unwrap_or_default();
    let ct_b64 = parts.next().unwrap_or_default();
    let enc = base64::engine::general_purpose::STANDARD;
    let nonce = enc.decode(nonce_b64).map_err(|e| format!("nonce decode: {e}"))?;
    let ct = enc.decode(ct_b64).map_err(|e| format!("ct decode: {e}"))?;
    let cipher = Aes256Gcm::new_from_slice(&machine_key()).map_err(|e| e.to_string())?;
    let pt = cipher
        .decrypt(Nonce::from_slice(&nonce), ct.as_ref())
        .map_err(|e| format!("secret decrypt failed（key 文件被换？）: {e}"))?;
    String::from_utf8(pt).map_err(|e| e.to_string())
}

pub fn init_db(path: &Path) -> AppResult<()> {
    let connection = open_db(path)?;
    // Initialize logs table
    crate::logger::init_logs_table(&connection)?;
    connection
        .execute_batch(
            "
        CREATE TABLE IF NOT EXISTS groups (
          id TEXT PRIMARY KEY, name TEXT NOT NULL, workspace_path TEXT NOT NULL,
          owner_member_id TEXT NOT NULL, admin_member_id TEXT, created_at INTEGER NOT NULL,
          announcement TEXT NOT NULL DEFAULT '',
          announcement_updated_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS members (
          id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
          kind TEXT NOT NULL CHECK(kind IN ('user','agent','chatbot')), display_name TEXT NOT NULL,
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
          output_message_id TEXT, error_message TEXT, review_status TEXT,
          reviewer_member_id TEXT, created_at INTEGER NOT NULL,
          started_at INTEGER, completed_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS run_events (
          id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES task_runs(id) ON DELETE CASCADE,
          kind TEXT NOT NULL, payload TEXT NOT NULL, created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS users (
          id TEXT PRIMARY KEY, username TEXT NOT NULL UNIQUE,
          password_hash TEXT NOT NULL, created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS message_attachments (
          id TEXT PRIMARY KEY, message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
          file_name TEXT NOT NULL, mime_type TEXT NOT NULL, file_data BLOB NOT NULL,
          file_size INTEGER NOT NULL, created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS message_feedback (
          message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
          member_id TEXT NOT NULL REFERENCES members(id) ON DELETE CASCADE,
          vote TEXT NOT NULL CHECK(vote IN ('up','down')),
          created_at INTEGER NOT NULL,
          PRIMARY KEY(message_id, member_id)
        );
        CREATE INDEX IF NOT EXISTS idx_messages_group_created ON messages(group_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_runs_group_status ON task_runs(group_id, status, created_at);
 
         CREATE TABLE IF NOT EXISTS roadmap_items (
           id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
           title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
           status TEXT NOT NULL DEFAULT 'backlog', priority TEXT NOT NULL DEFAULT 'medium',
           target_date TEXT, sort_order INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS features (
           id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
           title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
           status TEXT NOT NULL DEFAULT 'backlog', priority TEXT NOT NULL DEFAULT 'medium',
           area TEXT NOT NULL DEFAULT '',
           assignee_member_id TEXT REFERENCES members(id),
           target_roadmap_item_id TEXT REFERENCES roadmap_items(id) ON DELETE SET NULL,
           sort_order INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS feature_tasks (
           id TEXT PRIMARY KEY, feature_id TEXT NOT NULL REFERENCES features(id) ON DELETE CASCADE,
           title TEXT NOT NULL, done INTEGER NOT NULL DEFAULT 0,
           sort_order INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_roadmap_items_group ON roadmap_items(group_id, sort_order);
         CREATE INDEX IF NOT EXISTS idx_features_group ON features(group_id, status, sort_order);
         CREATE INDEX IF NOT EXISTS idx_feature_tasks_feature ON feature_tasks(feature_id, sort_order);

         CREATE TABLE IF NOT EXISTS experiences (
           id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
           source_member_id TEXT NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL,
           tags TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_exp_group ON experiences(group_id);
         CREATE INDEX IF NOT EXISTS idx_exp_tags ON experiences(tags);
        ",
        )
        .map_err(|e| e.to_string())?;
    // 版本化 schema 迁移（PRAGMA user_version）：吸收历史遗留的逐列 ALTER 补丁。
    crate::db_migrations::migrate(&connection)?;
    let _ = connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS group_invites (
          token TEXT PRIMARY KEY,
          group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
          member_id TEXT NOT NULL REFERENCES members(id) ON DELETE CASCADE,
          created_by_user_id TEXT,
          expires_at INTEGER NOT NULL,
          consumed_at INTEGER,
          created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_group_invites_member ON group_invites(member_id);
        "#,
    );
    let _ = connection.execute(
        "UPDATE users SET is_admin=1 WHERE username='root' OR id='seed-user-root'",
        [],
    );
    let _ = crate::extensions::ensure_extensions_table(&connection);
    let _ = crate::workflow::ensure_workflow_tables(&connection);
    let _ = connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS roadmap_orchestrations (
          id TEXT PRIMARY KEY,
          group_id TEXT NOT NULL,
          roadmap_item_id TEXT NOT NULL,
          status TEXT NOT NULL,
          cursor_feature_id TEXT,
          cursor_task_id TEXT,
          current_run_id TEXT,
          error_message TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_orch_group ON roadmap_orchestrations(group_id, updated_at);
        CREATE INDEX IF NOT EXISTS idx_orch_item_status ON roadmap_orchestrations(roadmap_item_id, status);
        CREATE TABLE IF NOT EXISTS chat_context_summaries (
          group_id TEXT PRIMARY KEY REFERENCES groups(id) ON DELETE CASCADE,
          summary_text TEXT NOT NULL,
          through_created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS group_read_cursors (
          user_id TEXT NOT NULL,
          group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
          last_read_at INTEGER NOT NULL,
          PRIMARY KEY (user_id, group_id)
        );
        "#,
    );
    for (key, value) in [
        ("max_concurrent_runs", "3"),
        ("run_timeout_seconds", "900"),
        ("context_message_limit", "40"),
        ("chat_context_message_limit", "12"),
        ("max_delegation_depth", "2"),
        ("heartbeat_auto", "1"),
        ("heartbeat_focus_seconds", "1"),
        ("heartbeat_background_seconds", "5"),
    ] {
        connection
            .execute(
                "INSERT OR IGNORE INTO app_settings(key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(|e| e.to_string())?;
    }
    // Default preset roles (always refresh known-good defaults for WorkPanel)
    let default_roles = serde_json::json!([
        {"name":"Codex","adapter":"codex","roleDescription":"项目开发主力（Codex CLI）","avatarColor":"#2b6cb0"},
        {"name":"OpenClaw","adapter":"openclaw","roleDescription":"产品设计、拉通对齐与运维","avatarColor":"#d69e2e"},
        {"name":"Cursor Agent","adapter":"cursor","roleDescription":"Cursor CLI（agent / cursor-agent）","avatarColor":"#38a169"}
    ]);
    connection.execute(
        "INSERT INTO app_settings(key, value) VALUES('preset_roles', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![default_roles.to_string()],
    ).map_err(|e| e.to_string())?;
    connection
        .execute(
            "UPDATE messages SET status='interrupted' WHERE status='streaming'",
            [],
        )
        .map_err(|e| e.to_string())?;
    // Smooth restart (方案 A): re-queue incomplete runs instead of leaving them dead.
    // Streaming bubbles are already interrupted above; scheduler will open a new output message.
    let ts = now();
    connection
        .execute(
            "UPDATE task_runs SET status='queued', started_at=NULL, completed_at=NULL,
             output_message_id=NULL, phase='recovering', phase_updated_at=?1,
             error_message='recovered_after_restart'
             WHERE status IN ('queued','running')",
            params![ts],
        )
        .map_err(|e| e.to_string())?;
    let _ = crate::release_drain::clear_on_startup(&connection);
    ensure_default_seed(&connection)?;
    Ok(())
}

/// Built-in admin `root`/`root` + default LinlisWorkPanel group with Codex / OpenClaw / Cursor Agent.
pub fn ensure_default_seed(connection: &Connection) -> AppResult<()> {
    const ROOT_USER_ID: &str = "seed-user-root";
    const GROUP_ID: &str = "seed-group-workpanel";
    const OWNER_MEMBER_ID: &str = "seed-member-owner-root";
    const CODEX_MEMBER_ID: &str = "seed-member-codex";
    const OPENCLAW_MEMBER_ID: &str = "seed-member-openclaw";
    const CURSOR_MEMBER_ID: &str = "seed-member-cursor";
    const WORKSPACE: &str = "/AI/LinlisWorkPanel";

    let created_at = now();
    let password_hash = crate::auth::hash_password("root")?;

    connection
        .execute(
            "INSERT INTO users(id, username, password_hash, created_at, is_admin) VALUES(?1, 'root', ?2, ?3, 1)
             ON CONFLICT(username) DO UPDATE SET password_hash=excluded.password_hash, is_admin=1",
            params![ROOT_USER_ID, password_hash, created_at],
        )
        .map_err(|e| e.to_string())?;

    let group_exists: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM groups WHERE id=?1",
            params![GROUP_ID],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if group_exists > 0 {
        let _ = connection.execute(
            "UPDATE groups SET is_system=1 WHERE id=?1",
            params![GROUP_ID],
        );
        ensure_workpanel_super_harness(connection, GROUP_ID)?;
        return Ok(());
    }

    connection
        .execute(
            "INSERT INTO groups(id, name, workspace_path, owner_member_id, admin_member_id, created_at, is_system)
             VALUES(?1, 'LinlisWorkPanel', ?2, ?3, ?4, ?5, 1)",
            params![GROUP_ID, WORKSPACE, OWNER_MEMBER_ID, CODEX_MEMBER_ID, created_at],
        )
        .map_err(|e| e.to_string())?;

    connection
        .execute(
            "INSERT INTO members(id, group_id, kind, display_name, avatar_color, role_description, is_active, created_at)
             VALUES(?1, ?2, 'user', 'root', '#5167f6', '管理员', 1, ?3)",
            params![OWNER_MEMBER_ID, GROUP_ID, created_at],
        )
        .map_err(|e| e.to_string())?;

    let agents = [
        (
            CODEX_MEMBER_ID,
            "Codex",
            "codex",
            "#2b6cb0",
            "项目开发主力（Codex CLI）",
        ),
        (
            OPENCLAW_MEMBER_ID,
            "OpenClaw",
            "openclaw",
            "#d69e2e",
            "产品设计、拉通对齐与运维",
        ),
        (
            CURSOR_MEMBER_ID,
            "Cursor Agent",
            "cursor",
            "#38a169",
            "Cursor CLI（agent / cursor-agent）",
        ),
    ];
    for (member_id, name, adapter, color, desc) in agents {
        connection
            .execute(
                "INSERT INTO members(id, group_id, kind, display_name, avatar_color, role_description, is_active, created_at)
                 VALUES(?1, ?2, 'agent', ?3, ?4, ?5, 1, ?6)",
                params![member_id, GROUP_ID, name, color, desc, created_at],
            )
            .map_err(|e| e.to_string())?;
        connection
            .execute(
                "INSERT INTO agent_profiles(member_id, adapter, executable_path, runtime_status, updated_at)
                 VALUES(?1, ?2, NULL, 'unknown', ?3)",
                params![member_id, adapter, created_at],
            )
            .map_err(|e| e.to_string())?;
    }

    // WorkPanel 组(种子/系统群)唯一完整自举执行者：linlis-super-harness（不可修改）
    ensure_workpanel_super_harness(connection, GROUP_ID)?;

    Ok(())
}

/// Platform-locked WorkPanel self-bootstrap agent (`linlis-super-harness`).
/// Lives in the WorkPanel seed/system group (`is_system=1`) and is the ONLY executor
/// holding full self-bootstrap (面板自举/自改) write capability. system_locked=1 makes
/// it read-only in UI and rejects all mutations in backend commands.
fn ensure_workpanel_super_harness(connection: &Connection, group_id: &str) -> AppResult<()> {
    const SUPER_HARNESS_MEMBER_ID: &str = "seed-member-linlis-super-harness";
    let created_at = now();
    connection
        .execute(
            "INSERT OR IGNORE INTO members(id, group_id, kind, display_name, avatar_color, role_description, is_active, created_at)
             VALUES(?1, ?2, 'agent', 'linlis-super-harness', '#7c3aed', 'WorkPanel 自举引导器（不可修改；唯一拥有面板自举/自改完整执行权）', 1, ?3)",
            params![SUPER_HARNESS_MEMBER_ID, group_id, created_at],
        )
        .map_err(|e| e.to_string())?;
    connection
        .execute(
            "INSERT INTO agent_profiles(member_id, adapter, executable_path, runtime_status, updated_at, system_locked)
             VALUES(?1, 'dsh', NULL, 'unknown', ?2, 1)
             ON CONFLICT(member_id) DO UPDATE SET adapter='dsh', system_locked=1, runtime_status='unknown'",
            params![SUPER_HARNESS_MEMBER_ID, created_at],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 普通项目群极简自举 Agent（bootstrap-dsh-<group>，system_locked=1，无面板写回权）。
/// chat 群不创建。幂等：重复调用 INSERT OR IGNORE / ON CONFLICT。
pub fn ensure_minimal_bootstrap_dsh(
    connection: &Connection,
    group_id: &str,
    group_kind: &str,
) -> AppResult<()> {
    if group_kind == "chat" {
        return Ok(());
    }
    let member_id = format!("bootstrap-dsh-{group_id}");
    let display = format!("bootstrap-dsh·{}", &group_id[..group_id.len().min(8)]);
    let ts = now();
    connection
        .execute(
            "INSERT OR IGNORE INTO members(id, group_id, kind, display_name, avatar_color, role_description, is_active, created_at)
             VALUES(?1, ?2, 'agent', ?3, '#6d28d9', '极简 DSH 自举引导器（不可修改；仅组内 dsh 执行，无面板写回权）', 1, ?4)",
            params![&member_id, group_id, display, ts],
        )
        .map_err(|e| e.to_string())?;
    connection
        .execute(
            "INSERT INTO agent_profiles(member_id, adapter, executable_path, runtime_status, updated_at, system_locked)
             VALUES(?1, 'dsh', NULL, 'unknown', ?2, 1)
             ON CONFLICT(member_id) DO UPDATE SET adapter='dsh', system_locked=1",
            params![&member_id, ts],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Reject mutations on platform-locked bootstrap agents (system_locked=1).
pub fn assert_member_mutable(connection: &Connection, member_id: &str) -> AppResult<()> {
    let locked: i64 = connection
        .query_row(
            "SELECT COALESCE(p.system_locked,0) FROM agent_profiles p WHERE p.member_id=?1",
            params![member_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if locked != 0 {
        return Err("平台锁定的自举 Agent（bootstrap-dsh / linlis-super-harness）不可修改或移除。".into());
    }
    Ok(())
}



pub const GROUP_SELECT: &str = "SELECT id,name,workspace_path,owner_member_id,admin_member_id,created_at,COALESCE(announcement,''),announcement_updated_at,COALESCE(group_kind,'project'),COALESCE(archived,0),COALESCE(is_system,0) FROM groups";

pub fn group_from_row(row: &Row<'_>) -> rusqlite::Result<Group> {
    Ok(Group {
        id: row.get(0)?,
        name: row.get(1)?,
        workspace_path: row.get(2)?,
        owner_member_id: row.get(3)?,
        admin_member_id: row.get(4)?,
        created_at: row.get(5)?,
        announcement: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
        announcement_updated_at: row.get(7)?,
        group_kind: row
            .get::<_, Option<String>>(8)?
            .unwrap_or_else(|| "project".into()),
        archived: row.get::<_, i64>(9).unwrap_or(0) != 0,
        is_system: row.get::<_, i64>(10).unwrap_or(0) != 0,
        unread_count: 0,
    })
}

/// Reject deleting built-in system groups (guard for future delete APIs).
pub fn assert_group_deletable(connection: &Connection, group_id: &str) -> AppResult<()> {
    let is_system: i64 = connection
        .query_row(
            "SELECT COALESCE(is_system,0) FROM groups WHERE id=?1",
            params![group_id],
            |r| r.get(0),
        )
        .map_err(|_| format!("群不存在：{group_id}"))?;
    if is_system != 0 {
        return Err("系统种子群不可删除".into());
    }
    Ok(())
}

pub fn member_from_row(row: &Row<'_>) -> rusqlite::Result<Member> {
    let api_key: Option<String> = row.get(13)?;
    let kind: String = row.get(2)?;
    let is_active = row.get::<_, i64>(6)? != 0;
    let auth_user_id: Option<String> = row.get(17).ok().flatten();
    let invite_pending = kind == "user" && is_active && auth_user_id.is_none();
    Ok(Member {
        id: row.get(0)?,
        group_id: row.get(1)?,
        kind,
        display_name: row.get(3)?,
        avatar_color: row.get(4)?,
        role_description: row.get(5)?,
        is_active,
        adapter: row.get(7)?,
        executable_path: row.get(8)?,
        runtime_status: row.get(9)?,
        tags: row.get(10)?,
        created_at: row.get(11)?,
        workspace_path: row.get(12)?,
        api_key_set: api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false),
        keep_alive: row.get::<_, i64>(14)? != 0,
        warm_status: row.get(15)?,
        model: row.get(16).ok().flatten(),
        auth_user_id,
        invite_pending,
          system_locked: row.get::<_, i64>(18).ok().unwrap_or(0) != 0,
    })
}

pub const MEMBER_SELECT: &str = "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,COALESCE(m.tags,''),m.created_at,p.workspace_path,p.api_key,COALESCE(p.keep_alive,0),p.warm_status,p.model,m.auth_user_id,COALESCE(p.system_locked,0)
         FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id";

pub fn message_from_row(row: &Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        group_id: row.get(1)?,
        sender_member_id: row.get(2)?,
        parent_run_id: row.get(3)?,
        content: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        has_thinking: false,
        has_artifact: false,
    })
}

/// Strip thinking/artifact bodies for chat list payloads; keep flags for lazy UI fetch.
pub fn project_message_for_client(mut message: Message) -> Message {
    let projected = crate::message_content::project_content_for_list(&message.content);
    message.content = projected.content;
    message.has_thinking = projected.has_thinking;
    message.has_artifact = projected.has_artifact;
    message
}

pub fn project_messages_for_client(messages: Vec<Message>) -> Vec<Message> {
    messages.into_iter().map(project_message_for_client).collect()
}

pub fn get_message_channel_text(
    connection: &Connection,
    group_id: &str,
    message_id: &str,
    channel: &str,
) -> AppResult<String> {
    let content: String = connection
        .query_row(
            "SELECT content FROM messages WHERE id=?1 AND group_id=?2",
            params![message_id, group_id],
            |row| row.get(0),
        )
        .map_err(|_| "消息不存在。".to_string())?;
    Ok(crate::message_content::extract_channel_text(&content, channel))
}

pub fn run_from_row(row: &Row<'_>) -> rusqlite::Result<TaskRun> {
    Ok(TaskRun {
        id: row.get(0)?,
        group_id: row.get(1)?,
        root_message_id: row.get(2)?,
        agent_member_id: row.get(3)?,
        parent_run_id: row.get(4)?,
        depth: row.get(5)?,
        status: row.get(6)?,
        output_message_id: row.get(7)?,
        error_message: row.get(8)?,
        review_status: row.get(9)?,
        reviewer_member_id: row.get(10)?,
        created_at: row.get(11)?,
        started_at: row.get(12)?,
        completed_at: row.get(13)?,
        phase: row.get(14).ok().flatten(),
        phase_updated_at: row.get(15).ok().flatten(),
    })
}

pub const RUN_SELECT: &str = "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at FROM task_runs";

pub fn get_groups(connection: &Connection) -> AppResult<Vec<Group>> {
    let mut stmt = connection
        .prepare(&format!("{GROUP_SELECT} ORDER BY archived ASC, created_at DESC"))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], group_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn is_admin_user(connection: &Connection, user_id: &str) -> AppResult<bool> {
    let flag: i64 = connection
        .query_row(
            "SELECT COALESCE(is_admin,0) FROM users WHERE id=?1",
            params![user_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    Ok(flag != 0 || user_id == "seed-user-root")
}

/// Users not already an active `kind=user` member of `group_id`.
pub fn list_joinable_users(connection: &Connection, group_id: &str) -> AppResult<Vec<JoinableUser>> {
    let mut stmt = connection
        .prepare(
            "SELECT id, username FROM users
             WHERE id NOT IN (
               SELECT auth_user_id FROM members
               WHERE group_id=?1 AND kind='user' AND is_active=1 AND auth_user_id IS NOT NULL
             )
             ORDER BY username COLLATE NOCASE ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], |r| {
            Ok(JoinableUser {
                id: r.get(0)?,
                username: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Resolve `auth_user_id` when adding a kind=user member: link existing or create login.
pub fn resolve_user_member_auth_id(
    connection: &Connection,
    group_id: &str,
    existing_auth_user_id: Option<&str>,
    login_username: Option<&str>,
    login_password: Option<&str>,
) -> AppResult<String> {
    if let Some(uid) = existing_auth_user_id.map(str::trim).filter(|s| !s.is_empty()) {
        let username: String = connection
            .query_row(
                "SELECT username FROM users WHERE id=?1",
                params![uid],
                |r| r.get(0),
            )
            .map_err(|_| "所选登录用户不存在".to_string())?;
        let n: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM members WHERE group_id=?1 AND auth_user_id=?2 AND is_active=1 AND kind='user' AND COALESCE(roster_hidden,0)=0",
                params![group_id, uid],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if n > 0 {
            return Err(format!("用户 {username} 已是本群成员"));
        }
        return Ok(uid.to_string());
    }

    let login_user = login_username
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "添加用户需填写登录用户名".to_string())?;
    let login_pass = login_password
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "添加用户需填写登录密码".to_string())?;
    if login_user.eq_ignore_ascii_case("root") {
        return Err("不能使用保留用户名 root".into());
    }
    if connection
        .query_row(
            "SELECT id FROM users WHERE username=?1",
            params![login_user],
            |_| Ok(()),
        )
        .is_ok()
    {
        return Err("登录用户名已被占用".into());
    }
    let uid = id();
    let password_hash = crate::auth::hash_password(login_pass)?;
    let created_at = now();
    connection
        .execute(
            "INSERT INTO users(id,username,password_hash,created_at,is_admin) VALUES(?1,?2,?3,?4,0)",
            params![uid, login_user, password_hash, created_at],
        )
        .map_err(|e| e.to_string())?;
    Ok(uid)
}

/// Groups visible to a login user. Admins see all; others only groups linked via members.auth_user_id.
pub fn get_groups_for_user(connection: &Connection, user_id: &str) -> AppResult<Vec<Group>> {
    let mut groups = if is_admin_user(connection, user_id)? {
        get_groups(connection)?
    } else {
        let mut stmt = connection
            .prepare(&format!(
                "{GROUP_SELECT} WHERE id IN (
               SELECT DISTINCT group_id FROM members
               WHERE auth_user_id=?1 AND is_active=1 AND kind='user' AND COALESCE(roster_hidden,0)=0
             ) ORDER BY archived ASC, created_at DESC"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![user_id], group_from_row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows
    };
    for g in &mut groups {
        g.unread_count = count_group_unread(connection, user_id, &g.id)?;
    }
    // Unread first among non-archived; keep archived after (archived ASC already).
    groups.sort_by(|a, b| {
        match (a.archived, b.archived) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            _ => {
                let au = if a.unread_count > 0 { 1 } else { 0 };
                let bu = if b.unread_count > 0 { 1 } else { 0 };
                bu.cmp(&au)
                    .then_with(|| b.created_at.cmp(&a.created_at))
            }
        }
    });
    Ok(groups)
}

fn read_cursor_baseline(
    connection: &Connection,
    user_id: &str,
    group_id: &str,
) -> AppResult<i64> {
    let cursor: Option<i64> = connection
        .query_row(
            "SELECT last_read_at FROM group_read_cursors WHERE user_id=?1 AND group_id=?2",
            params![user_id, group_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(ts) = cursor {
        return Ok(ts);
    }
    // Never opened: baseline = membership join time (not entire history).
    let joined: Option<i64> = connection
        .query_row(
            "SELECT created_at FROM members WHERE group_id=?1 AND auth_user_id=?2 AND kind='user' AND is_active=1 AND COALESCE(roster_hidden,0)=0 ORDER BY created_at ASC LIMIT 1",
            params![group_id, user_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(joined.unwrap_or(0))
}

/// Messages after baseline, excluding the viewer's own user-member sends.
pub fn count_group_unread(
    connection: &Connection,
    user_id: &str,
    group_id: &str,
) -> AppResult<i64> {
    let baseline = read_cursor_baseline(connection, user_id, group_id)?;
    let n: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM messages m
             WHERE m.group_id=?1 AND m.created_at > ?2
               AND m.status != 'streaming'
               AND m.sender_member_id NOT IN (
                 SELECT id FROM members
                 WHERE group_id=?1 AND auth_user_id=?3 AND kind='user' AND is_active=1
               )",
            params![group_id, baseline, user_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(n)
}

pub fn mark_group_read(
    connection: &Connection,
    user_id: &str,
    group_id: &str,
) -> AppResult<()> {
    let ts = now();
    connection
        .execute(
            "INSERT INTO group_read_cursors(user_id, group_id, last_read_at)
             VALUES(?1,?2,?3)
             ON CONFLICT(user_id, group_id) DO UPDATE SET last_read_at=excluded.last_read_at",
            params![user_id, group_id, ts],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn user_can_access_group(connection: &Connection, user_id: &str, group_id: &str) -> AppResult<bool> {
    if is_admin_user(connection, user_id)? {
        return Ok(true);
    }
    let n: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM members WHERE group_id=?1 AND auth_user_id=?2 AND is_active=1 AND kind='user' AND COALESCE(roster_hidden,0)=0",
            params![group_id, user_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

pub fn require_group_access(connection: &Connection, user_id: &str, group_id: &str) -> AppResult<()> {
    if user_can_access_group(connection, user_id, group_id)? {
        Ok(())
    } else {
        Err("无权访问该群组。".into())
    }
}

pub fn require_admin(connection: &Connection, user_id: &str) -> AppResult<()> {
    if is_admin_user(connection, user_id)? {
        Ok(())
    } else {
        Err("需要管理员权限。".into())
    }
}

pub fn get_group(connection: &Connection, group_id: &str) -> AppResult<Group> {
    connection
        .query_row(
            &format!("{GROUP_SELECT} WHERE id=?1"),
            params![group_id],
            group_from_row,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "找不到群聊。".to_string())
}

pub fn set_group_archived(connection: &Connection, group_id: &str, archived: bool) -> AppResult<Group> {
    let n = connection
        .execute(
            "UPDATE groups SET archived=?1 WHERE id=?2",
            params![if archived { 1 } else { 0 }, group_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("找不到群聊。".into());
    }
    get_group(connection, group_id)
}

pub fn set_member_model(connection: &Connection, member_id: &str, model: Option<&str>) -> AppResult<()> {
    let value = model.map(str::trim).filter(|s| !s.is_empty());
    let n = connection
        .execute(
            "UPDATE agent_profiles SET model=?1, updated_at=?2 WHERE member_id=?3",
            params![value, now(), member_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("找不到 Agent 配置。".into());
    }
    Ok(())
}

/// Set the stored API key for an agent member profile (never returned to UI as raw value).
pub fn set_member_api_key(
    connection: &Connection,
    member_id: &str,
    key: Option<&str>,
) -> AppResult<()> {
    let trimmed = key.map(str::trim).filter(|s| !s.is_empty());
    let stored = match trimmed {
        Some(k) => Some(encrypt_secret(k)?),
        None => None,
    };
    let n = connection
        .execute(
            "UPDATE agent_profiles SET api_key=?1, updated_at=?2 WHERE member_id=?3",
            params![stored, now(), member_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("找不到 Agent 配置。".into());
    }
    Ok(())
}

/// Set the executable path override for an agent member profile.
pub fn set_member_executable(
    connection: &Connection,
    member_id: &str,
    executable: Option<&str>,
) -> AppResult<()> {
    let n = connection
        .execute(
            "UPDATE agent_profiles SET executable_path=?1, updated_at=?2 WHERE member_id=?3",
            params![
                executable.map(str::trim).filter(|s| !s.is_empty()),
                now(),
                member_id
            ],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("找不到 Agent 配置。".into());
    }
    Ok(())
}

/// All agent member profiles (id, group, adapter, display name, model, executable, key set, locked).
/// Used by the agent-config import to provision existing members without creating new ones.
pub fn list_agent_profiles(connection: &Connection) -> AppResult<Vec<crate::agent_config::AgentProfileRow>> {
    let mut stmt = connection
        .prepare(
            "SELECT m.id, m.group_id, m.display_name, p.adapter, p.model, p.executable_path,
                    (p.api_key IS NOT NULL AND p.api_key <> '') AS key_set,
                    COALESCE(p.system_locked,0) AS locked
             FROM members m
             JOIN agent_profiles p ON p.member_id = m.id
             WHERE m.kind='agent' AND m.is_active=1
             ORDER BY m.group_id, m.created_at",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(crate::agent_config::AgentProfileRow {
                member_id: row.get(0)?,
                group_id: row.get(1)?,
                display_name: row.get(2)?,
                adapter: row.get(3)?,
                model: row.get(4)?,
                executable_path: row.get(5)?,
                api_key_set: row.get::<_, i64>(6)? != 0,
                system_locked: row.get::<_, i64>(7)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Generic app_settings string read (None when absent).
pub fn get_setting_str(connection: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(connection
        .query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            params![key],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?)
}

/// Generic app_settings upsert.
pub fn set_setting_str(connection: &Connection, key: &str, value: &str) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_group_announcement(
    connection: &Connection,
    group_id: &str,
    announcement: &str,
) -> AppResult<Group> {
    let ts = now();
    let n = connection
        .execute(
            "UPDATE groups SET announcement=?1, announcement_updated_at=?2 WHERE id=?3",
            params![announcement, ts, group_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("找不到群聊。".into());
    }
    get_group(connection, group_id)
}

pub fn update_group_workspace(
    connection: &Connection,
    group_id: &str,
    workspace_path: &str,
) -> AppResult<Group> {
    let n = connection
        .execute(
            "UPDATE groups SET workspace_path=?1 WHERE id=?2",
            params![workspace_path, group_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("找不到群聊。".into());
    }
    get_group(connection, group_id)
}

pub fn list_run_phases(conn: &Connection, run_id: &str) -> AppResult<Vec<crate::models::RunPhaseEntry>> {
    // 阶段时间线从 run_events 的 kind='phase' 事件投影（单一事件源），payload 形如
    // {"phase": "...", "elapsedMs": n, "totalMs": n}。
    let mut stmt = conn
        .prepare("SELECT payload,created_at FROM run_events WHERE run_id=?1 AND kind='phase' ORDER BY COALESCE(seq,0), created_at, id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![run_id], |row| {
            let payload: String = row.get(0)?;
            let phase = serde_json::from_str::<serde_json::Value>(&payload)
                .ok()
                .and_then(|v| v.get("phase").and_then(|p| p.as_str().map(|s| s.to_string())))
                .unwrap_or_else(|| "?".to_string());
            Ok(crate::models::RunPhaseEntry { phase, note: String::new(), created_at: row.get(1)? })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn set_run_phase(connection: &Connection, run_id: &str, phase: &str) -> AppResult<(i64, i64)> {
    let ts = now();
    let created: i64 = connection
        .query_row(
            "SELECT created_at FROM task_runs WHERE id=?1",
            params![run_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let prev: Option<i64> = connection
        .query_row(
            "SELECT phase_updated_at FROM task_runs WHERE id=?1",
            params![run_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();
    connection
        .execute(
            "UPDATE task_runs SET phase=?1, phase_updated_at=?2 WHERE id=?3",
            params![phase, ts, run_id],
        )
        .map_err(|e| e.to_string())?;
    let elapsed = ts - prev.unwrap_or(created);
    let total = ts - created;
    let payload = serde_json::json!({"phase": phase, "elapsedMs": elapsed, "totalMs": total});
    insert_run_event(connection, run_id, "phase", &payload.to_string())?;
    Ok((elapsed, total))
}

pub fn get_agent_api_key(connection: &Connection, member_id: &str) -> AppResult<Option<String>> {
    let raw: Option<String> = connection
        .query_row(
            "SELECT api_key FROM agent_profiles WHERE member_id=?1",
            params![member_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();
    match raw {
        Some(v) if !v.is_empty() => Ok(Some(decrypt_secret(&v)?)),
        _ => Ok(None),
    }
}

pub fn set_member_workspace(
    connection: &Connection,
    member_id: &str,
    workspace_path: &str,
) -> AppResult<()> {
    let n = connection
        .execute(
            "UPDATE agent_profiles SET workspace_path=?1, updated_at=?2 WHERE member_id=?3",
            params![workspace_path, now(), member_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("找不到 Agent 配置。".into());
    }
    Ok(())
}

pub fn get_members(connection: &Connection, group_id: &str) -> AppResult<Vec<Member>> {
    let mut stmt = connection
        .prepare(&format!(
            "{MEMBER_SELECT} WHERE m.group_id=?1 AND COALESCE(m.roster_hidden,0)=0 ORDER BY m.created_at"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], member_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get_member(connection: &Connection, member_id: &str) -> AppResult<Member> {
    connection
        .query_row(
            &format!("{MEMBER_SELECT} WHERE m.id=?1"),
            params![member_id],
            member_from_row,
        )
        .map_err(|_| format!("找不到成员：{member_id}"))
}

/// Invite link TTL: 24 hours.
pub const INVITE_TTL_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone)]
pub struct GroupInvite {
    pub token: String,
    pub group_id: String,
    pub member_id: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    pub group_name: String,
    pub display_name: String,
    pub expires_at: i64,
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub fn create_group_invite(
    connection: &Connection,
    group_id: &str,
    member_id: &str,
    created_by_user_id: Option<&str>,
) -> AppResult<GroupInvite> {
    let token = id();
    let created_at = now();
    let expires_at = created_at + INVITE_TTL_MS;
    connection
        .execute(
            "INSERT INTO group_invites(token,group_id,member_id,created_by_user_id,expires_at,consumed_at,created_at) VALUES(?1,?2,?3,?4,?5,NULL,?6)",
            params![
                token,
                group_id,
                member_id,
                created_by_user_id,
                expires_at,
                created_at
            ],
        )
        .map_err(|e| e.to_string())?;
    Ok(GroupInvite {
        token,
        group_id: group_id.into(),
        member_id: member_id.into(),
        expires_at,
    })
}

pub fn preview_invite(connection: &Connection, token: &str) -> AppResult<InvitePreview> {
    let row = connection
        .query_row(
            "SELECT i.group_id, i.member_id, i.expires_at, i.consumed_at, g.name, m.display_name, m.is_active, COALESCE(m.roster_hidden,0), m.auth_user_id
             FROM group_invites i
             JOIN groups g ON g.id=i.group_id
             JOIN members m ON m.id=i.member_id
             WHERE i.token=?1",
            params![token],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, Option<String>>(8)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((_gid, _mid, expires_at, consumed_at, group_name, display_name, is_active, hidden, auth_uid)) =
        row
    else {
        return Ok(InvitePreview {
            group_name: String::new(),
            display_name: String::new(),
            expires_at: 0,
            valid: false,
            reason: Some("邀请不存在或已失效".into()),
        });
    };
    let mut reason = None;
    let mut valid = true;
    if consumed_at.is_some() || auth_uid.is_some() {
        valid = false;
        reason = Some("邀请已被使用".into());
    } else if now() > expires_at {
        valid = false;
        reason = Some("邀请已过期".into());
    } else if is_active == 0 || hidden != 0 {
        valid = false;
        reason = Some("邀请成员已移除".into());
    }
    Ok(InvitePreview {
        group_name,
        display_name,
        expires_at,
        valid,
        reason,
    })
}

pub fn accept_invite(connection: &Connection, token: &str, user_id: &str) -> AppResult<Member> {
    let preview = preview_invite(connection, token)?;
    if !preview.valid {
        return Err(preview.reason.unwrap_or_else(|| "邀请无效".into()));
    }
    let (group_id, member_id, expires_at): (String, String, i64) = connection
        .query_row(
            "SELECT group_id, member_id, expires_at FROM group_invites WHERE token=?1",
            params![token],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| e.to_string())?;
    if now() > expires_at {
        return Err("邀请已过期".into());
    }
    let already: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM members WHERE group_id=?1 AND auth_user_id=?2 AND kind='user' AND is_active=1 AND COALESCE(roster_hidden,0)=0",
            params![group_id, user_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if already > 0 {
        return Err("你已是本群成员".into());
    }
    let user_exists: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM users WHERE id=?1",
            params![user_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if user_exists == 0 {
        return Err("用户不存在".into());
    }
    let ts = now();
    connection
        .execute(
            "UPDATE members SET auth_user_id=?1 WHERE id=?2 AND group_id=?3 AND kind='user' AND auth_user_id IS NULL AND is_active=1 AND COALESCE(roster_hidden,0)=0",
            params![user_id, member_id, group_id],
        )
        .map_err(|e| e.to_string())?;
    let changed = connection.changes();
    if changed == 0 {
        return Err("邀请已被使用或成员不可用".into());
    }
    connection
        .execute(
            "UPDATE group_invites SET consumed_at=?1 WHERE token=?2 AND consumed_at IS NULL",
            params![ts, token],
        )
        .map_err(|e| e.to_string())?;
    get_member(connection, &member_id)
}

/// Permanently remove a member from the group roster.
/// Real DELETE when no message/run refs; otherwise `roster_hidden=1` to preserve history FKs.
pub fn hard_delete_member(connection: &Connection, group_id: &str, member_id: &str) -> AppResult<()> {
    let group = get_group(connection, group_id)?;
    if group.owner_member_id == member_id {
        return Err("不能删除群主".into());
    }
    let exists: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM members WHERE id=?1 AND group_id=?2 AND COALESCE(roster_hidden,0)=0",
            params![member_id, group_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err("找不到成员".into());
    }
    let _ = connection.execute("DELETE FROM group_invites WHERE member_id=?1", params![member_id]);
    if group.admin_member_id.as_deref() == Some(member_id) {
        connection
            .execute(
                "UPDATE groups SET admin_member_id=NULL WHERE id=?1",
                params![group_id],
            )
            .map_err(|e| e.to_string())?;
    }
    let msg_refs: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE sender_member_id=?1",
            params![member_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let run_refs: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM task_runs WHERE agent_member_id=?1 OR reviewer_member_id=?1",
            params![member_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let _ = connection.execute("DELETE FROM mentions WHERE member_id=?1", params![member_id]);
    if msg_refs == 0 && run_refs == 0 {
        connection
            .execute(
                "DELETE FROM members WHERE id=?1 AND group_id=?2",
                params![member_id, group_id],
            )
            .map_err(|e| e.to_string())?;
    } else {
        connection
            .execute(
                "UPDATE members SET is_active=0, roster_hidden=1, auth_user_id=NULL WHERE id=?1 AND group_id=?2",
                params![member_id, group_id],
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Default hot window for group_state (older rows stay in DB, loaded via before-cursor).
pub const HOT_MESSAGE_LIMIT: i64 = 100;
pub const HOT_RUN_LIMIT: i64 = 100;

pub fn count_messages(connection: &Connection, group_id: &str) -> AppResult<i64> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE group_id=?1",
            params![group_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
}

/// Newest `limit` messages, returned in ascending created_at order.
pub fn get_messages_recent(
    connection: &Connection,
    group_id: &str,
    limit: i64,
) -> AppResult<Vec<Message>> {
    let limit = limit.clamp(1, 5_000);
    let mut stmt = connection
        .prepare(
            "SELECT id,group_id,sender_member_id,parent_run_id,content,status,created_at FROM (
               SELECT id,group_id,sender_member_id,parent_run_id,content,status,created_at
               FROM messages WHERE group_id=?1
               ORDER BY created_at DESC, id DESC LIMIT ?2
             ) ORDER BY created_at ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id, limit], message_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Messages with created_at strictly greater than `after_created_at`, oldest first, capped.
pub fn get_messages_after_created_at(
    connection: &Connection,
    group_id: &str,
    after_created_at: i64,
    limit: i64,
) -> AppResult<Vec<Message>> {
    let limit = limit.clamp(1, 5_000);
    let mut stmt = connection
        .prepare(
            "SELECT id,group_id,sender_member_id,parent_run_id,content,status,created_at
             FROM messages
             WHERE group_id=?1 AND created_at > ?2
             ORDER BY created_at ASC, id ASC
             LIMIT ?3",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id, after_created_at, limit], message_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[derive(Debug, Clone)]
pub struct ChatContextSummary {
    pub group_id: String,
    pub summary_text: String,
    pub through_created_at: i64,
    pub updated_at: i64,
}

pub fn get_chat_context_summary(
    connection: &Connection,
    group_id: &str,
) -> AppResult<Option<ChatContextSummary>> {
    connection
        .query_row(
            "SELECT group_id, summary_text, through_created_at, updated_at FROM chat_context_summaries WHERE group_id=?1",
            params![group_id],
            |r| {
                Ok(ChatContextSummary {
                    group_id: r.get(0)?,
                    summary_text: r.get(1)?,
                    through_created_at: r.get(2)?,
                    updated_at: r.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())
}

pub fn upsert_chat_context_summary(
    connection: &Connection,
    group_id: &str,
    summary_text: &str,
    through_created_at: i64,
) -> AppResult<()> {
    let ts = now();
    connection
        .execute(
            "INSERT INTO chat_context_summaries(group_id, summary_text, through_created_at, updated_at)
             VALUES(?1,?2,?3,?4)
             ON CONFLICT(group_id) DO UPDATE SET
               summary_text=excluded.summary_text,
               through_created_at=excluded.through_created_at,
               updated_at=excluded.updated_at",
            params![group_id, summary_text, through_created_at, ts],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Messages strictly older than (before_created_at, before_id), ascending, up to limit.
pub fn get_messages_before(
    connection: &Connection,
    group_id: &str,
    before_created_at: i64,
    before_id: &str,
    limit: i64,
) -> AppResult<Vec<Message>> {
    let limit = limit.clamp(1, 200);
    let mut stmt = connection
        .prepare(
            "SELECT id,group_id,sender_member_id,parent_run_id,content,status,created_at FROM (
               SELECT id,group_id,sender_member_id,parent_run_id,content,status,created_at
               FROM messages
               WHERE group_id=?1
                 AND (created_at < ?2 OR (created_at = ?2 AND id < ?3))
               ORDER BY created_at DESC, id DESC
               LIMIT ?4
             ) ORDER BY created_at ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params![group_id, before_created_at, before_id, limit],
            message_from_row,
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get_runs(connection: &Connection, group_id: &str) -> AppResult<Vec<TaskRun>> {
    get_runs_recent(connection, group_id, HOT_RUN_LIMIT)
}

pub fn get_runs_recent(
    connection: &Connection,
    group_id: &str,
    limit: i64,
) -> AppResult<Vec<TaskRun>> {
    let limit = limit.clamp(1, 500);
    let mut stmt = connection
        .prepare(
            "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at FROM (
               SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at
               FROM task_runs WHERE group_id=?1
               ORDER BY created_at DESC, id DESC LIMIT ?2
             ) ORDER BY created_at ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id, limit], run_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn group_state(connection: &Connection, group_id: &str) -> AppResult<GroupState> {
    let messages = get_messages_recent(connection, group_id, HOT_MESSAGE_LIMIT)?;
    let total = count_messages(connection, group_id)?;
    Ok(GroupState {
        group: get_group(connection, group_id)?,
        members: get_members(connection, group_id)?,
        messages_has_more: total > messages.len() as i64,
        messages_total: total,
        messages: project_messages_for_client(messages),
        runs: get_runs_recent(connection, group_id, HOT_RUN_LIMIT)?,
    })
}

pub fn get_settings_from(connection: &Connection) -> AppResult<RuntimeSettings> {
    let get = |key: &str| -> AppResult<i64> {
        connection
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                params![key],
                |r| r.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())?
            .parse::<i64>()
            .map_err(|_| format!("设置 {key} 不是有效整数"))
    };
    let get_or = |key: &str, default: i64| -> i64 {
        connection
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                params![key],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    Ok(RuntimeSettings {
        max_concurrent_runs: get("max_concurrent_runs")?,
        run_timeout_seconds: get("run_timeout_seconds")?,
        context_message_limit: get("context_message_limit")?,
        chat_context_message_limit: get_or("chat_context_message_limit", 12).clamp(5, 40),
        max_delegation_depth: get("max_delegation_depth")?,
        heartbeat_auto: get_or("heartbeat_auto", 1) != 0,
        heartbeat_focus_seconds: get_or("heartbeat_focus_seconds", 1).max(1),
        heartbeat_background_seconds: get_or("heartbeat_background_seconds", 5).max(1),
    })
}

pub fn settings_or(conn: &Connection, key: &str, default: i64) -> AppResult<i64> {
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            params![key],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .and_then(|x| x.parse().ok())
        .unwrap_or(default))
}

pub fn get_cli_session_id(conn: &Connection, member_id: &str) -> AppResult<Option<String>> {
    let value: Option<Option<String>> = conn
        .query_row(
            "SELECT cli_session_id FROM agent_profiles WHERE member_id=?1",
            params![member_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(value.flatten().filter(|s| !s.trim().is_empty()))
}

pub fn set_cli_session_id(conn: &Connection, member_id: &str, session_id: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE agent_profiles SET cli_session_id=?1, updated_at=?2 WHERE member_id=?3",
        params![session_id, now(), member_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_preset_roles(connection: &Connection) -> AppResult<Vec<crate::models::PresetRole>> {
    let json: Option<String> = connection
        .query_row("SELECT value FROM app_settings WHERE key='preset_roles'", [], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    match json {
        Some(val) => serde_json::from_str(&val).map_err(|e| format!("?????????{e}")),
        None => Ok(Vec::new()),
    }
}

pub fn insert_run_event(
    connection: &Connection,
    run_id: &str,
    kind: &str,
    payload: &str,
) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO run_events(id,run_id,kind,payload,seq,created_at) VALUES(?1,?2,?3,?4,(SELECT COALESCE(MAX(seq),0)+1 FROM run_events WHERE run_id=?2),?5)",
            params![id(), run_id, kind, payload, now()],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn create_task_run(
    conn: &Connection,
    group_id: &str,
    root_message_id: &str,
    agent_member_id: &str,
    parent_run_id: Option<&str>,
    depth: i64,
) -> AppResult<String> {
    let run_id = id();
    let ts = now();
    conn.execute(
        "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,created_at,phase,phase_updated_at) VALUES(?1,?2,?3,?4,?5,?6,'queued',?7,'queued',?7)",
        params![
            run_id,
            group_id,
            root_message_id,
            agent_member_id,
            parent_run_id,
            depth,
            ts
        ],
    )
    .map_err(|e| e.to_string())?;
    insert_run_event(conn, &run_id, "queued", "{}")?;
    Ok(run_id)
}

/// 运输层（Tauri command / Web handler）需要的取消上下文。
pub struct CancelRunInfo {
    pub group_id: String,
    pub output_message_id: Option<String>,
}

/// 取消任务（仅 queued/running 生效）：标 cancelled + 流式输出消息标 cancelled。
/// 返回 None 表示任务不存在或不在可取消状态；Some 供 transport 发事件。
pub fn cancel_run(conn: &Connection, run_id: &str) -> AppResult<Option<CancelRunInfo>> {
    let run: Option<TaskRun> = conn
        .query_row(&format!("{} WHERE id=?1", RUN_SELECT), params![run_id], run_from_row)
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(run) = run else {
        return Ok(None);
    };
    if !matches!(run.status.as_str(), "queued" | "running") {
        return Ok(None);
    }
    conn.execute(
        "UPDATE task_runs SET status='cancelled',completed_at=?1 WHERE id=?2",
        params![now(), run_id],
    )
    .map_err(|e| e.to_string())?;
    if let Some(message_id) = &run.output_message_id {
        conn.execute(
            "UPDATE messages SET status='cancelled' WHERE id=?1 AND status='streaming'",
            params![message_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(Some(CancelRunInfo {
        group_id: run.group_id,
        output_message_id: run.output_message_id,
    }))
}

/// 重试任务：以原 root/agent 建新 run。返回 (new_id, group_id)；None 表示原任务不存在。
pub fn retry_run(conn: &Connection, run_id: &str) -> AppResult<Option<(String, String)>> {
    let run: Option<TaskRun> = conn
        .query_row(&format!("{} WHERE id=?1", RUN_SELECT), params![run_id], run_from_row)
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(run) = run else {
        return Ok(None);
    };
    let new_id = create_task_run(
        conn,
        &run.group_id,
        &run.root_message_id,
        &run.agent_member_id,
        run.parent_run_id.as_deref(),
        run.depth,
    )?;
    Ok(Some((new_id, run.group_id)))
}

pub fn active_agent_ids(
    conn: &Connection,
    group_id: &str,
    mentions: &[String],
) -> AppResult<Vec<String>> {
    use std::collections::HashSet;
    let mut unique = HashSet::new();
    let mut agents = Vec::new();
    for member_id in mentions {
        if !unique.insert(member_id.clone()) {
            continue;
        }
        let kind = conn
            .query_row(
                "SELECT kind || ':' || is_active FROM members WHERE id=?1 AND group_id=?2",
                params![member_id, group_id],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if matches!(kind.as_deref(), Some("agent:1") | Some("chatbot:1")) {
            agents.push(member_id.clone());
        }
    }
    Ok(agents)
}

/// Default responder (group admin) may be an active agent or chatbot.
pub fn member_is_default_responder_candidate(
    conn: &Connection,
    group_id: &str,
    member_id: &str,
) -> AppResult<bool> {
    let kind: Option<String> = conn
        .query_row(
            "SELECT kind FROM members WHERE id=?1 AND group_id=?2 AND is_active=1",
            params![member_id, group_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(matches!(kind.as_deref(), Some("agent") | Some("chatbot")))
}

/// Who should run for a new message.
/// - No @mentions → fall back to group admin **only if set** and admin is agent/chatbot.
/// - @ only users (or inactive) → no agents (admin must not speak).
/// - @ agent/chatbot → those agents only.
pub fn resolve_target_agent_ids(
    conn: &Connection,
    group_id: &str,
    admin_member_id: Option<&str>,
    mentions: &[String],
) -> AppResult<Vec<String>> {
    let mut agents = active_agent_ids(conn, group_id, mentions)?;
    if agents.is_empty() && mentions.is_empty() {
        if let Some(admin) = admin_member_id {
            if member_is_default_responder_candidate(conn, group_id, admin)? {
                agents.push(admin.to_string());
            }
        }
    }
    Ok(agents)
}
 
 // === PM: Roadmap Items ===
 
 pub fn roadmap_item_from_row(row: &Row<'_>) -> rusqlite::Result<RoadmapItem> {
     Ok(RoadmapItem {
         id: row.get(0)?,
         group_id: row.get(1)?,
         title: row.get(2)?,
         description: row.get(3)?,
         status: row.get(4)?,
         priority: row.get(5)?,
         target_date: row.get(6)?,
         sort_order: row.get(7)?,
         created_at: row.get(8)?,
     })
 }
 
 pub fn get_roadmap_items(conn: &Connection, group_id: &str) -> AppResult<Vec<RoadmapItem>> {
     let mut stmt = conn
         .prepare("SELECT id,group_id,title,description,status,priority,target_date,sort_order,created_at FROM roadmap_items WHERE group_id=?1 ORDER BY sort_order,created_at")
         .map_err(|e| e.to_string())?;
     let rows = stmt
         .query_map(params![group_id], roadmap_item_from_row)
         .map_err(|e| e.to_string())?
         .collect::<Result<Vec<_>, _>>()
         .map_err(|e| e.to_string())?;
     Ok(rows)
 }
 
 pub fn create_roadmap_item_db(conn: &Connection, input: &crate::models::CreateRoadmapItemInput) -> AppResult<RoadmapItem> {
     let item = RoadmapItem {
         id: id(),
         group_id: input.group_id.clone(),
         title: input.title.clone(),
         description: input.description.clone().unwrap_or_default(),
         status: input.status.clone().unwrap_or_else(|| "backlog".into()),
         priority: input.priority.clone().unwrap_or_else(|| "medium".into()),
         target_date: input.target_date.clone(),
         sort_order: 0,
         created_at: now(),
     };
     conn.execute(
         "INSERT INTO roadmap_items(id,group_id,title,description,status,priority,target_date,sort_order,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
         params![item.id, item.group_id, item.title, item.description, item.status, item.priority, item.target_date, item.sort_order, item.created_at],
     )
     .map_err(|e| e.to_string())?;
     Ok(item)
 }
 
 pub fn update_roadmap_item_db(conn: &Connection, id: &str, input: &crate::models::UpdateRoadmapItemInput) -> AppResult<RoadmapItem> {
     let orig: RoadmapItem = conn
         .query_row("SELECT id,group_id,title,description,status,priority,target_date,sort_order,created_at FROM roadmap_items WHERE id=?1", params![id], roadmap_item_from_row)
         .map_err(|e| format!("roadmap item not found: {e}"))?;
     let updated = RoadmapItem {
         title: input.title.clone().unwrap_or(orig.title),
         description: input.description.clone().unwrap_or(orig.description),
         status: input.status.clone().unwrap_or(orig.status),
         priority: input.priority.clone().unwrap_or(orig.priority),
         target_date: input.target_date.clone().or(orig.target_date),
         sort_order: input.sort_order.unwrap_or(orig.sort_order),
         ..orig
     };
     conn.execute(
         "UPDATE roadmap_items SET title=?1,description=?2,status=?3,priority=?4,target_date=?5,sort_order=?6 WHERE id=?7",
         params![updated.title, updated.description, updated.status, updated.priority, updated.target_date, updated.sort_order, id],
     )
     .map_err(|e| e.to_string())?;
     Ok(updated)
 }
 
 pub fn delete_roadmap_item_db(conn: &Connection, id: &str) -> AppResult<()> {
     conn.execute("DELETE FROM roadmap_items WHERE id=?1", params![id])
         .map_err(|e| e.to_string())?;
     Ok(())
 }
 
 // === PM: Features ===
 
 pub fn feature_from_row(row: &Row<'_>) -> rusqlite::Result<Feature> {
     Ok(Feature {
         id: row.get(0)?,
         group_id: row.get(1)?,
         title: row.get(2)?,
         description: row.get(3)?,
         status: row.get(4)?,
         priority: row.get(5)?,
         area: row.get(6)?,
         assignee_member_id: row.get(7)?,
         target_roadmap_item_id: row.get(8)?,
         sort_order: row.get(9)?,
         created_at: row.get(10)?,
         updated_at: row.get(11)?,
     })
 }
 
 pub fn get_features(conn: &Connection, group_id: &str) -> AppResult<Vec<Feature>> {
     let mut stmt = conn
         .prepare("SELECT id,group_id,title,description,status,priority,area,assignee_member_id,target_roadmap_item_id,sort_order,created_at,updated_at FROM features WHERE group_id=?1 ORDER BY sort_order,created_at")
         .map_err(|e| e.to_string())?;
     let rows = stmt
         .query_map(params![group_id], feature_from_row)
         .map_err(|e| e.to_string())?
         .collect::<Result<Vec<_>, _>>()
         .map_err(|e| e.to_string())?;
     Ok(rows)
 }
 
 pub fn create_feature_db(conn: &Connection, input: &crate::models::CreateFeatureInput) -> AppResult<Feature> {
     let now_ts = now();
     let feature = Feature {
         id: id(),
         group_id: input.group_id.clone(),
         title: input.title.clone(),
         description: input.description.clone().unwrap_or_default(),
         status: input.status.clone().unwrap_or_else(|| "backlog".into()),
         priority: input.priority.clone().unwrap_or_else(|| "medium".into()),
         area: input.area.clone().unwrap_or_default(),
         assignee_member_id: input.assignee_member_id.clone(),
         target_roadmap_item_id: input.target_roadmap_item_id.clone(),
         sort_order: 0,
         created_at: now_ts,
         updated_at: now_ts,
     };
     conn.execute(
         "INSERT INTO features(id,group_id,title,description,status,priority,area,assignee_member_id,target_roadmap_item_id,sort_order,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
         params![feature.id, feature.group_id, feature.title, feature.description, feature.status, feature.priority, feature.area, feature.assignee_member_id, feature.target_roadmap_item_id, feature.sort_order, feature.created_at, feature.updated_at],
     )
     .map_err(|e| e.to_string())?;
     Ok(feature)
 }
 
 pub fn update_feature_db(conn: &Connection, id: &str, input: &crate::models::UpdateFeatureInput) -> AppResult<Feature> {
     let orig: Feature = conn
         .query_row("SELECT id,group_id,title,description,status,priority,area,assignee_member_id,target_roadmap_item_id,sort_order,created_at,updated_at FROM features WHERE id=?1", params![id], feature_from_row)
         .map_err(|e| format!("feature not found: {e}"))?;
     let updated = Feature {
         title: input.title.clone().unwrap_or(orig.title),
         description: input.description.clone().unwrap_or(orig.description),
         status: input.status.clone().unwrap_or(orig.status),
         priority: input.priority.clone().unwrap_or(orig.priority),
         area: input.area.clone().unwrap_or(orig.area),
         assignee_member_id: input.assignee_member_id.clone().or(orig.assignee_member_id),
         target_roadmap_item_id: input.target_roadmap_item_id.clone().or(orig.target_roadmap_item_id),
         sort_order: input.sort_order.unwrap_or(orig.sort_order),
         updated_at: now(),
         ..orig
     };
     conn.execute(
         "UPDATE features SET title=?1,description=?2,status=?3,priority=?4,area=?5,assignee_member_id=?6,target_roadmap_item_id=?7,sort_order=?8,updated_at=?9 WHERE id=?10",
         params![updated.title, updated.description, updated.status, updated.priority, updated.area, updated.assignee_member_id, updated.target_roadmap_item_id, updated.sort_order, updated.updated_at, id],
     )
     .map_err(|e| e.to_string())?;
     Ok(updated)
 }
 
 pub fn delete_feature_db(conn: &Connection, id: &str) -> AppResult<()> {
     conn.execute("DELETE FROM features WHERE id=?1", params![id])
         .map_err(|e| e.to_string())?;
     Ok(())
 }
 
 // === PM: Feature Tasks ===
 
 pub fn feature_task_from_row(row: &Row<'_>) -> rusqlite::Result<FeatureTask> {
     Ok(FeatureTask {
         id: row.get(0)?,
         feature_id: row.get(1)?,
         title: row.get(2)?,
         done: row.get::<_, i64>(3)? != 0,
         sort_order: row.get(4)?,
         created_at: row.get(5)?,
     })
 }
 
 pub fn get_feature_tasks(conn: &Connection, feature_id: &str) -> AppResult<Vec<FeatureTask>> {
     let mut stmt = conn
         .prepare("SELECT id,feature_id,title,done,sort_order,created_at FROM feature_tasks WHERE feature_id=?1 ORDER BY sort_order,created_at")
         .map_err(|e| e.to_string())?;
     let rows = stmt
         .query_map(params![feature_id], feature_task_from_row)
         .map_err(|e| e.to_string())?
         .collect::<Result<Vec<_>, _>>()
         .map_err(|e| e.to_string())?;
     Ok(rows)
 }
 
 pub fn create_feature_task_db(conn: &Connection, input: &crate::models::CreateFeatureTaskInput) -> AppResult<FeatureTask> {
     let task = FeatureTask {
         id: id(),
         feature_id: input.feature_id.clone(),
         title: input.title.clone(),
         done: false,
         sort_order: 0,
         created_at: now(),
     };
     conn.execute(
         "INSERT INTO feature_tasks(id,feature_id,title,done,sort_order,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
         params![task.id, task.feature_id, task.title, 0i64, task.sort_order, task.created_at],
     )
     .map_err(|e| e.to_string())?;
     Ok(task)
 }
 
 pub fn update_feature_task_db(conn: &Connection, id: &str, input: &crate::models::UpdateFeatureTaskInput) -> AppResult<FeatureTask> {
     let orig: FeatureTask = conn
         .query_row("SELECT id,feature_id,title,done,sort_order,created_at FROM feature_tasks WHERE id=?1", params![id], feature_task_from_row)
         .map_err(|e| format!("task not found: {e}"))?;
     let updated = FeatureTask {
         title: input.title.clone().unwrap_or(orig.title),
         done: input.done.unwrap_or(orig.done),
         sort_order: input.sort_order.unwrap_or(orig.sort_order),
         ..orig
     };
     conn.execute(
         "UPDATE feature_tasks SET title=?1,done=?2,sort_order=?3 WHERE id=?4",
         params![updated.title, updated.done as i64, updated.sort_order, id],
     )
     .map_err(|e| e.to_string())?;
     Ok(updated)
 }
 
 pub fn delete_feature_task_db(conn: &Connection, id: &str) -> AppResult<()> {
     conn.execute("DELETE FROM feature_tasks WHERE id=?1", params![id])
         .map_err(|e| e.to_string())?;
     Ok(())
 }
 
 // === PM: Aggregated State ===
 
 pub fn get_roadmap_state_db(conn: &Connection, group_id: &str) -> AppResult<crate::models::RoadmapState> {
     let features = get_features(conn, group_id)?;
     let all_task_ids: Vec<String> = features.iter().map(|f| f.id.clone()).collect();
     let mut all_tasks = Vec::new();
     for fid in &all_task_ids {
         all_tasks.extend(get_feature_tasks(conn, fid)?);
     }
     Ok(crate::models::RoadmapState {
         group_id: group_id.into(),
         items: get_roadmap_items(conn, group_id)?,
         features,
         tasks: all_tasks,
     })
 }

// === Shared Memory: Experiences ===

pub fn save_experience(conn: &Connection, group_id: &str, source_member_id: &str, title: &str, content: &str, tags: &str) -> AppResult<String> {
    let eid = id();
    let now_ts = now();
    conn.execute(
        "INSERT INTO experiences(id,group_id,source_member_id,title,content,tags,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![eid, group_id, source_member_id, title, content, tags, now_ts, now_ts],
    ).map_err(|e| e.to_string())?;
    if let Ok(group) = get_group(conn, group_id) {
        let ws = std::path::Path::new(&group.workspace_path);
        let _ = crate::memory::append_group_memory(ws, title, content);
        let kind: Option<String> = conn
            .query_row(
                "SELECT kind FROM members WHERE id=?1",
                params![source_member_id],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten();
        if kind.as_deref() == Some("agent") {
            let _ = crate::memory::append_agent_memory(ws, source_member_id, title, content);
        }
    }
    Ok(eid)
}

pub fn query_experiences(conn: &Connection, group_id: &str, query: &str, limit: i64) -> AppResult<Vec<Experience>> {
    let limit = limit.min(50).max(1);
    let pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT id,group_id,source_member_id,title,content,tags,created_at,updated_at FROM experiences WHERE group_id=?1 AND (content LIKE ?2 OR title LIKE ?3 OR tags LIKE ?4) ORDER BY created_at DESC LIMIT ?5"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![group_id, pattern, pattern, pattern, limit], |r| {
        Ok(Experience {
            id: r.get(0)?,
            group_id: r.get(1)?,
            source_member_id: r.get(2)?,
            title: r.get(3)?,
            content: r.get(4)?,
            tags: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    }).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| e.to_string())?);
    }
    Ok(results)
}

pub fn delete_experience(conn: &Connection, id: &str) -> AppResult<bool> {
    let n = conn.execute("DELETE FROM experiences WHERE id=?1", params![id]).map_err(|e| e.to_string())?;
    Ok(n > 0)
}


/// 裁决处于 pending 审查的任务（人类批准/拒绝与调度器内部 Agent 裁决共用同一状态机）。
/// 返回是否真正发生了变更（只有 pending → approved/rejected 才算）。
pub fn set_run_review(conn: &Connection, run_id: &str, review: &str, status: &str) -> AppResult<bool> {
    let changed = conn.execute(
        "UPDATE task_runs SET review_status=?1, status=?2 WHERE id=?3 AND review_status='pending'",
        params![review, status, run_id],
    ).map_err(|e| e.to_string())?;
    Ok(changed > 0)
}

fn feedback_counts(conn: &Connection, message_id: &str, member_id: &str) -> AppResult<(i64, i64, Option<String>)> {
    let up: i64 = conn
        .query_row("SELECT COUNT(*) FROM message_feedback WHERE message_id=?1 AND vote='up'", params![message_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let down: i64 = conn
        .query_row("SELECT COUNT(*) FROM message_feedback WHERE message_id=?1 AND vote='down'", params![message_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let mine: Option<String> = conn
        .query_row("SELECT vote FROM message_feedback WHERE message_id=?1 AND member_id=?2", params![message_id, member_id], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    Ok((up, down, mine))
}

/// 投票/取消投票：vote=Some("up"|"down") 覆盖；None 清除。返回聚合 (up, down, my_vote)。
pub fn vote_message(conn: &Connection, message_id: &str, member_id: &str, vote: Option<&str>) -> AppResult<(i64, i64, Option<String>)> {
    match vote {
        Some(v) if v == "up" || v == "down" => {
            conn.execute(
                "INSERT INTO message_feedback(message_id,member_id,vote,created_at) VALUES(?1,?2,?3,?4)
                 ON CONFLICT(message_id,member_id) DO UPDATE SET vote=?3, created_at=?4",
                params![message_id, member_id, v, now()],
            ).map_err(|e| e.to_string())?;
        }
        _ => {
            conn.execute(
                "DELETE FROM message_feedback WHERE message_id=?1 AND member_id=?2",
                params![message_id, member_id],
            ).map_err(|e| e.to_string())?;
        }
    }
    feedback_counts(conn, message_id, member_id)
}

/// 读取反馈聚合（不写）。
pub fn get_message_feedback(conn: &Connection, message_id: &str, member_id: &str) -> AppResult<(i64, i64, Option<String>)> {
    feedback_counts(conn, message_id, member_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_feedback_vote_toggles_and_counts() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.', 'u',NULL,1)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('u','g','user','u','#000','',1,1,'')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('a','g','agent','a','#000','',1,1,'')",
            [],
        ).unwrap();
        conn.execute("INSERT INTO messages VALUES('m','g','u',NULL,'x','completed',1)", []).unwrap();

        let (up, down, mine) = vote_message(&conn, "m", "u", Some("up")).unwrap();
        assert_eq!((up, down), (1, 0));
        assert_eq!(mine.as_deref(), Some("up"));

        // 同用户切到 down：up 归零
        let (up2, down2, mine2) = vote_message(&conn, "m", "u", Some("down")).unwrap();
        assert_eq!((up2, down2), (0, 1));
        assert_eq!(mine2.as_deref(), Some("down"));

        // 另一用户查看：聚合 1 down，my_vote None
        let (up3, down3, mine3) = get_message_feedback(&conn, "m", "a").unwrap();
        assert_eq!((up3, down3), (0, 1));
        assert_eq!(mine3, None);

        // 取消表决：清空
        let (up4, down4, mine4) = vote_message(&conn, "m", "u", None).unwrap();
        assert_eq!((up4, down4), (0, 0));
        assert_eq!(mine4, None);
    }

    #[test]
    fn secret_encrypt_roundtrip_and_legacy_passthrough() {
        // 1) 纯函数进退
        let plain = "sk-test-abcdef1234567890";
        let enc = encrypt_secret(plain).unwrap();
        assert!(enc.starts_with("v1:"), "落库应为加密格式");
        assert_eq!(decrypt_secret(&enc).unwrap(), plain);
        // 密文不可含明文
        assert!(!enc.contains(plain));

        // 2) 旧明文直通（存量兼容）
        assert_eq!(decrypt_secret("sk-legacy-plain").unwrap(), "sk-legacy-plain");

        // 3) 经 set/get 全链路（真实表）
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute("INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.','u',NULL,1)", []).unwrap();
        conn.execute("INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('a','g','agent','a','#000','',1,1,'')", []).unwrap();
        conn.execute("INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at) VALUES('a','mock',NULL,'ready',1)", []).unwrap();
        set_member_api_key(&conn, "a", Some("sk-real-99887766")).unwrap();
        // 落库原始值为 v1 密文
        let stored: String = conn.query_row("SELECT api_key FROM agent_profiles WHERE member_id='a'", [], |r| r.get(0)).unwrap();
        assert!(stored.starts_with("v1:"));
        // 读回解出原文
        assert_eq!(get_agent_api_key(&conn, "a").unwrap().as_deref(), Some("sk-real-99887766"));
        // 清空
        set_member_api_key(&conn, "a", None).unwrap();
        assert!(get_agent_api_key(&conn, "a").unwrap().is_none());
    }

    #[test]
    fn cancel_and_retry_service_functions() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute("INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.','u',NULL,1)", []).unwrap();
        conn.execute("INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('u','g','user','u','#000','',1,1,'')", []).unwrap();
        conn.execute("INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('a','g','agent','a','#000','',1,1,'')", []).unwrap();
        conn.execute("INSERT INTO messages VALUES('m','g','u',NULL,'x','completed',1)", []).unwrap();
        conn.execute("INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,created_at) VALUES('r','g','m','a',NULL,0,'running',1)", []).unwrap();

        // cancel：running → cancelled，返回 group/输出
        let info = cancel_run(&conn, "r").unwrap().unwrap();
        assert_eq!(info.group_id, "g");
        let status: String = conn.query_row("SELECT status FROM task_runs WHERE id='r'", [], |r| r.get(0)).unwrap();
        assert_eq!(status, "cancelled");
        // 已取消 → None（幂等）
        assert!(cancel_run(&conn, "r").unwrap().is_none());

        // retry：重开新 run
        let (new_id, group_id) = retry_run(&conn, "r").unwrap().unwrap();
        assert_eq!(group_id, "g");
        assert_ne!(new_id, "r");
        let new_status: String = conn.query_row("SELECT status FROM task_runs WHERE id=?1", params![new_id], |r| r.get(0)).unwrap();
        assert_eq!(new_status, "queued");
        // 不存在 → None
        assert!(retry_run(&conn, "no-such").unwrap().is_none());
    }

    #[test]
    fn run_phase_timeline_projected_from_run_events() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.', 'u',NULL,1)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('u','g','user','u','#000','',1,1,'')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('a','g','agent','a','#000','',1,1,'')",
            [],
        ).unwrap();
        conn.execute("INSERT INTO messages VALUES('m','g','u',NULL,'x','completed',1)", []).unwrap();
        conn.execute(
            "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,created_at) VALUES('r','g','m','a',NULL,0,'running',1)",
            [],
        ).unwrap();

        set_run_phase(&conn, "r", "starting").unwrap();
        set_run_phase(&conn, "r", "streaming").unwrap();
        let phases = list_run_phases(&conn, "r").unwrap();
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].phase, "starting");
        assert_eq!(phases[1].phase, "streaming");
        assert!(phases[0].created_at <= phases[1].created_at);
    }

    #[test]
    fn set_run_review_only_resolves_pending() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.', 'u',NULL,1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('u','g','user','u','#000','',1,1,'')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('a','g','agent','a','#000','',1,1,'')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at,cli_session_id) VALUES('a','mock',NULL,'unknown',1,NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages VALUES('m','g','u',NULL,'x','completed',1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at) VALUES('r','g','m','a',NULL,0,'awaiting_review',NULL,NULL,'pending',NULL,1,NULL,NULL)",
            [],
        )
        .unwrap();

        // 批准 pending → 持久化
        assert!(set_run_review(&conn, "r", "approved", "completed").unwrap());
        let (review, status): (String, String) = conn
            .query_row("SELECT review_status,status FROM task_runs WHERE id='r'", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(review, "approved");
        assert_eq!(status, "completed");

        // 已裁决 → 二次调用为空操作
        assert!(!set_run_review(&conn, "r", "rejected", "changes_requested").unwrap());
    }

    #[test]
    fn database_requeues_incomplete_runs_after_restart() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.', 'u',NULL,1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('u','g','user','u','#000','',1,1,'')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('a','g','agent','a','#000','',1,1,'')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agent_profiles(member_id,adapter,executable_path,runtime_status,updated_at,cli_session_id) VALUES('a','mock',NULL,'unknown',1,NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages VALUES('m','g','u',NULL,'x','completed',1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,depth,status,created_at) VALUES('r','g','m','a',0,'running',1)",
            [],
        )
        .unwrap();
        drop(conn);
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        let status: String = conn
            .query_row("SELECT status FROM task_runs WHERE id='r'", [], |r| r.get(0))
            .unwrap();
        let phase: Option<String> = conn
            .query_row("SELECT phase FROM task_runs WHERE id='r'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "queued");
        assert_eq!(phase.as_deref(), Some("recovering"));
    }

    #[test]
    fn scoped_user_only_sees_linked_group() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        let ts = now();
        conn.execute(
            "INSERT INTO users(id,username,password_hash,created_at,is_admin) VALUES('u1','alice','x',?1,0)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g1','G1','.','m-owner',NULL,?1)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g2','G2','.','m-owner2',NULL,?1)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,auth_user_id) VALUES('m1','g1','user','Alice','#000','',1,?1,'u1')",
            params![ts],
        )
        .unwrap();
        assert!(user_can_access_group(&conn, "u1", "g1").unwrap());
        assert!(!user_can_access_group(&conn, "u1", "g2").unwrap());
        let visible = get_groups_for_user(&conn, "u1").unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "g1");
        assert!(is_admin_user(&conn, "seed-user-root").unwrap());
    }

    #[test]
    fn list_joinable_and_link_existing_user() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        let ts = now();
        conn.execute(
            "INSERT INTO users(id,username,password_hash,created_at,is_admin) VALUES('u1','alice','x',?1,0)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users(id,username,password_hash,created_at,is_admin) VALUES('u2','bob','x',?1,0)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g1','G1','.','m-owner',NULL,?1)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,auth_user_id) VALUES('m1','g1','user','Alice','#000','',1,?1,'u1')",
            params![ts],
        )
        .unwrap();

        let joinable = list_joinable_users(&conn, "g1").unwrap();
        assert!(joinable.iter().any(|u| u.username == "bob"));
        assert!(!joinable.iter().any(|u| u.username == "alice"));

        let linked = resolve_user_member_auth_id(&conn, "g1", Some("u2"), None, None).unwrap();
        assert_eq!(linked, "u2");
        let err = resolve_user_member_auth_id(&conn, "g1", Some("u1"), None, None).unwrap_err();
        assert!(err.contains("已是本群"));
        let taken = resolve_user_member_auth_id(&conn, "g1", None, Some("alice"), Some("pw")).unwrap_err();
        assert!(taken.contains("已被占用"));
    }

    #[test]
    fn messages_hot_window_and_before_cursor() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.','u',NULL,1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES('u','g','user','u','#000','',1,1,'')",
            [],
        )
        .unwrap();
        for i in 1..=15 {
            conn.execute(
                "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,'g','u',NULL,?2,'completed',?3)",
                params![format!("m{i}"), format!("c{i}"), i * 10],
            )
            .unwrap();
        }
        let recent = get_messages_recent(&conn, "g", 5).unwrap();
        assert_eq!(recent.len(), 5);
        assert_eq!(recent.first().unwrap().id, "m11");
        assert_eq!(recent.last().unwrap().id, "m15");
        let older = get_messages_before(&conn, "g", recent[0].created_at, &recent[0].id, 3).unwrap();
        assert_eq!(older.len(), 3);
        assert_eq!(older.last().unwrap().id, "m10");
        let state = group_state(&conn, "g").unwrap();
        assert_eq!(state.messages_total, 15);
        assert!(!state.messages_has_more); // 15 < HOT 100
        assert_eq!(state.messages.len(), 15);
    }

    #[test]
    fn resolve_target_agents_skips_admin_when_only_user_mentioned() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        let ts = now();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.','u','admin',?1)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES('u','g','user','Owner','#000','',1,?1)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES('guest','g','user','Guest','#000','',1,?1)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES('admin','g','agent','Admin','#000','',1,?1)",
            params![ts],
        )
        .unwrap();
        // No mentions → admin fallback
        let no_mention = resolve_target_agent_ids(&conn, "g", Some("admin"), &[]).unwrap();
        assert_eq!(no_mention, vec!["admin".to_string()]);
        // Unset admin → no default responder
        let no_admin = resolve_target_agent_ids(&conn, "g", None, &[]).unwrap();
        assert!(no_admin.is_empty());
        // @user only → no agent
        let user_only =
            resolve_target_agent_ids(&conn, "g", Some("admin"), &["guest".into()]).unwrap();
        assert!(user_only.is_empty());
        // @agent → that agent
        let agent_hit =
            resolve_target_agent_ids(&conn, "g", Some("admin"), &["admin".into()]).unwrap();
        assert_eq!(agent_hit, vec!["admin".to_string()]);
    }

    #[test]
    fn resolve_target_chatbot_admin_is_default_responder() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        let ts = now();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,group_kind) VALUES('g','g','.','u','bot',?1,'chat')",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES('u','g','user','Owner','#000','',1,?1)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES('bot','g','chatbot','Cat','#000','',1,?1)",
            params![ts],
        )
        .unwrap();
        assert!(member_is_default_responder_candidate(&conn, "g", "bot").unwrap());
        let ids = resolve_target_agent_ids(&conn, "g", Some("bot"), &[]).unwrap();
        assert_eq!(ids, vec!["bot".to_string()]);
        // user cannot be default responder even if listed as admin
        let skip_user = resolve_target_agent_ids(&conn, "g", Some("u"), &[]).unwrap();
        assert!(skip_user.is_empty());
    }

    #[test]
    fn unread_counts_after_cursor_excludes_own() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        // Fixed timeline (avoid wall-clock races with mark_group_read's now()).
        let join_at = 1_000_000_i64;
        conn.execute(
            "INSERT INTO users(id,username,password_hash,created_at,is_admin) VALUES('u1','alice','x',?1,0)",
            params![join_at],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.','m1',NULL,?1)",
            params![join_at],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,auth_user_id) VALUES('m1','g','user','Alice','#000','',1,?1,'u1')",
            params![join_at],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES('bot','g','chatbot','Bot','#000','',1,?1)",
            params![join_at],
        )
        .unwrap();
        // before mark: baseline = member created_at → later messages count
        conn.execute(
            "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES('m-own','g','m1',NULL,'me','completed',?1)",
            params![join_at + 10],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES('m-bot','g','bot',NULL,'hi','completed',?1)",
            params![join_at + 20],
        )
        .unwrap();
        assert_eq!(count_group_unread(&conn, "u1", "g").unwrap(), 1);
        // Pin cursor after existing messages (do not depend on wall clock).
        conn.execute(
            "INSERT INTO group_read_cursors(user_id, group_id, last_read_at) VALUES('u1','g',?1)",
            params![join_at + 50],
        )
        .unwrap();
        assert_eq!(count_group_unread(&conn, "u1", "g").unwrap(), 0);
        conn.execute(
            "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES('m-bot2','g','bot',NULL,'again','completed',?1)",
            params![join_at + 100],
        )
        .unwrap();
        assert_eq!(count_group_unread(&conn, "u1", "g").unwrap(), 1);
    }

    #[test]
    fn chat_context_summary_roundtrip_and_messages_after() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        let ts = now();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,group_kind) VALUES('g','g','.','u',NULL,?1,'chat')",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES('u','g','user','u','#000','',1,?1)",
            params![ts],
        )
        .unwrap();
        for i in 1..=5 {
            conn.execute(
                "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES(?1,'g','u',NULL,?2,'completed',?3)",
                params![format!("m{i}"), format!("c{i}"), i * 10],
            )
            .unwrap();
        }
        upsert_chat_context_summary(&conn, "g", "摘要A", 30).unwrap();
        let s = get_chat_context_summary(&conn, "g").unwrap().unwrap();
        assert_eq!(s.summary_text, "摘要A");
        assert_eq!(s.through_created_at, 30);
        let after = get_messages_after_created_at(&conn, "g", 30, 10).unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(after[0].id, "m4");
    }

    #[test]
    fn seed_group_is_system_and_not_deletable() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        let seed = get_group(&conn, "seed-group-workpanel").unwrap();
        assert!(seed.is_system);
        let err = assert_group_deletable(&conn, "seed-group-workpanel").unwrap_err();
        assert!(err.contains("不可删除"));
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at,is_system) VALUES('g2','g2','.','u',NULL,1,0)",
            [],
        )
        .unwrap();
        assert!(assert_group_deletable(&conn, "g2").is_ok());
    }

    #[test]
    fn invite_create_preview_accept_and_hard_delete() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        let ts = now();
        conn.execute(
            "INSERT INTO users(id,username,password_hash,created_at,is_admin) VALUES('u2','bob','x',?1,0)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g1','InviteG','.','m-owner',NULL,?1)",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,auth_user_id) VALUES('m-owner','g1','user','Owner','#000','',1,?1,'seed-user-root')",
            params![ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,auth_user_id) VALUES('m-pending','g1','user','Guest','#000','',1,?1,NULL)",
            params![ts],
        )
        .unwrap();
        let invite = create_group_invite(&conn, "g1", "m-pending", Some("seed-user-root")).unwrap();
        let preview = preview_invite(&conn, &invite.token).unwrap();
        assert!(preview.valid);
        assert_eq!(preview.group_name, "InviteG");
        let member = get_member(&conn, "m-pending").unwrap();
        assert!(member.invite_pending);
        let joined = accept_invite(&conn, &invite.token, "u2").unwrap();
        assert_eq!(joined.auth_user_id.as_deref(), Some("u2"));
        assert!(!joined.invite_pending);
        assert!(user_can_access_group(&conn, "u2", "g1").unwrap());
        assert!(!preview_invite(&conn, &invite.token).unwrap().valid);

        // Pending-free hard delete with no history → row gone.
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES('m-tmp','g1','user','Tmp','#000','',0,?1)",
            params![ts],
        )
        .unwrap();
        hard_delete_member(&conn, "g1", "m-tmp").unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM members WHERE id='m-tmp'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);

        // With message history → roster_hidden.
        conn.execute(
            "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES('msg1','g1','m-pending',NULL,'hi','completed',?1)",
            params![ts],
        )
        .unwrap();
        hard_delete_member(&conn, "g1", "m-pending").unwrap();
        let hidden: i64 = conn
            .query_row(
                "SELECT roster_hidden FROM members WHERE id='m-pending'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hidden, 1);
        assert!(get_members(&conn, "g1")
            .unwrap()
            .iter()
            .all(|m| m.id != "m-pending"));
        assert!(!user_can_access_group(&conn, "u2", "g1").unwrap());
    }
}
