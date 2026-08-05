//! Extension Host — load/unload Extend services (PanelLive MVP).

use crate::db::{now, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const PANELLIVE_ID: &str = "panellive";

/// Fallback when `GET /v1/llm-prompt` is unreachable (must match WorkPanelLive docs).
pub const PANELLIVE_LLM_PROMPT_FALLBACK: &str = "【PanelLive 语音模式 · 强制】你的最终回复将送给 CosyVoice TTS。每次最终输出必须严格少于 50 个汉字（含标点按字计；勿超过 50）。只说结论与必要动作，禁止长文、列表堆砌、代码块、多段解释。若信息较多，只保留最关键一句，其余留到下一轮语音。";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    #[serde(default)]
    pub description: String,
    pub contributes: ExtensionContributes,
    pub runtime: ExtensionRuntime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionContributes {
    #[serde(default)]
    pub tabs: Vec<ExtensionTab>,
    #[serde(default)]
    pub a2a_skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionTab {
    pub id: String,
    pub title: String,
    pub route: String,
    pub entry: String,
    #[serde(default)]
    pub peer_of: Vec<String>,
    #[serde(default)]
    pub disabled_when_unloaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRuntime {
    #[serde(default = "default_health_path")]
    pub health_path: String,
    #[serde(default = "default_port")]
    pub default_port: u16,
    #[serde(default)]
    pub media_plane: String,
}

fn default_health_path() -> String {
    "/health".into()
}
fn default_port() -> u16 {
    8790
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStatus {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub enabled: bool,
    pub healthy: bool,
    pub health_detail: String,
    pub base_url: String,
    pub tabs: Vec<ExtensionTab>,
    pub a2a_skills: Vec<String>,
    pub media_plane: String,
}

pub fn panellive_root() -> PathBuf {
    std::env::var("LINLIS_PANELLIVE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/AI/WorkPanelLive"))
}

pub fn load_panellive_manifest(root: &Path) -> AppResult<ExtensionManifest> {
    let path = root.join("extension.manifest.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取 PanelLive 清单失败（{}）：{e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 extension.manifest.json：{e}"))
}

pub fn ensure_extensions_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS group_extensions (
          group_id TEXT NOT NULL,
          extension_id TEXT NOT NULL,
          enabled INTEGER NOT NULL DEFAULT 0,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY (group_id, extension_id)
        );
        "#,
    )
    .map_err(|e| e.to_string())
}

pub fn is_extension_enabled(conn: &Connection, group_id: &str, extension_id: &str) -> AppResult<bool> {
    let flag: Option<i64> = conn
        .query_row(
            "SELECT enabled FROM group_extensions WHERE group_id=?1 AND extension_id=?2",
            params![group_id, extension_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(flag.unwrap_or(0) != 0)
}

pub fn set_extension_enabled(
    conn: &Connection,
    group_id: &str,
    extension_id: &str,
    enabled: bool,
) -> AppResult<()> {
    ensure_extensions_table(conn)?;
    conn.execute(
        "INSERT INTO group_extensions(group_id, extension_id, enabled, updated_at)
         VALUES(?1,?2,?3,?4)
         ON CONFLICT(group_id, extension_id) DO UPDATE SET enabled=excluded.enabled, updated_at=excluded.updated_at",
        params![group_id, extension_id, if enabled { 1 } else { 0 }, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct HttpExchange {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

/// Minimal HTTP/1.0 exchange — avoids adding reqwest on 2GB hosts.
pub fn http_exchange_local(
    method: &str,
    host: &str,
    port: u16,
    path: &str,
    body: Option<&[u8]>,
    content_type: Option<&str>,
) -> AppResult<HttpExchange> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr).map_err(|e| format!("连接 {addr} 失败：{e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut req = format!("{method} {path} HTTP/1.0\r\nHost: {host}:{port}\r\nConnection: close\r\n");
    if let Some(bytes) = body {
        let ct = content_type.unwrap_or("application/octet-stream");
        req.push_str(&format!("Content-Type: {ct}\r\nContent-Length: {}\r\n", bytes.len()));
        req.push_str("\r\n");
        stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
        stream.write_all(bytes).map_err(|e| e.to_string())?;
    } else {
        req.push_str("\r\n");
        stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    }
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let sep = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "PanelLive 响应无头部结束标记".to_string())?;
    let header = String::from_utf8_lossy(&buf[..sep]);
    let status = header
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let mut content_type = "application/octet-stream".to_string();
    for line in header.lines().skip(1) {
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-type:") {
            content_type = rest.trim().to_string();
            // restore original casing from header line
            if let Some((_, v)) = line.split_once(':') {
                content_type = v.trim().to_string();
            }
            break;
        }
    }
    Ok(HttpExchange {
        status,
        content_type,
        body: buf[sep + 4..].to_vec(),
    })
}

pub fn http_get_local(host: &str, port: u16, path: &str) -> AppResult<(u16, String)> {
    let ex = http_exchange_local("GET", host, port, path, None, None)?;
    Ok((
        ex.status,
        String::from_utf8_lossy(&ex.body).into_owned(),
    ))
}

pub fn http_post_json_local(host: &str, port: u16, path: &str, json_body: &str) -> AppResult<(u16, String)> {
    let ex = http_exchange_local(
        "POST",
        host,
        port,
        path,
        Some(json_body.as_bytes()),
        Some("application/json"),
    )?;
    Ok((
        ex.status,
        String::from_utf8_lossy(&ex.body).into_owned(),
    ))
}

pub fn sanitize_proxy_path(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return Ok("/live.html".into());
    }
    if trimmed.contains("..") || trimmed.contains('\\') || trimmed.contains('\0') {
        return Err("非法代理路径".into());
    }
    Ok(format!("/{trimmed}"))
}

/// Same-origin base for browser iframe (never expose 127.0.0.1 to clients).
pub fn panellive_public_base() -> &'static str {
    "/api/extensions/panellive"
}

pub fn check_panellive_health(manifest: &ExtensionManifest) -> (bool, String) {
    let path = if manifest.runtime.health_path.starts_with('/') {
        manifest.runtime.health_path.clone()
    } else {
        format!("/{}", manifest.runtime.health_path)
    };
    match http_get_local("127.0.0.1", manifest.runtime.default_port, &path) {
        Ok((200, body)) => (true, body.chars().take(200).collect()),
        Ok((code, body)) => (false, format!("HTTP {code}: {}", body.chars().take(120).collect::<String>())),
        Err(e) => (false, e),
    }
}

pub fn panellive_upstream_port(manifest: &ExtensionManifest) -> u16 {
    manifest.runtime.default_port
}

pub fn panellive_base_url(_manifest: &ExtensionManifest) -> String {
    panellive_public_base().into()
}

pub fn panellive_token() -> String {
    std::env::var("LINLIS_PANELLIVE_TOKEN").unwrap_or_else(|_| "panellive-dev-token".into())
}

/// Parse `GET /v1/llm-prompt` body (`{"prompt":"..."}` or plain text).
pub fn parse_llm_prompt_response(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(p) = v
            .get("prompt")
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(p.to_string());
        }
        return None;
    }
    Some(trimmed.to_string())
}

pub fn fetch_panellive_llm_prompt(port: u16) -> String {
    match http_get_local("127.0.0.1", port, "/v1/llm-prompt") {
        Ok((200, body)) => parse_llm_prompt_response(&body)
            .unwrap_or_else(|| PANELLIVE_LLM_PROMPT_FALLBACK.to_string()),
        _ => PANELLIVE_LLM_PROMPT_FALLBACK.to_string(),
    }
}

/// ChatBot or the group's admin member when Live is on.
pub fn should_inject_live_short_reply(
    agent_kind: &str,
    agent_id: &str,
    admin_member_id: Option<&str>,
) -> bool {
    agent_kind == "chatbot" || admin_member_id == Some(agent_id)
}

/// Non-empty suffix for prompts when PanelLive is enabled for the group.
pub fn live_short_reply_block(
    conn: &Connection,
    group_id: &str,
    agent_kind: &str,
    agent_id: &str,
    admin_member_id: Option<&str>,
) -> String {
    if !should_inject_live_short_reply(agent_kind, agent_id, admin_member_id) {
        return String::new();
    }
    if !is_extension_enabled(conn, group_id, PANELLIVE_ID).unwrap_or(false) {
        return String::new();
    }
    let port = load_panellive_manifest(&panellive_root())
        .map(|m| panellive_upstream_port(&m))
        .unwrap_or(8790);
    let prompt = fetch_panellive_llm_prompt(port);
    format!("\n\n{prompt}\n")
}

pub fn list_group_extensions(conn: &Connection, group_id: &str) -> AppResult<Vec<ExtensionStatus>> {
    ensure_extensions_table(conn)?;
    let root = panellive_root();
    let manifest = load_panellive_manifest(&root)?;
    let enabled = is_extension_enabled(conn, group_id, PANELLIVE_ID)?;
    let (healthy, detail) = if enabled {
        check_panellive_health(&manifest)
    } else {
        (false, "unloaded".into())
    };
    Ok(vec![ExtensionStatus {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        kind: manifest.kind.clone(),
        enabled,
        healthy,
        health_detail: detail,
        base_url: panellive_base_url(&manifest),
        tabs: manifest.contributes.tabs.clone(),
        a2a_skills: manifest.contributes.a2a_skills.clone(),
        media_plane: manifest.runtime.media_plane.clone(),
    }])
}

/// Enable = load (require health); disable = unload.
pub fn set_panellive_enabled(conn: &Connection, group_id: &str, enabled: bool) -> AppResult<ExtensionStatus> {
    ensure_extensions_table(conn)?;
    let root = panellive_root();
    let manifest = load_panellive_manifest(&root)?;
    if enabled {
        let (ok, detail) = check_panellive_health(&manifest);
        if !ok {
            return Err(format!(
                "CONFLICT:PanelLive 未就绪，无法 load。请先在 {} 执行 npm start（默认 :{}）。详情：{detail}",
                root.display(),
                manifest.runtime.default_port
            ));
        }
    }
    set_extension_enabled(conn, group_id, PANELLIVE_ID, enabled)?;
    let list = list_group_extensions(conn, group_id)?;
    list.into_iter()
        .next()
        .ok_or_else(|| "扩展状态缺失".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_db, open_db};

    #[test]
    fn loads_manifest_from_workpanellive() {
        let root = PathBuf::from("/AI/WorkPanelLive");
        if !root.join("extension.manifest.json").exists() {
            return;
        }
        let m = load_panellive_manifest(&root).unwrap();
        assert_eq!(m.id, "panellive");
        assert!(m.contributes.a2a_skills.iter().any(|s| s == "live.session.start"));
        assert!(m.contributes.tabs.iter().any(|t| t.route == "tab://live"));
    }

    #[test]
    fn sanitize_proxy_rejects_dotdot() {
        assert!(sanitize_proxy_path("../etc/passwd").is_err());
        assert_eq!(sanitize_proxy_path("live.html").unwrap(), "/live.html");
        assert_eq!(sanitize_proxy_path("v1/session/start").unwrap(), "/v1/session/start");
    }

    #[test]
    fn enable_disable_persists() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        ensure_extensions_table(&conn).unwrap();
        assert!(!is_extension_enabled(&conn, "g1", PANELLIVE_ID).unwrap());
        set_extension_enabled(&conn, "g1", PANELLIVE_ID, true).unwrap();
        assert!(is_extension_enabled(&conn, "g1", PANELLIVE_ID).unwrap());
        set_extension_enabled(&conn, "g1", PANELLIVE_ID, false).unwrap();
        assert!(!is_extension_enabled(&conn, "g1", PANELLIVE_ID).unwrap());
    }

    #[test]
    fn parse_llm_prompt_json_and_plain() {
        let json = r#"{"mode":"panellive","prompt":"短回复强制","ttsMaxChars":50}"#;
        assert_eq!(parse_llm_prompt_response(json).as_deref(), Some("短回复强制"));
        assert_eq!(
            parse_llm_prompt_response("  纯文本提示  ").as_deref(),
            Some("纯文本提示")
        );
        assert!(parse_llm_prompt_response(r#"{"ttsMaxChars":50}"#).is_none());
        assert!(should_inject_live_short_reply("chatbot", "c1", None));
        assert!(should_inject_live_short_reply("agent", "a1", Some("a1")));
        assert!(!should_inject_live_short_reply("agent", "a2", Some("a1")));
    }

    #[test]
    fn live_block_empty_when_extension_off() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        ensure_extensions_table(&conn).unwrap();
        let block = live_short_reply_block(&conn, "g1", "chatbot", "bot", None);
        assert!(block.is_empty());
        set_extension_enabled(&conn, "g1", PANELLIVE_ID, true).unwrap();
        let block = live_short_reply_block(&conn, "g1", "chatbot", "bot", None);
        assert!(block.contains("50") || block.contains("PanelLive"));
        let skip = live_short_reply_block(&conn, "g1", "agent", "other", Some("admin"));
        assert!(skip.is_empty());
    }
}
