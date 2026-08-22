//! Live agent model catalogs. Cursor is synced from CLI; others stay static (TODO).

use crate::adapters::{models as static_models, AdapterKind};
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_SYNC_SECS: u64 = 6 * 60 * 60;
const LIST_TIMEOUT_SECS: u64 = 45;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentModelsResponse {
    pub adapters: HashMap<String, Vec<String>>,
    pub cursor_source: String,
    pub cursor_synced_at: Option<i64>,
    /// Adapters not yet covered by live sync.
    pub todos: Vec<String>,
}

#[derive(Debug, Default)]
struct CursorCache {
    models: Vec<String>,
    synced_at: Option<i64>,
    source: String,
    refreshing: bool,
}

fn cache() -> &'static Mutex<CursorCache> {
    static CACHE: OnceLock<Mutex<CursorCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(CursorCache {
            models: Vec::new(),
            synced_at: None,
            source: "fallback".into(),
            refreshing: false,
        })
    })
}

fn sync_interval_secs() -> Option<u64> {
    match std::env::var("OHMYWORKPANEL_CURSOR_MODEL_SYNC_SECS") {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Some(DEFAULT_SYNC_SECS);
            }
            let n: u64 = trimmed.parse().unwrap_or(DEFAULT_SYNC_SECS);
            if n == 0 {
                None
            } else {
                Some(n.max(60))
            }
        }
        Err(_) => Some(DEFAULT_SYNC_SECS),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Parse `cursor-agent --list-models` text into model ids (CLI order).
pub fn parse_cursor_list_models(output: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("Available models")
            || line.starts_with("Tip:")
            || line.starts_with("Parameterized")
        {
            continue;
        }
        // `id - Label` or `id — Label`
        let id = line
            .split_once(" - ")
            .or_else(|| line.split_once(" — "))
            .map(|(a, _)| a.trim())
            .unwrap_or("");
        if id.is_empty() || !is_plausible_model_id(id) {
            continue;
        }
        if !out.iter().any(|x| x == id) {
            out.push(id.to_string());
        }
    }
    out
}

fn is_plausible_model_id(id: &str) -> bool {
    if id.len() > 80 || id.contains(' ') {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '[' | ']' | '=' | ','))
}

fn resolve_cursor_bin() -> String {
    AdapterKind::Cursor
        .resolve_executable(None)
        .unwrap_or_else(|_| "cursor-agent".into())
}

fn run_cursor_list_models() -> Result<Vec<String>, String> {
    let bin = resolve_cursor_bin();
    let mut child = Command::new(&bin)
        .arg("--list-models")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {bin}: {e}"))?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > Duration::from_secs(LIST_TIMEOUT_SECS) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{bin} --list-models timed out after {LIST_TIMEOUT_SECS}s"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(format!("wait {bin}: {e}")),
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("read {bin} output: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stdout.trim().is_empty() {
        stderr.into_owned()
    } else {
        stdout.into_owned()
    };
    if !output.status.success() && parse_cursor_list_models(&combined).is_empty() {
        return Err(format!(
            "{bin} --list-models exit {}: {}",
            output.status,
            combined.chars().take(240).collect::<String>()
        ));
    }
    let models = parse_cursor_list_models(&combined);
    if models.is_empty() {
        return Err("parsed zero models from --list-models".into());
    }
    Ok(models)
}

/// Refresh Cursor catalog from CLI. Concurrent calls coalesce.
pub fn refresh_cursor_blocking() -> Result<usize, String> {
    {
        let mut g = cache().lock().map_err(|e| e.to_string())?;
        if g.refreshing {
            return Ok(g.models.len());
        }
        g.refreshing = true;
    }
    let result = run_cursor_list_models();
    let mut g = cache().lock().map_err(|e| e.to_string())?;
    g.refreshing = false;
    match result {
        Ok(models) => {
            let n = models.len();
            g.models = models;
            g.synced_at = Some(now_ms());
            g.source = "live".into();
            Ok(n)
        }
        Err(e) => {
            if g.models.is_empty() {
                g.source = "fallback".into();
            }
            Err(e)
        }
    }
}

fn static_adapter_models(adapter: &str) -> Vec<String> {
    static_models::models_for_adapter(adapter)
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

fn cursor_models_locked(g: &CursorCache) -> (Vec<String>, String) {
    if !g.models.is_empty() {
        (g.models.clone(), g.source.clone())
    } else {
        (static_adapter_models("cursor"), "fallback".into())
    }
}

pub fn catalog_response() -> AgentModelsResponse {
    let mut adapters = HashMap::new();
    for key in [
        "codex",
        "cursor",
        "claude-code",
        "opencode",
        "openclaw",
        "chatbot-deepseek",
        "chatbot-opencode-go",
        "mock",
    ] {
        if key == "cursor" {
            continue;
        }
        let models = static_adapter_models(key);
        if !models.is_empty() || key == "mock" {
            adapters.insert(key.to_string(), models);
        }
    }
    let (cursor_models, cursor_source, cursor_synced_at) = {
        let g = cache().lock().unwrap_or_else(|e| e.into_inner());
        let (m, src) = cursor_models_locked(&g);
        (m, src, g.synced_at)
    };
    adapters.insert("cursor".into(), cursor_models);

    AgentModelsResponse {
        adapters,
        cursor_source,
        cursor_synced_at,
        todos: vec![
            "TODO: live sync Codex model list".into(),
            "TODO: live sync Claude Code model list".into(),
            "TODO: live sync OpenClaw / OpenCode model lists".into(),
            "TODO: persist live catalog to SQLite for cold start".into(),
        ],
    }
}

/// Background: refresh once at start, then on interval (if configured).
pub fn start_cursor_model_sync_loop() {
    tokio::spawn(async move {
        match tokio::task::spawn_blocking(refresh_cursor_blocking).await {
            Ok(Ok(n)) => eprintln!("model_catalog: cursor live sync ok ({n} models)"),
            Ok(Err(e)) => eprintln!("model_catalog: cursor live sync failed (using fallback): {e}"),
            Err(e) => eprintln!("model_catalog: cursor sync join error: {e}"),
        }
        let Some(secs) = sync_interval_secs() else {
            eprintln!("model_catalog: periodic sync disabled (OHMYWORKPANEL_CURSOR_MODEL_SYNC_SECS=0)");
            return;
        };
        let mut ticker = tokio::time::interval(Duration::from_secs(secs));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await; // skip immediate double-fire
        loop {
            ticker.tick().await;
            match tokio::task::spawn_blocking(refresh_cursor_blocking).await {
                Ok(Ok(n)) => eprintln!("model_catalog: cursor refresh ok ({n} models)"),
                Ok(Err(e)) => eprintln!("model_catalog: cursor refresh failed: {e}"),
                Err(e) => eprintln!("model_catalog: cursor refresh join error: {e}"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_list_models_sample() {
        let raw = r#"Available models

auto - Auto (default)
cursor-grok-4.6-high-fast - Cursor Grok 4.6 Fast
cursor-grok-4.6-xhigh - Cursor Grok 4.6 Extra High
composer-2.5 - Composer 2.5

Tip: use --model <id>
"#;
        let ids = parse_cursor_list_models(raw);
        assert_eq!(
            ids,
            vec![
                "auto",
                "cursor-grok-4.6-high-fast",
                "cursor-grok-4.6-xhigh",
                "composer-2.5",
            ]
        );
    }

    #[test]
    fn catalog_includes_cursor_and_todos() {
        let resp = catalog_response();
        assert!(resp.adapters.get("cursor").is_some());
        assert!(resp.adapters.get("codex").is_some());
        assert!(resp.todos.iter().any(|t| t.contains("Codex")));
    }
}
