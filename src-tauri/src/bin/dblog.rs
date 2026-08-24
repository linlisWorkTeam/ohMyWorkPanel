// 开发调试工具：导出桌面端 SQLite 的最近运行/事件/日志（只读打开，不影响运行中的应用）。
// 用法：cargo run --bin dblog -- "C:\Users\<你>\AppData\Roaming\com.ohmyworkpanel.app\ohmyworkpanel.sqlite3" [条数]
use rusqlite::{params, Connection};

fn short(s: &str, max: usize) -> String {
    let t: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        format!("{t}…")
    } else {
        t
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).cloned().unwrap_or_else(|| {
        eprintln!("用法: dblog <db路径> [条数]");
        std::process::exit(2);
    });
    let limit = args.get(2).and_then(|s| s.parse::<i64>().ok()).unwrap_or(25);
    let conn = Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap_or_else(|e| {
        eprintln!("打开 {} 失败：{e}", path);
        std::process::exit(1);
    });
    println!("== 最近 run_events（run_id / kind / payload / 时间） ==");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT run_id, kind, COALESCE(payload,''), datetime(created_at/1000,'unixepoch','localtime')
         FROM run_events ORDER BY created_at DESC LIMIT ?1",
    ) {
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (rid, kind, payload, ts) = row.unwrap();
            println!("[{ts}] {rid} {kind} {payload}", payload = short(&payload, 260));
        }
    } else {
        println!("(run_events 表不存在？)");
    }

    println!("\n== 最近 task_runs（id / status / error / phase） ==");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, status, COALESCE(error_message,''), COALESCE(phase,''), datetime(created_at/1000,'unixepoch','localtime')
         FROM task_runs ORDER BY created_at DESC LIMIT ?1",
    ) {
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (id, status, err, phase, ts) = row.unwrap();
            println!("[{ts}] {status} phase={phase} err={}", err = short(&err, 220));
        }
    } else {
        println!("(task_runs 表不存在？)");
    }

    println!("\n== 最近 logs（level / source / message） ==");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT level, source, message, datetime(created_at/1000,'unixepoch','localtime')
         FROM logs ORDER BY created_at DESC LIMIT ?1",
    ) {
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (lvl, src, msg, ts) = row.unwrap();
            println!("[{ts}] {lvl} {src}: {}", short(&msg, 220));
        }
    } else {
        println!("(logs 表不存在？)");
    }

    println!("\n== 群（id / name / kind / workspace） ==");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, name, COALESCE(group_kind,'project'), COALESCE(workspace_path,'') FROM groups ORDER BY created_at",
    ) {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (id, name, kind, ws) = row.unwrap();
            println!("{kind:8} {id} 「{name}」 ws={}", short(&ws, 60));
        }
    }

    println!("\n== 机器人配置（agent_profiles） ==");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT member_id, COALESCE(adapter,''), COALESCE(model,''), COALESCE(api_url,''), COALESCE(warm_status,'') FROM agent_profiles",
    ) {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (mid, adapter, model, api_url, warm) = row.unwrap();
            println!("{mid} adapter={adapter} model={model} api_url={} warm={warm}", short(&api_url, 50));
        }
    }

    println!("\n== 最近消息（group / sender / content 前 160 字 / status） ==");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT m.group_id, m.sender_member_id, m.status, substr(m.content,1,160), datetime(m.created_at/1000,'unixepoch','localtime')
         FROM messages m ORDER BY m.created_at DESC LIMIT ?1",
    ) {
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (gid, sid, status, content, ts) = row.unwrap();
            println!("[{ts}] {gid} {sid} [{status}] {content}", content = short(&content, 160));
        }
    } else {
        println!("(messages 表查询失败？)");
    }

    println!("\n== chat_context_summaries（chatbot 滚动摘要） ==");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT group_id, COALESCE(summary_text,''), through_created_at, datetime(updated_at/1000,'unixepoch','localtime')
         FROM chat_context_summaries",
    ) {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (gid, summary, through, ts) = row.unwrap();
            println!("[{ts}] {gid} through={through} summary={}", short(&summary, 260));
        }
    } else {
        println!("(chat_context_summaries 表不存在？)");
    }

    // 生命周期锚：确保 conn 存活到函数末尾（stmt 的借用提前结束后强约束）
    let _ = &conn;
    let _ = params![];
}