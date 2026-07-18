mod claude;
mod codex;
mod cursor;
mod mock;
mod opencode;
pub mod parse;

use crate::db::AppResult;
use parse::parse_agent_line;
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

pub use mock::STREAM_CHUNKS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    Mock,
    Codex,
    ClaudeCode,
    OpenCode,
    Cursor,
}

impl AdapterKind {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "mock" => Ok(Self::Mock),
            "codex" => Ok(Self::Codex),
            "claude-code" => Ok(Self::ClaudeCode),
            "opencode" => Ok(Self::OpenCode),
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
            Self::Cursor => "cursor",
        }
    }

    pub fn candidate_executables(self) -> &'static [&'static str] {
        match self {
            Self::Mock => &[],
            Self::Codex => &["codex"],
            Self::ClaudeCode => &["claude"],
            Self::OpenCode => &["opencode"],
            Self::Cursor => cursor::candidate_executables(),
        }
    }

    pub fn default_executable(self) -> Option<&'static str> {
        self.candidate_executables().first().copied()
    }

    /// Resolve the executable to launch. Configured path wins; otherwise prefer the first
    /// candidate found on PATH, else the preferred default name (for detect/spawn errors).
    pub fn resolve_executable(self, configured: Option<&str>) -> Result<String, String> {
        if let Some(path) = configured.map(str::trim).filter(|s| !s.is_empty()) {
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

    pub fn build_args(self, prompt: &str) -> Vec<String> {
        match self {
            Self::Mock => Vec::new(),
            Self::Codex => codex::build_args(prompt),
            Self::ClaudeCode => claude::build_args(prompt),
            Self::OpenCode => opencode::build_args(prompt),
            Self::Cursor => cursor::build_args(prompt),
        }
    }

    pub fn parse_line(self, line: &str) -> String {
        parse_agent_line(line)
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

pub async fn run_mock_stream<F, Fut>(token: &Arc<AtomicBool>, mut on_delta: F) -> AppResult<()>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = AppResult<()>>,
{
    for chunk in STREAM_CHUNKS {
        if token.load(Ordering::SeqCst) {
            return Ok(());
        }
        on_delta((*chunk).to_string()).await?;
        sleep(Duration::from_millis(280)).await;
    }
    Ok(())
}

pub async fn run_streaming<F, Fut>(
    kind: AdapterKind,
    executable: &str,
    workspace: &Path,
    prompt: &str,
    timeout_secs: u64,
    token: &Arc<AtomicBool>,
    mut on_delta: F,
) -> AppResult<()>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = AppResult<()>>,
{
    let adapter = kind.as_str();
    let mut command = prepare_command(executable);
    command
       .current_dir(workspace)
       .args(kind.build_args(prompt))
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
    loop {
        tokio::select! {
            result = lines.next_line() => match result {
                Ok(Some(line)) => {
                    let output = kind.parse_line(&line);
                    if !output.is_empty() {
                        on_delta(output).await?;
                    }
                }
                Ok(None) => break,
                Err(error) => return Err(format!("读取 Agent 输出失败：{error}")),
            },
            _ = sleep(Duration::from_millis(200)) => {
                if token.load(Ordering::SeqCst) {
                    let _ = child.kill().await;
                    return Ok(());
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_match_cli_contracts() {
        assert_eq!(
            AdapterKind::Codex.build_args("do work"),
            vec!["exec", "--json", "--skip-git-repo-check", "do work"]
        );
        assert_eq!(
            AdapterKind::ClaudeCode.build_args("do work"),
            vec!["-p", "--output-format", "stream-json", "--verbose", "do work"]
        );
        assert_eq!(
            AdapterKind::OpenCode.build_args("do work"),
            vec!["run", "do work", "--format", "json"]
        );
        assert_eq!(
            AdapterKind::Cursor.build_args("do work"),
            vec!["-p", "do work", "--output-format", "stream-json"]
        );
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
