//! 版本化 schema 迁移（`PRAGMA user_version` 驱动）。
//!
//! 历史遗留的「每启动跑一遍的 ADD COLUMN / 表重建」被收编为受版本控制的迁移：
//! v1 = 当前 schema（吸收所有已有增量列 + members 表重建以便支持 chatbot/tags）。
//! 之后任何 schema 改动：新增 `migrate_v{N}` + `SCHEMA_VERSION += 1` + 一个升级测试，
//! 不要再往 `db::init_db` 里裸加 ALTER。

use rusqlite::Connection;

use crate::db::AppResult;

pub const SCHEMA_VERSION: i64 = 1;

/// 运行所有未执行的迁移，并把 `user_version` 升到 `SCHEMA_VERSION`。
/// 幂等：已是最新版本时直接返回。
pub fn migrate(connection: &Connection) -> AppResult<()> {
    let current: i64 = connection
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if current >= SCHEMA_VERSION {
        return Ok(());
    }
    for version in (current + 1)..=SCHEMA_VERSION {
        match version {
            1 => migrate_v1(connection)?,
            _ => unreachable!("invalid schema version {}", version),
        }
    }
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// v1：从「schema 逐列打补丁」时代快照出的完整补丁集。
/// 每条 ALTER 幂等（列已存在时 SQLite 报错，特此忽略）；members 重建由 DDL 守卫幂等。
fn migrate_v1(connection: &Connection) -> AppResult<()> {
    for col in &["review_status", "reviewer_member_id"] {
        let _ = connection.execute(
            &format!("ALTER TABLE task_runs ADD COLUMN {} TEXT", col),
            [],
        );
    }
    let _ = connection.execute(
        "ALTER TABLE members ADD COLUMN tags TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = connection.execute(
        "ALTER TABLE agent_profiles ADD COLUMN cli_session_id TEXT",
        [],
    );
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
        "ALTER TABLE agent_profiles ADD COLUMN model TEXT",
        "ALTER TABLE agent_profiles ADD COLUMN system_locked INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE task_runs ADD COLUMN phase TEXT",
        "ALTER TABLE task_runs ADD COLUMN phase_updated_at INTEGER",
        "ALTER TABLE groups ADD COLUMN group_kind TEXT NOT NULL DEFAULT 'project'",
        "ALTER TABLE groups ADD COLUMN archived INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE groups ADD COLUMN is_system INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE members ADD COLUMN auth_user_id TEXT",
        "ALTER TABLE members ADD COLUMN roster_hidden INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0",
    ] {
        let _ = connection.execute(col_sql, []);
    }
    migrate_members_allow_chatbot(connection)?;
    Ok(())
}

/// 老库 members 表没有 `chatbot` 分支 / `tags` 列时重建（幂等：DDL 已含 chatbot 则跳过）。
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_db, open_db};
    use tempfile::NamedTempFile;

    fn column_names(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({})", table))
            .unwrap();
        let cols = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        cols
    }

    fn user_version(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn fresh_db_sets_version_and_has_target_columns() {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        assert!(column_names(&conn, "members").contains(&"tags".to_string()));
        assert!(column_names(&conn, "members").contains(&"auth_user_id".to_string()));
        assert!(column_names(&conn, "task_runs").contains(&"review_status".to_string()));
        assert!(column_names(&conn, "agent_profiles").contains(&"api_key".to_string()));
        assert!(column_names(&conn, "groups").contains(&"group_kind".to_string()));

        // members DDL 允许 chatbot
        let ddl: String = conn
            .query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='members'", [], |r| r.get(0))
            .unwrap();
        assert!(ddl.contains("chatbot"));
    }

    #[test]
    fn legacy_db_upgrades_preserving_rows() {
        let file = NamedTempFile::new().unwrap();
        // 用「旧版」schema 裸建库（无 chatbot/tags、无增量列），预置一行数据
        {
            let conn = open_db(file.path()).unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE groups (
                  id TEXT PRIMARY KEY, name TEXT NOT NULL, workspace_path TEXT NOT NULL,
                  owner_member_id TEXT NOT NULL, admin_member_id TEXT, created_at INTEGER NOT NULL
                );
                CREATE TABLE members (
                  id TEXT PRIMARY KEY, group_id TEXT NOT NULL,
                  kind TEXT NOT NULL CHECK(kind IN ('user','agent')), display_name TEXT NOT NULL,
                  avatar_color TEXT NOT NULL, role_description TEXT NOT NULL,
                  is_active INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL
                );
                CREATE TABLE agent_profiles (
                  member_id TEXT PRIMARY KEY, adapter TEXT NOT NULL, executable_path TEXT,
                  runtime_status TEXT NOT NULL DEFAULT 'unknown', updated_at INTEGER NOT NULL
                );
                CREATE TABLE task_runs (
                  id TEXT PRIMARY KEY, group_id TEXT NOT NULL, root_message_id TEXT NOT NULL,
                  agent_member_id TEXT NOT NULL, parent_run_id TEXT, depth INTEGER NOT NULL DEFAULT 0,
                  status TEXT NOT NULL, output_message_id TEXT, error_message TEXT,
                  created_at INTEGER NOT NULL, started_at INTEGER, completed_at INTEGER
                );
                INSERT INTO groups(id,name,workspace_path,owner_member_id,created_at) VALUES('g','old','.','o',1);
                INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at) VALUES('m','g','user','olo','#000','',1,1);
                "#,
            )
            .unwrap();
        }
        // 应用迁移（init_db 内部会跑 db_migrations::migrate）
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);

        // 增量列全部补上
        assert!(column_names(&conn, "members").contains(&"tags".to_string()));
        assert!(column_names(&conn, "task_runs").contains(&"review_status".to_string()));
        assert!(column_names(&conn, "agent_profiles").contains(&"api_key".to_string()));
        assert!(column_names(&conn, "groups").contains(&"group_kind".to_string()));

        // members 重建后允许 chatbot 且数据保留
        let ddl: String = conn
            .query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='members'", [], |r| r.get(0))
            .unwrap();
        assert!(ddl.contains("chatbot"));
        let names: String = conn
            .query_row("SELECT display_name FROM members WHERE id='m'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(names, "olo");
        let group: String = conn
            .query_row("SELECT name FROM groups WHERE id='g'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(group, "old");
    }

    #[test]
    fn second_boot_is_idempotent() {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        // 二次 init_db：不报错、版本稳定、基础表仍在
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='members'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
