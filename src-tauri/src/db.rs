use crate::models::{
    Experience, Feature, FeatureTask, Group, GroupState, Member, Message, RoadmapItem, RuntimeSettings, TaskRun,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use uuid::Uuid;

pub type AppResult<T> = Result<T, String>;

pub fn now() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn id() -> String {
    Uuid::new_v4().to_string()
}

pub fn open_db(path: &Path) -> AppResult<Connection> {
    let connection = Connection::open(path).map_err(|e| e.to_string())?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
        .map_err(|e| e.to_string())?;
    Ok(connection)
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
    // Migration: add review columns to task_runs for existing databases
    for col in &["review_status", "reviewer_member_id"] {
        let _ = connection.execute(
            &format!("ALTER TABLE task_runs ADD COLUMN {} TEXT", col),
            [],
        );
    }
    // Phase 2: agent_tags column for smart routing
    for col in &["tags"] {
        let _ = connection.execute(
            &format!("ALTER TABLE members ADD COLUMN {} TEXT NOT NULL DEFAULT ''", col),
            [],
        );
    }
    // Cursor (and future) CLI session reuse per agent member
    let _ = connection.execute(
        "ALTER TABLE agent_profiles ADD COLUMN cli_session_id TEXT",
        [],
    );
    // Group announcement (= project-level rule for all agents)
    let _ = connection.execute(
        "ALTER TABLE groups ADD COLUMN announcement TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = connection.execute(
        "ALTER TABLE groups ADD COLUMN announcement_updated_at INTEGER",
        [],
    );
    for col_sql in [
        "ALTER TABLE agent_profiles ADD COLUMN workspace_path TEXT",
        "ALTER TABLE agent_profiles ADD COLUMN api_key TEXT",
        "ALTER TABLE agent_profiles ADD COLUMN keep_alive INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE agent_profiles ADD COLUMN last_heartbeat_at INTEGER",
        "ALTER TABLE agent_profiles ADD COLUMN warm_status TEXT NOT NULL DEFAULT 'cold'",
        "ALTER TABLE task_runs ADD COLUMN phase TEXT",
        "ALTER TABLE task_runs ADD COLUMN phase_updated_at INTEGER",
    ] {
        let _ = connection.execute(col_sql, []);
    }
    migrate_members_allow_chatbot(&connection)?;
    for (key, value) in [
        ("max_concurrent_runs", "3"),
        ("run_timeout_seconds", "900"),
        ("context_message_limit", "40"),
        ("max_delegation_depth", "2"),
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
            "UPDATE task_runs SET status='interrupted', completed_at=?1 WHERE status IN ('queued','running')",
            params![now()],
        )
        .map_err(|e| e.to_string())?;
    connection
        .execute(
            "UPDATE messages SET status='interrupted' WHERE status='streaming'",
            [],
        )
        .map_err(|e| e.to_string())?;
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
            "INSERT INTO users(id, username, password_hash, created_at) VALUES(?1, 'root', ?2, ?3)
             ON CONFLICT(username) DO UPDATE SET password_hash=excluded.password_hash",
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
        return Ok(());
    }

    connection
        .execute(
            "INSERT INTO groups(id, name, workspace_path, owner_member_id, admin_member_id, created_at)
             VALUES(?1, 'LinlisWorkPanel', ?2, ?3, ?4, ?5)",
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

    Ok(())
}

fn migrate_members_allow_chatbot(connection: &Connection) -> AppResult<()> {
    let ddl: String = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='members'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if ddl.contains("chatbot") {
        return Ok(());
    }
    connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .map_err(|e| e.to_string())?;
    let result = (|| -> AppResult<()> {
        connection
            .execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS members_new (
  id TEXT PRIMARY KEY,
  group_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('user','agent','chatbot')),
  display_name TEXT NOT NULL,
  avatar_color TEXT NOT NULL,
  role_description TEXT NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL,
  tags TEXT NOT NULL DEFAULT ''
);
INSERT INTO members_new(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags)
SELECT id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,COALESCE(tags,'') FROM members;
DROP TABLE members;
ALTER TABLE members_new RENAME TO members;
"#,
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    let _ = connection.execute_batch("PRAGMA foreign_keys=ON;");
    result
}

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
    })
}

pub fn member_from_row(row: &Row<'_>) -> rusqlite::Result<Member> {
    let api_key: Option<String> = row.get(13)?;
    Ok(Member {
        id: row.get(0)?,
        group_id: row.get(1)?,
        kind: row.get(2)?,
        display_name: row.get(3)?,
        avatar_color: row.get(4)?,
        role_description: row.get(5)?,
        is_active: row.get::<_, i64>(6)? != 0,
        adapter: row.get(7)?,
        executable_path: row.get(8)?,
        runtime_status: row.get(9)?,
        tags: row.get(10)?,
        created_at: row.get(11)?,
        workspace_path: row.get(12)?,
        api_key_set: api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false),
        keep_alive: row.get::<_, i64>(14)? != 0,
        warm_status: row.get(15)?,
    })
}

pub const MEMBER_SELECT: &str = "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,COALESCE(m.tags,''),m.created_at,p.workspace_path,p.api_key,COALESCE(p.keep_alive,0),p.warm_status
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
    })
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
        .prepare(
            "SELECT id,name,workspace_path,owner_member_id,admin_member_id,created_at,COALESCE(announcement,''),announcement_updated_at FROM groups ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], group_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get_group(connection: &Connection, group_id: &str) -> AppResult<Group> {
    connection
        .query_row(
            "SELECT id,name,workspace_path,owner_member_id,admin_member_id,created_at,COALESCE(announcement,''),announcement_updated_at FROM groups WHERE id=?1",
            params![group_id],
            group_from_row,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "找不到群聊。".to_string())
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
    connection
        .query_row(
            "SELECT api_key FROM agent_profiles WHERE member_id=?1",
            params![member_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())
        .map(|v| v.flatten())
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
            "{MEMBER_SELECT} WHERE m.group_id=?1 ORDER BY m.created_at"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], member_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get_messages(connection: &Connection, group_id: &str) -> AppResult<Vec<Message>> {
    let mut stmt = connection
        .prepare(
            "SELECT id,group_id,sender_member_id,parent_run_id,content,status,created_at FROM messages WHERE group_id=?1 ORDER BY created_at",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], message_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get_runs(connection: &Connection, group_id: &str) -> AppResult<Vec<TaskRun>> {
    let mut stmt = connection
        .prepare(
            "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,review_status,reviewer_member_id,created_at,started_at,completed_at,phase,phase_updated_at FROM task_runs WHERE group_id=?1 ORDER BY created_at",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], run_from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn group_state(connection: &Connection, group_id: &str) -> AppResult<GroupState> {
    Ok(GroupState {
        group: get_group(connection, group_id)?,
        members: get_members(connection, group_id)?,
        messages: get_messages(connection, group_id)?,
        runs: get_runs(connection, group_id)?,
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
    Ok(RuntimeSettings {
        max_concurrent_runs: get("max_concurrent_runs")?,
        run_timeout_seconds: get("run_timeout_seconds")?,
        context_message_limit: get("context_message_limit")?,
        max_delegation_depth: get("max_delegation_depth")?,
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
            "INSERT INTO run_events(id,run_id,kind,payload,created_at) VALUES(?1,?2,?3,?4,?5)",
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_marks_incomplete_runs_interrupted() {
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
        assert_eq!(status, "interrupted");
    }
}
