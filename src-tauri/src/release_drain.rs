//! Release drain: reject new agent starts while in-flight runs finish (smooth deploy).

use crate::db::{now, open_db, AppResult};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

pub const SETTING_DRAIN: &str = "release_drain";
pub const SETTING_DRAIN_SINCE: &str = "release_drain_since";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrainStatus {
    pub enabled: bool,
    pub since: Option<i64>,
    pub running: i64,
    pub queued: i64,
}

fn setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let mut stmt = conn
        .prepare("SELECT value FROM app_settings WHERE key=?1")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![key]).map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let v: String = row.get(0).map_err(|e| e.to_string())?;
        Ok(Some(v))
    } else {
        Ok(None)
    }
}

fn set_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn is_enabled(conn: &Connection) -> AppResult<bool> {
    Ok(matches!(
        setting(conn, SETTING_DRAIN)?.as_deref().map(str::trim),
        Some("1") | Some("true") | Some("yes") | Some("on")
    ))
}

pub fn set_enabled(conn: &Connection, enabled: bool) -> AppResult<DrainStatus> {
    if enabled {
        set_setting(conn, SETTING_DRAIN, "1")?;
        let since_empty = setting(conn, SETTING_DRAIN_SINCE)?
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        if since_empty {
            set_setting(conn, SETTING_DRAIN_SINCE, &now().to_string())?;
        }
    } else {
        set_setting(conn, SETTING_DRAIN, "0")?;
        set_setting(conn, SETTING_DRAIN_SINCE, "")?;
    }
    status(conn)
}

pub fn clear_on_startup(conn: &Connection) -> AppResult<()> {
    set_setting(conn, SETTING_DRAIN, "0")?;
    set_setting(conn, SETTING_DRAIN_SINCE, "")?;
    Ok(())
}

pub fn status(conn: &Connection) -> AppResult<DrainStatus> {
    let enabled = is_enabled(conn)?;
    let since = setting(conn, SETTING_DRAIN_SINCE)?
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| s.parse::<i64>().ok());
    let running: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_runs WHERE status='running'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let queued: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_runs WHERE status='queued'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(DrainStatus {
        enabled,
        since,
        running,
        queued,
    })
}

pub fn status_at(db_path: &Path) -> AppResult<DrainStatus> {
    let conn = open_db(db_path)?;
    status(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use tempfile::NamedTempFile;

    #[test]
    fn toggle_drain_and_counts() {
        let file = NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        let s0 = status(&conn).unwrap();
        assert!(!s0.enabled);
        let s1 = set_enabled(&conn, true).unwrap();
        assert!(s1.enabled);
        assert!(s1.since.is_some());
        let s2 = set_enabled(&conn, false).unwrap();
        assert!(!s2.enabled);
    }
}
