use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;

/// Log severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

impl From<&str> for LogLevel {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "debug" => LogLevel::Debug,
            "warn" | "warning" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub level: String,
    pub source: String,
    pub message: String,
    pub details: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub level: Option<String>,
    pub source: Option<String>,
    pub since: Option<i64>,
}

/// Create the logs table if not exists
pub fn init_logs_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS logs (
            id TEXT PRIMARY KEY,
            level TEXT NOT NULL,
            source TEXT NOT NULL,
            message TEXT NOT NULL,
            details TEXT,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_logs_created ON logs(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_logs_level ON logs(level);",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn now() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn id() -> String {
    Uuid::new_v4().to_string()
}

/// Write a log entry to the database
pub fn log(
    conn: &Connection,
    level: LogLevel,
    source: &str,
    message: &str,
    details: Option<&str>,
) -> Result<(), String> {
    let entry = LogEntry {
        id: id(),
        level: level.to_string(),
        source: source.to_string(),
        message: message.to_string(),
        details: details.map(|d| d.to_string()),
        created_at: now(),
    };
    conn.execute(
        "INSERT INTO logs(id,level,source,message,details,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
        params![entry.id, entry.level, entry.source, entry.message, entry.details, entry.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Query logs with filters and pagination
pub fn query_logs(conn: &Connection, q: &LogQuery) -> Result<Vec<LogEntry>, String> {
    let limit = q.limit.unwrap_or(100).min(500);
    let offset = q.offset.unwrap_or(0);

    let mut sql = String::from(
        "SELECT id,level,source,message,details,created_at FROM logs WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref level) = q.level {
        sql.push_str(" AND level = ?");
        params_vec.push(Box::new(level.clone()));
    }
    if let Some(ref source) = q.source {
        sql.push_str(" AND source LIKE ?");
        params_vec.push(Box::new(format!("%{}%", source)));
    }
    if let Some(since) = q.since {
        sql.push_str(" AND created_at >= ?");
        params_vec.push(Box::new(since));
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
    params_vec.push(Box::new(limit));
    params_vec.push(Box::new(offset));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(LogEntry {
                id: row.get(0)?,
                level: row.get(1)?,
                source: row.get(2)?,
                message: row.get(3)?,
                details: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| e.to_string())?);
    }
    Ok(results)
}

/// Count logs with optional level/source filter (matches query_logs behavior)
pub fn count_logs(conn: &Connection, level: Option<&str>, source: Option<&str>) -> Result<i64, String> {
    let mut sql = "SELECT COUNT(*) FROM logs WHERE 1=1".to_string();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(lvl) = level {
        sql.push_str(" AND level=?");
        params_vec.push(Box::new(lvl.to_string()));
    }
    if let Some(src) = source {
        sql.push_str(" AND source LIKE ?");
        params_vec.push(Box::new(format!("%{}%", src)));
    }
    let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    conn.query_row(&sql, param_refs.as_slice(), |r| r.get(0))
        .map_err(|e| e.to_string())
}

/// Clear all logs
pub fn clear_logs(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM logs", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Convenience: log an info event
pub fn info(conn: &Connection, source: &str, message: &str, details: Option<&str>) {
    let _ = log(conn, LogLevel::Info, source, message, details);
}

/// Convenience: log a warning event
pub fn warn(conn: &Connection, source: &str, message: &str, details: Option<&str>) {
    let _ = log(conn, LogLevel::Warn, source, message, details);
}

/// Convenience: log an error event
pub fn error(conn: &Connection, source: &str, message: &str, details: Option<&str>) {
    let _ = log(conn, LogLevel::Error, source, message, details);
}
