use crate::models::{Group, GroupState, Member, Message, RuntimeSettings, TaskRun};
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
    connection
        .execute_batch(
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
        ",
        )
        .map_err(|e| e.to_string())?;
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
    Ok(())
}

pub fn group_from_row(row: &Row<'_>) -> rusqlite::Result<Group> {
    Ok(Group {
        id: row.get(0)?,
        name: row.get(1)?,
        workspace_path: row.get(2)?,
        owner_member_id: row.get(3)?,
        admin_member_id: row.get(4)?,
        created_at: row.get(5)?,
    })
}

pub fn member_from_row(row: &Row<'_>) -> rusqlite::Result<Member> {
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
        created_at: row.get(10)?,
    })
}

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
        created_at: row.get(9)?,
        started_at: row.get(10)?,
        completed_at: row.get(11)?,
    })
}

pub fn get_groups(connection: &Connection) -> AppResult<Vec<Group>> {
    let mut stmt = connection
        .prepare(
            "SELECT id,name,workspace_path,owner_member_id,admin_member_id,created_at FROM groups ORDER BY created_at DESC",
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
            "SELECT id,name,workspace_path,owner_member_id,admin_member_id,created_at FROM groups WHERE id=?1",
            params![group_id],
            group_from_row,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "找不到群聊。".to_string())
}

pub fn get_members(connection: &Connection, group_id: &str) -> AppResult<Vec<Member>> {
    let mut stmt = connection
        .prepare(
            "SELECT m.id,m.group_id,m.kind,m.display_name,m.avatar_color,m.role_description,m.is_active,p.adapter,p.executable_path,p.runtime_status,m.created_at
         FROM members m LEFT JOIN agent_profiles p ON p.member_id=m.id WHERE m.group_id=?1 ORDER BY m.created_at",
        )
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
            "SELECT id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,output_message_id,error_message,created_at,started_at,completed_at FROM task_runs WHERE group_id=?1 ORDER BY created_at",
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
    conn.execute(
        "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,created_at) VALUES(?1,?2,?3,?4,?5,?6,'queued',?7)",
        params![
            run_id,
            group_id,
            root_message_id,
            agent_member_id,
            parent_run_id,
            depth,
            now()
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
        if kind.as_deref() == Some("agent:1") {
            agents.push(member_id.clone());
        }
    }
    Ok(agents)
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
            "INSERT INTO groups VALUES('g','g','.', 'u',NULL,1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members VALUES('u','g','user','u','#000','',1,1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO members VALUES('a','g','agent','a','#000','',1,1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agent_profiles VALUES('a','mock',NULL,'unknown',1)",
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
