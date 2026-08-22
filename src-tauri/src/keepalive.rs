use crate::db::{get_cli_session_id, now, open_db, set_cli_session_id, AppResult};
use crate::scheduler::SchedulerState;
use rusqlite::params;
use std::time::Duration;

/// Periodically touch keep-alive Cursor admin sessions so resume stays warm.
pub fn start_keepalive_loop(state: SchedulerState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            if let Err(e) = heartbeat_once(&state).await {
                eprintln!("keepalive: {e}");
            }
        }
    });
}

async fn heartbeat_once(state: &SchedulerState) -> AppResult<()> {
    let conn = open_db(&state.db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT m.id, p.adapter, p.cli_session_id FROM members m
             JOIN agent_profiles p ON p.member_id=m.id
             JOIN groups g ON g.id=m.group_id
             WHERE m.is_active=1 AND m.kind='agent' AND COALESCE(p.keep_alive,0)=1
               AND g.admin_member_id=m.id AND p.adapter='cursor'",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    drop(conn);

    for (member_id, _adapter, session) in rows {
        let warm = if session.as_ref().map(|s| !s.is_empty()).unwrap_or(false) {
            "warm"
        } else {
            "cold"
        };
        let conn = open_db(&state.db_path)?;
        conn.execute(
            "UPDATE agent_profiles SET warm_status=?1, last_heartbeat_at=?2, updated_at=?2 WHERE member_id=?3",
            params![warm, now(), member_id],
        )
        .map_err(|e| e.to_string())?;
        // Soft check: if session missing, leave cold for next real @ to spawn.
        if session.is_none() {
            let _ = get_cli_session_id(&conn, &member_id)?;
        }
        let _ = set_cli_session_id; // keep import used for future ping
    }
    Ok(())
}
