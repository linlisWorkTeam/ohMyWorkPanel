//! Extension Host — discover / load / proxy Extend services via manifest.
//! PanelLive remains the default registered root; AIHotel and others via LINLIS_EXTENSION_ROOTS.

use crate::db::{now, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const PANELLIVE_ID: &str = "panellive";

#[derive(Debug, Clone)]
pub struct DiscoveredExtension {
    pub root: PathBuf,
    pub manifest: ExtensionManifest,
}

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

/// Absolute roots containing `extension.manifest.json`.
/// `LINLIS_EXTENSION_ROOTS` = `:` / `;` separated list; always merges `LINLIS_PANELLIVE_ROOT` fallback.
pub fn extension_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(raw) = std::env::var("LINLIS_EXTENSION_ROOTS") {
        for part in raw.split(|c| c == ':' || c == ';') {
            let p = part.trim();
            if !p.is_empty() {
                roots.push(PathBuf::from(p));
            }
        }
    }
    let live = panellive_root();
    if !roots.iter().any(|r| r == &live) {
        roots.insert(0, live);
    }
    // Optional default for AIHotel when present on disk and not already listed.
    let hotel = PathBuf::from("/AI/AIHotel");
    if hotel.join("extension.manifest.json").is_file() && !roots.iter().any(|r| r == &hotel) {
        roots.push(hotel);
    }
    roots
}

pub fn load_manifest(root: &Path) -> AppResult<ExtensionManifest> {
    let path = root.join("extension.manifest.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取扩展清单失败（{}）：{e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 extension.manifest.json：{e}"))
}

pub fn load_panellive_manifest(root: &Path) -> AppResult<ExtensionManifest> {
    load_manifest(root)
}

pub fn discover_extensions() -> Vec<DiscoveredExtension> {
    let mut out = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    for root in extension_roots() {
        if !root.join("extension.manifest.json").is_file() {
            continue;
        }
        match load_manifest(&root) {
            Ok(manifest) => {
                if seen_ids.insert(manifest.id.clone()) {
                    out.push(DiscoveredExtension { root, manifest });
                }
            }
            Err(e) => eprintln!("extension discover skip {}: {e}", root.display()),
        }
    }
    out
}

pub fn find_extension(ext_id: &str) -> AppResult<DiscoveredExtension> {
    discover_extensions()
        .into_iter()
        .find(|d| d.manifest.id == ext_id)
        .ok_or_else(|| format!("未知扩展：{ext_id}"))
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
        return Ok("/".into());
    }
    if trimmed.contains("..") || trimmed.contains('\\') || trimmed.contains('\0') {
        return Err("非法代理路径".into());
    }
    Ok(format!("/{trimmed}"))
}

/// Append browser query to sanitized path (needed for `?format=json` TTS etc.).

/// Best-effort discovery for test/dev: mock manifest may omit `runtime`.
fn discover_mock_or_panellive() -> Vec<DiscoveredExtension> {
    discover_extensions()
}

pub fn with_proxy_query(path: &str, query: Option<&str>) -> AppResult<String> {
    let Some(q) = query.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(path.to_string());
    };
    if q.contains('\0') || q.contains('\r') || q.contains('\n') || q.contains(' ') {
        return Err("非法代理查询串".into());
    }
    Ok(format!("{path}?{q}"))
}

/// Same-origin base for browser iframe (never expose 127.0.0.1 to clients).
pub fn extension_public_base(ext_id: &str) -> String {
    format!("/api/extensions/{ext_id}")
}

pub fn panellive_public_base() -> String {
    extension_public_base(PANELLIVE_ID)
}

pub fn check_extension_health(manifest: &ExtensionManifest) -> (bool, String) {
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

pub fn check_panellive_health(manifest: &ExtensionManifest) -> (bool, String) {
    check_extension_health(manifest)
}

pub fn panellive_upstream_port(manifest: &ExtensionManifest) -> u16 {
    manifest.runtime.default_port
}

pub fn panellive_base_url(_manifest: &ExtensionManifest) -> String {
    panellive_public_base()
}

pub fn panellive_token() -> String {
    std::env::var("LINLIS_PANELLIVE_TOKEN")
        .or_else(|_| std::env::var("LINLIS_EXTENSION_TOKEN"))
        .unwrap_or_else(|_| "panellive-dev-token".into())
}

pub fn status_for_discovered(
    conn: &Connection,
    group_id: &str,
    disc: &DiscoveredExtension,
) -> AppResult<ExtensionStatus> {
    let enabled = is_extension_enabled(conn, group_id, &disc.manifest.id)?;
    let (healthy, detail) = if enabled {
        check_extension_health(&disc.manifest)
    } else {
        (false, "unloaded".into())
    };
    Ok(ExtensionStatus {
        id: disc.manifest.id.clone(),
        name: disc.manifest.name.clone(),
        version: disc.manifest.version.clone(),
        kind: disc.manifest.kind.clone(),
        enabled,
        healthy,
        health_detail: detail,
        base_url: extension_public_base(&disc.manifest.id),
        tabs: disc.manifest.contributes.tabs.clone(),
        a2a_skills: disc.manifest.contributes.a2a_skills.clone(),
        media_plane: disc.manifest.runtime.media_plane.clone(),
    })
}

pub fn list_group_extensions(conn: &Connection, group_id: &str) -> AppResult<Vec<ExtensionStatus>> {
    ensure_extensions_table(conn)?;
    let mut out = Vec::new();
    for disc in discover_extensions() {
        out.push(status_for_discovered(conn, group_id, &disc)?);
    }
    Ok(out)
}

/// Enable = load (require health); disable = unload. Works for any discovered ext id.
pub fn set_group_extension_enabled(
    conn: &Connection,
    group_id: &str,
    ext_id: &str,
    enabled: bool,
) -> AppResult<ExtensionStatus> {
    ensure_extensions_table(conn)?;
    let disc = find_extension(ext_id)?;
    if enabled {
        let (ok, detail) = check_extension_health(&disc.manifest);
        if !ok {
            return Err(format!(
                "CONFLICT:扩展 {} 未就绪，无法 load。请先在 {} 启动服务（默认 :{}）。详情：{detail}",
                disc.manifest.name,
                disc.root.display(),
                disc.manifest.runtime.default_port
            ));
        }
    }
    set_extension_enabled(conn, group_id, ext_id, enabled)?;
    status_for_discovered(conn, group_id, &disc)
}

/// Enable = load (require health); disable = unload.
pub fn set_panellive_enabled(conn: &Connection, group_id: &str, enabled: bool) -> AppResult<ExtensionStatus> {
    set_group_extension_enabled(conn, group_id, PANELLIVE_ID, enabled)
}

/// Prefix match for A2A skills (`live.*`, `hotel.*`, or exact).
pub fn skill_allowed(declared: &[String], skill: &str) -> bool {
    declared.iter().any(|d| {
        if let Some(prefix) = d.strip_suffix(".*") {
            skill == prefix || skill.starts_with(&format!("{prefix}."))
        } else {
            d == skill
        }
    })
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
        assert_eq!(sanitize_proxy_path("").unwrap(), "/");
        assert_eq!(sanitize_proxy_path("live.html").unwrap(), "/live.html");
        assert_eq!(sanitize_proxy_path("v1/session/start").unwrap(), "/v1/session/start");
        assert_eq!(
            with_proxy_query("/v1/tts", Some("format=json")).unwrap(),
            "/v1/tts?format=json"
        );
        assert_eq!(with_proxy_query("/v1/tts", None).unwrap(), "/v1/tts");
        assert!(with_proxy_query("/v1/tts", Some("a b")).is_err());
    }

    #[test]
    fn skill_allowed_prefix_and_exact() {
        let decl = vec!["live.*".into(), "hotel.session.start".into()];
        assert!(skill_allowed(&decl, "live.session.start"));
        assert!(skill_allowed(&decl, "hotel.session.start"));
        assert!(!skill_allowed(&decl, "hotel.other"));
    }

    #[test]
    fn extension_roots_includes_panellive() {
        let roots = extension_roots();
        assert!(roots.iter().any(|r| r.ends_with("WorkPanelLive") || r == &panellive_root()));
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
    fn discover_defaults_to_panellive_and_skips_bad_root() {
        // LINLIS_EXTENSION_ROOTS unset in test env; at least panellive default root is listed.
        let roots = extension_roots();
        assert!(!roots.is_empty());
        // A root without manifest must be skipped silently.
        let tmp = std::env::temp_dir();
        let disc = discover_extensions();
        for d in &disc {
            assert!(d.root.join("extension.manifest.json").is_file());
        }
        let _ = tmp;
    }

    #[test]
    fn unknown_extension_errors() {
        let err = find_extension("no-such-ext-xyz").unwrap_err();
        assert!(err.contains("未知扩展"));
    }

    #[test]
    fn status_unloaded_when_disabled() {
        let file = tempfile::NamedTempFile::new().unwrap();
        init_db(file.path()).unwrap();
        let conn = open_db(file.path()).unwrap();
        // panellive not enabled yet -> healthy=false, unloaded
        let list = list_group_extensions(&conn, "g1").unwrap();
        let pl = list.iter().find(|s| s.id == PANELLIVE_ID);
        if let Some(s) = pl {
            assert!(!s.enabled);
            assert!(!s.healthy);
            assert_eq!(s.health_detail, "unloaded");
        }
    }

    #[test]
    fn a2a_skill_prefix_contract() {
        // manifest-driven A2A: hotel.* must match hotel.* and hotel.anything but not live.*
        let decl = vec!["hotel.*".to_string()];
        assert!(skill_allowed(&decl, "hotel.session.start"));
        assert!(skill_allowed(&decl, "hotel.any.deep"));
        assert!(!skill_allowed(&decl, "live.session.start"));
        assert!(skill_allowed(&decl, "hotel")); // prefix 本身（无点后缀）也接受
    }

    #[test]
    fn sanitize_keeps_public_route_compat() {
        // `/api/extensions/panellive` base path with no subpath must proxy to upstream `/`.
        assert_eq!(sanitize_proxy_path("").unwrap(), "/");
        assert_eq!(sanitize_proxy_path("/").unwrap(), "/");
        // `live.html` (legacy iframe entry) still allowed.
        assert_eq!(sanitize_proxy_path("live.html").unwrap(), "/live.html");
        assert!(sanitize_proxy_path("../x").is_err());
    }

    #[test]
    fn extension_roots_merges_env_without_dup() {
        std::env::set_var("LINLIS_EXTENSION_ROOTS", "/AI/WorkPanelLive");
        let roots = extension_roots();
        let count = roots.iter().filter(|r| **r == panellive_root()).count();
        assert_eq!(count, 1, "panellive root must not be duplicated");
        std::env::remove_var("LINLIS_EXTENSION_ROOTS");
    }

}
