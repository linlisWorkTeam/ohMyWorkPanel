//! Embedded Codex Responses↔ChatCompletions shim (listens on 127.0.0.1:18888).
//!
//! Runs as a Node sidecar owned by the WorkPanel server process (`kill_on_drop`),
//! so Codex no longer depends on a separate systemd unit that can die silently.
//! If the port is already bound (other slot / leftover), we skip spawn and reuse it.

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::sleep;

const DEFAULT_PORT: u16 = 18888;

pub struct CodexProxyHandle {
    child: Option<Child>,
    port: u16,
}

impl CodexProxyHandle {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn managed_child(&self) -> bool {
        self.child.is_some()
    }
}

/// Start the shim if nothing is listening yet. Keep the returned handle alive
/// for the lifetime of the server so the child is killed on shutdown.
pub async fn start_embedded() -> CodexProxyHandle {
    let port = std::env::var("LINLIS_CODEX_PROXY_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    if port_open(port) {
        eprintln!("Codex proxy: 127.0.0.1:{port} already up — reusing (no sidecar spawn)");
        return CodexProxyHandle { child: None, port };
    }

    let script = resolve_script_path();
    if !script.is_file() {
        eprintln!(
            "Codex proxy: script missing at {} — Codex DeepSeek shim unavailable",
            script.display()
        );
        return CodexProxyHandle { child: None, port };
    }

    let log_path = std::env::var("LINLIS_CODEX_PROXY_LOG")
        .unwrap_or_else(|_| "/tmp/codex-deepseek-proxy.log".into());
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);
    let (stdout, stderr) = match log_file {
        Ok(f) => {
            let f2 = f.try_clone().ok();
            (
                f.into(),
                f2.map(std::process::Stdio::from)
                    .unwrap_or_else(std::process::Stdio::null),
            )
        }
        Err(_) => (std::process::Stdio::null(), std::process::Stdio::null()),
    };

    match Command::new("node")
        .arg(&script)
        .env("LINLIS_CODEX_PROXY_PORT", port.to_string())
        .stdout(stdout)
        .stderr(stderr)
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => {
            let pid = child.id().unwrap_or(0);
            eprintln!(
                "Codex proxy: spawned sidecar pid={pid} script={} port={port}",
                script.display()
            );
            // Wait briefly for listen
            for _ in 0..20 {
                if port_open(port) {
                    eprintln!("Codex proxy: ready on 127.0.0.1:{port}");
                    return CodexProxyHandle {
                        child: Some(child),
                        port,
                    };
                }
                sleep(Duration::from_millis(100)).await;
            }
            eprintln!("Codex proxy: sidecar started but port {port} not open yet");
            CodexProxyHandle {
                child: Some(child),
                port,
            }
        }
        Err(e) => {
            eprintln!("Codex proxy: failed to spawn node: {e}");
            CodexProxyHandle { child: None, port }
        }
    }
}

fn port_open(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port))
        .map(|s| {
            let _ = s.shutdown(std::net::Shutdown::Both);
            true
        })
        .unwrap_or(false)
}

fn resolve_script_path() -> PathBuf {
    if let Ok(p) = std::env::var("LINLIS_CODEX_PROXY_SCRIPT") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return path;
        }
    }
    if let Ok(root) = std::env::var("LINLIS_ROOT") {
        let path = Path::new(&root).join("scripts/codex-deepseek-proxy.cjs");
        if path.is_file() {
            return path;
        }
    }
    // Deploy layout: workspace next to common host path
    let candidates = [
        PathBuf::from("/AI/LinlisWorkPanel/scripts/codex-deepseek-proxy.cjs"),
        PathBuf::from("scripts/codex-deepseek-proxy.cjs"),
        PathBuf::from("../scripts/codex-deepseek-proxy.cjs"),
    ];
    for c in candidates {
        if c.is_file() {
            return c;
        }
    }
    PathBuf::from("/AI/LinlisWorkPanel/scripts/codex-deepseek-proxy.cjs")
}

/// Pure helpers (shared with tests) — Responses `input` → chat messages.
pub fn extract_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|c| {
                let ty = c.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if ty == "input_text" || ty == "output_text" {
                    c.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub fn responses_to_chat(input: &serde_json::Value) -> Vec<serde_json::Value> {
    if input.is_null() {
        return vec![serde_json::json!({"role":"user","content":"hello"})];
    }
    if let Some(s) = input.as_str() {
        return vec![serde_json::json!({"role":"user","content":s})];
    }
    if let Some(arr) = input.as_array() {
        let mut messages = Vec::new();
        for item in arr {
            let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if ty == "message" {
                if let (Some(role), Some(content)) = (item.get("role"), item.get("content")) {
                    let mut role = role.as_str().unwrap_or("user").to_string();
                    if role == "developer" {
                        role = "system".into();
                    }
                    messages.push(serde_json::json!({
                        "role": role,
                        "content": extract_text(content),
                    }));
                }
            } else if ty == "input_item" {
                if let Some(content) = item.get("content") {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": extract_text(content),
                    }));
                }
            }
        }
        if messages.is_empty() {
            return vec![serde_json::json!({"role":"user","content":"hello"})];
        }
        return messages;
    }
    if let Some(inner) = input.get("input") {
        return responses_to_chat(inner);
    }
    vec![serde_json::json!({"role":"user","content":"hello"})]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_developer_to_system_and_extracts_text() {
        let input = json!([
            {
                "type": "message",
                "role": "developer",
                "content": [{"type": "input_text", "text": "sys"}]
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hi"}]
            }
        ]);
        let msgs = responses_to_chat(&input);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "sys");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[1]["content"], "hi");
    }

    #[test]
    fn string_input_becomes_user_message() {
        let msgs = responses_to_chat(&json!("ping"));
        assert_eq!(msgs[0]["content"], "ping");
    }
}
