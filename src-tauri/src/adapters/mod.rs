pub mod chatbot;
mod claude;
mod codex;
mod cursor;
mod mock;
pub mod models;
mod openclaw;
mod opencode;
pub mod parse;

use crate::db::AppResult;
use parse::{parse_agent_event, parse_agent_line, DeltaMode, ParsedEvent};
use std::{
    path::Path,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    time::sleep,
};

pub use mock::STREAM_EVENTS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    Mock,
    Codex,
    ClaudeCode,
    OpenCode,
    OpenClaw,
    Cursor,
}

impl AdapterKind {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "mock" => Ok(Self::Mock),
            "codex" => Ok(Self::Codex),
            "claude-code" => Ok(Self::ClaudeCode),
            "opencode" => Ok(Self::OpenCode),
            "openclaw" => Ok(Self::OpenClaw),
            "cursor" => Ok(Self::Cursor),
            other => Err(format!("不支持的 Agent 适配器：{other}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
            Self::OpenCode => "opencode",
            Self::OpenClaw => "openclaw",
            Self::Cursor => "cursor",
        }
    }

    pub fn candidate_executables(self) -> &'static [&'static str] {
        match self {
            Self::Mock => &[],
            Self::Codex => &["codex"],
            Self::ClaudeCode => &["claude"],
            Self::OpenCode => &["opencode"],
            Self::OpenClaw => openclaw::candidate_executables(),
            Self::Cursor => cursor::candidate_executables(),
        }
    }

    pub fn default_executable(self) -> Option<&'static str> {
        self.candidate_executables().first().copied()
    }

    /// Resolve the executable to launch. Configured path wins; otherwise prefer the first
    /// candidate found on PATH, else the preferred default name (for detect/spawn errors).
    pub fn resolve_executable(self, configured: Option<&str>) -> Result<String, String> {
        // Gateway URLs are not spawnable binaries (common OpenClaw misconfig).
        if let Some(path) = configured
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter(|s| !s.starts_with("http://") && !s.starts_with("https://"))
        {
            return Ok(path.to_string());
        }
        let candidates = self.candidate_executables();
        if candidates.is_empty() {
            return Err("此适配器暂未提供运行器。".into());
        }
        for name in candidates {
            if let Some(full_path) = find_executable_path(name) {
                return Ok(full_path);
            }
            #[cfg(windows)]
            {
                if let Some(npm_path) = find_in_npm(name) {
                    return Ok(npm_path);
                }
            }
        }
        Ok(self
            .default_executable()
            .unwrap_or(candidates[0])
            .to_string())
    }

    pub fn build_args(
        self,
        prompt: &str,
        session_id: Option<&str>,
        model: Option<&str>,
    ) -> Vec<String> {
        match self {
            Self::Mock => Vec::new(),
            Self::Codex => codex::build_args(prompt, model),
            Self::ClaudeCode => claude::build_args(prompt, model),
            Self::OpenCode => opencode::build_args(prompt, model),
            Self::OpenClaw => openclaw::build_args(prompt, session_id, model),
            Self::Cursor => cursor::build_args(prompt, session_id, model),
        }
    }

    pub fn parse_line(self, line: &str) -> String {
        parse_agent_line(line)
    }

    pub fn parse_event(self, line: &str) -> ParsedEvent {
        let _ = self;
        parse_agent_event(line)
    }
}

fn candidate_exists(path: &Path) -> bool {
    path.is_file()
}

fn find_executable_path(name: &str) -> Option<String> {
    std::env::var("PATH").ok().and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            #[cfg(windows)]
            {
                // Windows: only match files with executable extensions.
                // Bare names (e.g. Unix scripts without extension) will fail
                // at CreateProcess with "not a valid Win32 application".
                for ext in ["exe", "cmd", "bat", "ps1"] {
                    let full = dir.join(format!("{name}.{ext}"));
                    if candidate_exists(&full) {
                        return Some(full.to_string_lossy().to_string());
                    }
                }
            }
            #[cfg(not(windows))]
            {
                if candidate_exists(&dir.join(name)) {
                    return Some(dir.join(name).to_string_lossy().to_string());
                }
            }
        }
        None
    })
}

#[cfg(windows)]
fn find_in_npm(name: &str) -> Option<String> {
    let appdata = std::env::var("APPDATA").ok()?;
    let npm_dir = std::path::Path::new(&appdata).join("npm");
    for ext in ["cmd", "exe", "bat", "ps1"] {
        let full = npm_dir.join(format!("{name}.{ext}"));
        if candidate_exists(&full) {
            return Some(full.to_string_lossy().to_string());
        }
    }
    None
}

/// Windows helper: wraps .cmd/.bat through cmd.exe, .ps1 through powershell.exe,
/// so scripts that aren't valid PE executables still work.
#[cfg(windows)]
fn prepare_command(executable: &str) -> Command {
    let lower = executable.to_lowercase();
    if lower.ends_with(".ps1") {
        let mut cmd = Command::new("powershell.exe");
        cmd.arg("-ExecutionPolicy").arg("Bypass").arg("-File").arg(executable);
        cmd
    } else if lower.ends_with(".cmd") || lower.ends_with(".bat") {
        let mut cmd = Command::new("cmd.exe");
        cmd.arg("/c").arg(executable);
        cmd
    } else {
        Command::new(executable)
    }
}

#[cfg(not(windows))]
fn prepare_command(executable: &str) -> Command {
    Command::new(executable)
}

pub async fn run_mock_stream<F, Fut>(token: &Arc<AtomicBool>, mut on_delta: F) -> AppResult<Option<String>>
where
    F: FnMut(String, String, bool) -> Fut,
    Fut: std::future::Future<Output = AppResult<()>>,
{
    for (channel, chunk) in STREAM_EVENTS {
        if token.load(Ordering::SeqCst) {
            return Ok(None);
        }
        on_delta((*channel).to_string(), (*chunk).to_string(), false).await?;
        sleep(Duration::from_millis(280)).await;
    }
    Ok(None)
}

/// Run adapter process. `on_delta(channel, text, replace)`. Returns captured CLI session id if any.
pub async fn run_streaming<F, Fut>(
    kind: AdapterKind,
    executable: &str,
    workspace: &Path,
    prompt: &str,
    session_id: Option<&str>,
    model: Option<&str>,
    timeout_secs: u64,
    token: &Arc<AtomicBool>,
    mut on_delta: F,
) -> AppResult<Option<String>>
where
    F: FnMut(String, String, bool) -> Fut,
    Fut: std::future::Future<Output = AppResult<()>>,
{
    let adapter = kind.as_str();
    if workspace == Path::new("/") || !workspace.exists() {
        return Err(format!(
            "工作目录无效：{}。请把群工作目录设为具体项目路径（不能是 /）。",
            workspace.display()
        ));
    }
    let mut command = prepare_command(executable);
    command
        .current_dir(workspace)
        .args(kind.build_args(prompt, session_id, model))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("无法启动 {adapter}：{e}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 Agent 输出。".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取 Agent 诊断。".to_string())?;
    let stderr_task = tokio::spawn(async move {
        let mut result = String::new();
        let _ = stderr.read_to_string(&mut result).await;
        result
    });
    let mut lines = BufReader::new(stdout).lines();
    let started = Instant::now();
    let mut captured_session: Option<String> = None;
    // OpenClaw `--json` often emits one pretty-printed object across many lines.
    // Buffer until a full JSON value parses; never echo raw fragments into chat.
    let mut openclaw_buf = String::new();
    loop {
        tokio::select! {
            result = lines.next_line() => match result {
                Ok(Some(line)) => {
                    if kind == AdapterKind::OpenClaw {
                        if !openclaw_buf.is_empty() {
                            openclaw_buf.push('\n');
                        }
                        openclaw_buf.push_str(&line);
                        let trimmed = openclaw_buf.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if serde_json::from_str::<serde_json::Value>(trimmed).is_err() {
                            continue;
                        }
                        let event = kind.parse_event(trimmed);
                        openclaw_buf.clear();
                        if let Some(id) = event.session_id {
                            captured_session = Some(id);
                        }
                        if !event.text.is_empty() {
                            let replace = event.mode == DeltaMode::Replace;
                            on_delta(event.channel, event.text, replace).await?;
                        }
                    } else {
                        let event = kind.parse_event(&line);
                        if let Some(id) = event.session_id {
                            captured_session = Some(id);
                        }
                        if !event.text.is_empty() {
                            let replace = event.mode == DeltaMode::Replace;
                            on_delta(event.channel, event.text, replace).await?;
                        }
                    }
                }
                Ok(None) => {
                    if kind == AdapterKind::OpenClaw {
                        let trimmed = openclaw_buf.trim();
                        if !trimmed.is_empty() {
                            let event = kind.parse_event(trimmed);
                            if let Some(id) = event.session_id {
                                captured_session = Some(id);
                            }
                            if !event.text.is_empty() {
                                let replace = event.mode == DeltaMode::Replace;
                                on_delta(event.channel, event.text, replace).await?;
                            }
                        }
                    }
                    break;
                }
                Err(error) => return Err(format!("读取 Agent 输出失败：{error}")),
            },
            _ = sleep(Duration::from_millis(200)) => {
                if token.load(Ordering::SeqCst) {
                    let _ = child.kill().await;
                    return Ok(captured_session);
                }
                if started.elapsed() > Duration::from_secs(timeout_secs) {
                    let _ = child.kill().await;
                    return Err("Agent 任务超时，已停止。".into());
                }
            }
        }
    }
    let status = child
        .wait()
        .await
        .map_err(|e| format!("等待 Agent 结束失败：{e}"))?;
    let stderr = stderr_task.await.unwrap_or_default();
    if !status.success() {
        return Err(if stderr.trim().is_empty() {
            format!("{adapter} 异常退出（{status}）。")
        } else {
            format!("{adapter} 异常退出：{}", stderr.trim())
        });
    }
    Ok(captured_session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_match_cli_contracts() {
        assert_eq!(
            AdapterKind::Codex.build_args("do work", None, None),
            vec!["exec", "--json", "--skip-git-repo-check", "do work"]
        );
        assert_eq!(
            AdapterKind::Codex.build_args("do work", None, Some("o3")),
            vec!["exec", "--json", "--skip-git-repo-check", "-m", "o3", "do work"]
        );
        assert_eq!(
            AdapterKind::ClaudeCode.build_args("do work", None, None),
            vec!["-p", "--output-format", "stream-json", "--verbose", "do work"]
        );
        assert_eq!(
            AdapterKind::OpenCode.build_args("do work", None, None),
            vec!["run", "do work", "--format", "json"]
        );
        assert_eq!(
            AdapterKind::Cursor.build_args("do work", None, None),
            vec![
                "--trust",
                "-p",
                "do work",
                "--output-format",
                "stream-json",
                "--stream-partial-output"
            ]
        );
        assert_eq!(
            AdapterKind::Cursor.build_args("do work", Some("chat-abc"), Some("gpt-5")),
            vec![
                "--trust",
                "--resume",
                "chat-abc",
                "--model",
                "gpt-5",
                "-p",
                "do work",
                "--output-format",
                "stream-json",
                "--stream-partial-output"
            ]
        );
        assert_eq!(
            AdapterKind::OpenClaw.build_args("do work", None, None),
            vec![
                "agent",
                "--agent",
                "main",
                "--message",
                "do work",
                "--json"
            ]
        );
        assert_eq!(
            AdapterKind::OpenClaw.build_args("do work", Some("sess-1"), None),
            vec![
                "agent",
                "--session-id",
                "sess-1",
                "--message",
                "do work",
                "--json"
            ]
        );
    }

    #[test]
    fn openclaw_ignores_http_executable_path() {
        let resolved = AdapterKind::OpenClaw
            .resolve_executable(Some("http://localhost:18789"))
            .expect("resolve");
        assert!(!resolved.starts_with("http"));
        assert!(resolved.contains("openclaw") || resolved == "openclaw");
    }

    #[test]
    fn cursor_prefers_configured_path() {
        let resolved = AdapterKind::Cursor
            .resolve_executable(Some("C:\\tools\\agent.exe"))
            .unwrap();
        assert_eq!(resolved, "C:\\tools\\agent.exe");
    }

    #[test]
    fn cursor_candidate_order_is_agent_then_cursor_agent() {
        assert_eq!(
            AdapterKind::Cursor.candidate_executables(),
            &["agent", "cursor-agent"]
        );
    }

    #[test]
    fn resolve_falls_back_to_preferred_name_when_missing() {
        // Use a unique fake PATH so neither candidate is found.
        let original = std::env::var_os("PATH");
        std::env::set_var("PATH", "");
        let resolved = AdapterKind::Cursor.resolve_executable(None).unwrap();
        assert_eq!(resolved, "agent");
        match original {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }

    #[test]
    fn parse_rejects_unknown_adapter() {
        assert!(AdapterKind::parse("unknown").is_err());
    }
}
