use super::*;
use crate::db::{init_db, open_db};
use crate::event_sender::EventSender;
use rusqlite::params;

fn state(path: &std::path::Path) -> SchedulerState {
    let (tx, _rx) = tokio::sync::broadcast::channel(8);
    SchedulerState {
        db_path: path.to_path_buf(),
        event_sender: EventSender::Web(tx),
        cancellations: Default::default(),
        scheduling_groups: Default::default(),
        live_sessions: Default::default(),
    }
}

fn seed(path: &std::path::Path) {
    init_db(path).unwrap();
    let conn = open_db(path).unwrap();
    conn.execute(
        "INSERT INTO groups(id,name,workspace_path,owner_member_id,admin_member_id,created_at) VALUES('g','g','.', 'u','lead',1)",
        [],
    )
    .unwrap();
    for (id, name, kind) in [
        ("u", "Root", "user"),
        ("lead", "Leader", "agent"),
        ("a", "TargetA", "agent"),
    ] {
        conn.execute(
            "INSERT INTO members(id,group_id,kind,display_name,avatar_color,role_description,is_active,created_at,tags) VALUES(?1,'g',?2,?3,'#000','',1,1,'')",
            params![id, kind, name],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO messages(id,group_id,sender_member_id,parent_run_id,content,status,created_at) VALUES('m','g','u',NULL,'do it','completed',1)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,created_at) VALUES('r-lead','g','m','lead',NULL,0,'completed',1)",
        [],
    )
    .unwrap();
}

fn insert_child(conn: &rusqlite::Connection, id: &str, agent: &str, status: &str) {
    conn.execute(
        "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,created_at) VALUES(?1,'g','m',?2,'r-lead',1,?3,2)",
        params![id, agent, status],
    )
    .unwrap();
}

#[test]
fn classify_samples() {
    assert_eq!(
        classify("request timed out after 600s"),
        FailureClass::Retryable
    );
    assert_eq!(
        classify("Connection reset by peer"),
        FailureClass::Retryable
    );
    assert_eq!(classify("429 Too Many Requests"), FailureClass::Retryable);
    assert_eq!(
        classify("401 Unauthorized: invalid api key"),
        FailureClass::Permanent
    );
    assert_eq!(
        classify("codex: command not found"),
        FailureClass::Permanent
    );
    assert_eq!(classify("subscription expired"), FailureClass::Permanent);
    assert_eq!(classify("cancelled by user"), FailureClass::Cancelled);
    assert_eq!(classify("mysterious failure"), FailureClass::Unknown);
}

fn input() -> RouteInput {
    RouteInput {
        has_parent: true,
        is_reviewer: false,
        is_leader_run: false,
        has_leader: true,
        depth: 1,
        max_depth: 4,
        attempts: 1,
        max_retries: 1,
        class: FailureClass::Retryable,
    }
}

#[test]
fn decide_first_retryable_failure_retries() {
    assert_eq!(decide(&input()), RouteAction::Retry);
}

#[test]
fn decide_exhausted_retries_escalates() {
    let exhausted = RouteInput {
        attempts: 2,
        ..input()
    };
    assert_eq!(decide(&exhausted), RouteAction::Escalate);
}

#[test]
fn decide_permanent_escalates_without_retry() {
    let permanent = RouteInput {
        class: FailureClass::Permanent,
        ..input()
    };
    assert_eq!(decide(&permanent), RouteAction::Escalate);
}

#[test]
fn decide_cancelled_stops() {
    let cancelled = RouteInput {
        class: FailureClass::Cancelled,
        ..input()
    };
    assert_eq!(decide(&cancelled), RouteAction::Stop);
}

#[test]
fn decide_guards_stop() {
    assert_eq!(
        decide(&RouteInput {
            has_parent: false,
            ..input()
        }),
        RouteAction::Stop
    );
    assert_eq!(
        decide(&RouteInput {
            is_reviewer: true,
            ..input()
        }),
        RouteAction::Stop
    );
    assert_eq!(
        decide(&RouteInput {
            is_leader_run: true,
            ..input()
        }),
        RouteAction::Stop
    );
    assert_eq!(
        decide(&RouteInput {
            depth: 4,
            ..input()
        }),
        RouteAction::Stop
    );
}

#[test]
fn decide_escalate_without_leader_stops() {
    let permanent = RouteInput {
        class: FailureClass::Permanent,
        has_leader: false,
        ..input()
    };
    assert_eq!(decide(&permanent), RouteAction::Stop);
}

#[test]
fn first_failure_creates_retry_run() {
    let file = tempfile::NamedTempFile::new().unwrap();
    seed(file.path());
    let conn = open_db(file.path()).unwrap();
    insert_child(&conn, "r-a", "a", "failed");
    drop(conn);

    let s = state(file.path());
    let (group_id, runs) = route_failed_run(&s, "r-a", "request timed out").expect("should route");
    assert_eq!(group_id, "g");
    assert_eq!(runs.len(), 1);

    let conn = open_db(file.path()).unwrap();
    let (agent, parent, depth, status): (String, Option<String>, i64, String) = conn
        .query_row(
            "SELECT agent_member_id,parent_run_id,depth,status FROM task_runs WHERE id=?1",
            params![runs[0]],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(agent, "a");
    assert_eq!(parent.as_deref(), Some("r-lead"));
    assert_eq!(depth, 1);
    assert_eq!(status, "queued");
    let events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM run_events WHERE run_id='r-a' AND kind='route'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(events, 1);
}

#[test]
fn second_failure_escalates_to_leader() {
    let file = tempfile::NamedTempFile::new().unwrap();
    seed(file.path());
    let conn = open_db(file.path()).unwrap();
    insert_child(&conn, "r-a1", "a", "failed");
    insert_child(&conn, "r-a2", "a", "failed");
    drop(conn);

    let s = state(file.path());
    let (_group, runs) = route_failed_run(&s, "r-a2", "timed out again").expect("should escalate");
    assert_eq!(runs.len(), 1);

    let conn = open_db(file.path()).unwrap();
    let (agent, depth, status, root): (String, i64, String, String) = conn
        .query_row(
            "SELECT agent_member_id,depth,status,root_message_id FROM task_runs WHERE id=?1",
            params![runs[0]],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(agent, "lead");
    assert_eq!(depth, 1);
    assert_eq!(status, "queued");
    let content: String = conn
        .query_row(
            "SELECT content FROM messages WHERE id=?1",
            params![root],
            |row| row.get(0),
        )
        .unwrap();
    assert!(content.starts_with("[路由升级]"));
    assert!(content.contains("timed out again"));
}

#[test]
fn leader_failure_does_not_escalate() {
    let file = tempfile::NamedTempFile::new().unwrap();
    seed(file.path());
    let conn = open_db(file.path()).unwrap();
    insert_child(&conn, "r-lead-fail", "lead", "failed");
    drop(conn);

    let s = state(file.path());
    assert!(route_failed_run(&s, "r-lead-fail", "timed out").is_none());
}

#[test]
fn root_failure_is_not_routed() {
    let file = tempfile::NamedTempFile::new().unwrap();
    seed(file.path());
    let conn = open_db(file.path()).unwrap();
    conn.execute(
        "INSERT INTO task_runs(id,group_id,root_message_id,agent_member_id,parent_run_id,depth,status,created_at) VALUES('r-root','g','m','a',NULL,0,'failed',2)",
        [],
    )
    .unwrap();
    drop(conn);

    let s = state(file.path());
    assert!(route_failed_run(&s, "r-root", "timed out").is_none());
}

#[test]
fn permanent_failure_escalates_immediately() {
    let file = tempfile::NamedTempFile::new().unwrap();
    seed(file.path());
    let conn = open_db(file.path()).unwrap();
    insert_child(&conn, "r-a", "a", "failed");
    drop(conn);

    let s = state(file.path());
    let (_group, runs) = route_failed_run(&s, "r-a", "401 unauthorized").expect("should escalate");
    let conn = open_db(file.path()).unwrap();
    let agent: String = conn
        .query_row(
            "SELECT agent_member_id FROM task_runs WHERE id=?1",
            params![runs[0]],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(agent, "lead");
}
