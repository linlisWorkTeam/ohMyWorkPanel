pub mod chatbot;
mod claude;
pub(crate) mod codex;
mod cursor;
mod dsh;
mod mock;
pub mod models;
mod openclaw;
mod opencode;
pub mod parse;
pub mod manifest;

use crate::db::AppResult;
use manifest::SpawnSpec;
use parse::{
    parse_agent_event, parse_agent_line, parse_openclaw_mixed_output, DeltaMode, ParsedEvent,
};
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
    Dsh,
}

/// 适配器能力描述（轻量注册表条目）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterDescriptor {
    pub key: &'static str,
    pub label: &'static str,
    pub needs_member_api_key: bool,
    pub spawnable: bool,
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
            "dsh" => Ok(Self::Dsh),
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
            Self::Dsh => "dsh",
        }
    }

    /// 全部适配器（轻量注册表：新增适配器只需在此登记并在 `descriptor`/`parse` 补一行）。
    pub const ALL: [AdapterKind; 7] = [
        Self::Mock,
        Self::Codex,
        Self::ClaudeCode,
        Self::OpenCode,
        Self::OpenClaw,
        Self::Cursor,
        Self::Dsh,
    ];

    /// 能力描述（key / 展示名 / 是否需要成员级 API key / 是否可 spawn）。
    pub fn descriptor(self) -> AdapterDescriptor {
        let label = match self {
            Self::Mock => "模拟 Agent",
            Self::Codex => "Codex CLI",
            Self::ClaudeCode => "Claude Code",
            Self::OpenCode => "OpenCode",
            Self::OpenClaw => "OpenClaw",
            Self::Cursor => "Cursor CLI",
            Self::Dsh => "DeepSeek Harness",
        };
        AdapterDescriptor {
            key: self.as_str(),
            label,
            needs_member_api_key: false, // 当前 7 个 CLI 适配器均由自身 env/auth 文件取密钥
            spawnable: !self.candidate_executables().is_empty(),
        }
    }

    pub fn candidate_executables(self) -> &'static [&'static str] {
        match self {
            Self::Mock => &[],
            Self::Codex => &["codex"],
            Self::ClaudeCode => &["claude"],
            Self::OpenCode => &["opencode"],
            Self::OpenClaw => openclaw::candidate_executables(),
            Self::Dsh => dsh::candidate_executables(),
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
            Self::Dsh => dsh::build_args(prompt, session_id, model),
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

pub(crate) fn find_executable_path(name: &str) -> Option<String> {
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

/// Public wrapper for agent-config / provisioning: whether a CLI executable (or a Windows
/// shim extension of it) is findable on PATH (incl. npm global on Windows).
pub fn find_executable_on_path(name: &str) -> Option<String> {
    find_executable_path(name)
        .or_else(|| {
            #[cfg(windows)]
            {
                find_in_npm(name)
            }
            #[cfg(not(windows))]
            {
                None
            }
        })
}

#[cfg(windows)]
pub(crate) fn find_in_npm(name: &str) -> Option<String> {
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
/// `api_key`: when set for Codex, exported as `OPENAI_API_KEY` (DeepSeek OpenAI-compatible auth).
pub async fn run_streaming<F, Fut>(
    spec: SpawnSpec,
    executable: &str,
    workspace: &Path,
    prompt: &str,
    session_id: Option<&str>,
    model: Option<&str>,
    timeout_secs: u64,
    token: &Arc<AtomicBool>,
    api_key: Option<&str>,
    mut on_delta: F,
) -> AppResult<Option<String>>
where
    F: FnMut(String, String, bool) -> Fut,
    Fut: std::future::Future<Output = AppResult<()>>,
{
    let adapter = spec.id().to_string();
    let kind = spec.builtin_kind();
    if workspace == Path::new("/") || !workspace.exists() {
        return Err(format!(
            "工作目录无效：{}。请把群工作目录设为具体项目路径（不能是 /）。",
            workspace.display()
        ));
    }
    let mut command = prepare_command(executable);
    command
        .current_dir(workspace)
        .args(spec.build_args(prompt, session_id, model))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if kind == Some(AdapterKind::Codex) {
        // systemd units usually omit shell OPENAI_API_KEY; fall back to ~/.codex/auth.json
        // (same key OpenCode Zen Go / local Codex CLI already use).
        match codex::resolve_api_key(api_key) {
            Some(key) => {
                command.env("OPENAI_API_KEY", key);
            }
            None => {
                return Err(
                    "Codex 缺少 OPENAI_API_KEY：请在成员 API Key、环境变量 LINLIS_CODEX_API_KEY/OPENAI_API_KEY，或 ~/.codex/auth.json 中配置（OpenCode Go 所用密钥）。"
                        .into(),
                );
            }
        }
    }

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
    let mut openclaw_got_final = false;
    let mut last_failure_hint: Option<String> = None;
    loop {
        tokio::select! {
            result = lines.next_line() => match result {
                Ok(Some(line)) => {
                    if kind == Some(AdapterKind::OpenClaw) {
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
                        let event = spec.parse_event(trimmed);
                        openclaw_buf.clear();
                        if let Some(id) = event.session_id {
                            captured_session = Some(id);
                        }
                        if !event.text.is_empty() {
                            let replace = event.mode == DeltaMode::Replace;
                            if event.channel == "final" {
                                openclaw_got_final = true;
                            }
                            on_delta(event.channel, event.text, replace).await?;
                        }
                    } else {
                        if kind == Some(AdapterKind::Codex) {
                            if let Some(hint) = codex_failure_hint(&line) {
                                last_failure_hint = Some(hint);
                            }
                        }
                        let event = spec.parse_event(&line);
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
                    if kind == Some(AdapterKind::OpenClaw) {
                        let trimmed = openclaw_buf.trim();
                        if !trimmed.is_empty() {
                            let event = spec.parse_event(trimmed);
                            if let Some(id) = event.session_id {
                                captured_session = Some(id);
                            }
                            if !event.text.is_empty() {
                                let replace = event.mode == DeltaMode::Replace;
                                if event.channel == "final" {
                                    openclaw_got_final = true;
                                }
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
    // OpenClaw 2026.3 often prints the JSON envelope on stderr (stdout empty),
    // especially after gateway→embedded fallback. Recover payloads[].text.
    if kind == Some(AdapterKind::OpenClaw) && !openclaw_got_final {
        if let Some(event) = parse_openclaw_mixed_output(&stderr) {
            if let Some(id) = event.session_id.clone() {
                captured_session = Some(id);
            }
            if !event.text.is_empty() {
                let replace = event.mode == DeltaMode::Replace;
                on_delta(event.channel, event.text, replace).await?;
                openclaw_got_final = true;
            }
        }
    }
    if !status.success() {
        // Prefer a clean payload over dumping gateway noise + raw JSON.
        if kind == Some(AdapterKind::OpenClaw) && openclaw_got_final {
            return Ok(captured_session);
        }
        return Err(format_cli_failure(&adapter, &status.to_string(), &stderr, last_failure_hint.as_deref()));
    }
    if kind == Some(AdapterKind::OpenClaw) && !openclaw_got_final {
        return Err(format!(
            "{adapter} 未解析到回复（stdout 空且 stderr 无 payloads）。stderr 摘要：{}",
            stderr.chars().take(280).collect::<String>()
        ));
    }
    Ok(captured_session)
}

/// Fold adapter stdout/stderr into the user-visible **final** text.
/// Used by contract tests (and mirrors OpenClaw stderr recovery in `run_streaming`).
pub fn resolve_adapter_final_text(kind: AdapterKind, stdout: &str, stderr: &str) -> Option<String> {
    let mut final_text = String::new();
    let mut got_final = false;

    if kind == AdapterKind::OpenClaw {
        let trimmed = stdout.trim();
        if !trimmed.is_empty() {
            if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
                let event = kind.parse_event(trimmed);
                if event.channel == "final" && !event.text.is_empty() {
                    final_text = event.text;
                    got_final = true;
                }
            } else if let Some(event) = parse_openclaw_mixed_output(stdout) {
                if !event.text.is_empty() {
                    final_text = event.text;
                    got_final = true;
                }
            }
        }
        if !got_final {
            if let Some(event) = parse_openclaw_mixed_output(stderr) {
                if !event.text.is_empty() {
                    final_text = event.text;
                    got_final = true;
                }
            }
        }
        return got_final.then_some(final_text);
    }

    for line in stdout.lines() {
        let event = kind.parse_event(line);
        if event.channel != "final" || event.text.is_empty() {
            continue;
        }
        got_final = true;
        match event.mode {
            DeltaMode::Replace => final_text = event.text,
            DeltaMode::Append => final_text.push_str(&event.text),
        }
    }
    got_final.then_some(final_text)
}

fn codex_failure_hint(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let ty = value.get("type")?.as_str()?.to_ascii_lowercase();
    if ty != "error" && ty != "turn.failed" {
        return None;
    }
    value
        .pointer("/error/message")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("message").and_then(|v| v.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn format_cli_failure(adapter: &str, status: &str, stderr: &str, hint: Option<&str>) -> String {
    let cleaned = stderr
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.eq_ignore_ascii_case("Reading additional input from stdin...")
                && !line.starts_with("thread 'main'")
                && !line.starts_with("note: run with")
                && !line.starts_with("failed printing to stdout")
        })
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(h) = hint.filter(|s| !s.is_empty()) {
        if cleaned.is_empty() {
            return format!("{adapter} 异常退出：{h}");
        }
        return format!("{adapter} 异常退出：{h}\n{cleaned}");
    }
    if cleaned.is_empty() {
        format!("{adapter} 异常退出（{status}）。")
    } else {
        format!("{adapter} 异常退出：{cleaned}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_final_response_contracts() {
        // Acceptance: each adapter fixture must yield the expected user-visible token,
        // not envelopes / gateway noise / empty 「已完成。」 placeholders.
        let cases: &[(AdapterKind, &str, &str, &str)] = &[
            (
                AdapterKind::OpenClaw,
                "",
                r#"Gateway agent failed; falling back to embedded: Error: gateway closed (1006)
{
  "payloads": [{ "text": "PONG_OPENCLAW", "mediaUrl": null }],
  "meta": { "agentMeta": { "sessionId": "s-oc" } }
}
"#,
                "PONG_OPENCLAW",
            ),
            (
                AdapterKind::OpenClaw,
                r#"{"runId":"r1","status":"ok","result":{"payloads":[{"text":"PONG_OPENCLAW_STDOUT"}]}}"#,
                "ignored stderr noise",
                "PONG_OPENCLAW_STDOUT",
            ),
            (
                AdapterKind::Cursor,
                r#"{"type":"system","session_id":"c1"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"PONG_CURSOR"}]},"session_id":"c1"}
{"type":"result","subtype":"success","result":"PONG_CURSOR","session_id":"c1"}"#,
                "",
                "PONG_CURSOR",
            ),
            (
                AdapterKind::Codex,
                r#"{"type":"thread.started","thread_id":"t1"}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"PONG_CODEX"}}
{"type":"turn.completed"}"#,
                "",
                "PONG_CODEX",
            ),
            (
                AdapterKind::ClaudeCode,
                r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"PONG_CLAUDE"}]},"session_id":"cl1"}
{"type":"result","subtype":"success","result":"PONG_CLAUDE","session_id":"cl1"}"#,
                "",
                "PONG_CLAUDE",
            ),
            (
                AdapterKind::OpenCode,
                r#"{"type":"text","text":"PONG_OPENCODE"}"#,
                "",
                "PONG_OPENCODE",
            ),
            (
                AdapterKind::Dsh,
                "PONG_DSH\n",
                "",
                "PONG_DSH",
            ),
        ];

        for (kind, stdout, stderr, expected) in cases {
            let got = resolve_adapter_final_text(*kind, stdout, stderr)
                .unwrap_or_else(|| panic!("{kind:?}: expected final text, got None"));
            assert_eq!(got, *expected, "{kind:?} final text mismatch");
            assert!(
                !got.contains("\"runId\"") && !got.contains("\"payloads\""),
                "{kind:?} must not leak envelope JSON"
            );
            assert!(
                !got.contains("Gateway agent failed"),
                "{kind:?} must not leak gateway stderr"
            );
        }
    }

    #[test]
    fn openclaw_empty_streams_yield_no_final() {
        assert!(resolve_adapter_final_text(AdapterKind::OpenClaw, "", "gateway only, no json").is_none());
    }

    #[test]
    fn build_args_match_cli_contracts() {
        assert_eq!(
            AdapterKind::Codex.build_args("do work", None, None),
            vec![
                "exec",
                "--json",
                "--skip-git-repo-check",
                "-c",
                "model_provider=\"deepseek\"",
                "-c",
                "model_providers.deepseek.base_url=\"http://127.0.0.1:18888/v1\"",
                "-c",
                "model_providers.deepseek.env_key=\"OPENAI_API_KEY\"",
                "-c",
                "model_providers.deepseek.name=\"deepseek\"",
                "-m",
                "deepseek-v4-flash",
                "do work"
            ]
        );
        assert_eq!(
            AdapterKind::Codex.build_args("do work", None, Some("o3"))
                .into_iter()
                .skip_while(|a| a != "-m")
                .take(2)
                .collect::<Vec<_>>(),
            vec!["-m", "deepseek-v4-flash"]
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
        assert_eq!(
            AdapterKind::Dsh.build_args("do work", None, None),
            vec!["--profile", "headless", "do work"]
        );
        assert_eq!(
            AdapterKind::Dsh.build_args("do work", Some("sess-1"), Some("gpt-5")),
            vec!["--profile", "headless", "do work"]
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

    #[test]
    fn format_cli_failure_prefers_json_hint_over_stdin_noise() {
        let msg = format_cli_failure(
            "codex",
            "exit status: 1",
            "Reading additional input from stdin...\n",
            Some("unexpected status 401 Unauthorized"),
        );
        assert!(msg.contains("401"));
        assert!(!msg.contains("Reading additional input"));
    }

    #[test]
    fn adapter_registry_all_parse_back_and_describe() {
        // 轻量注册表：ALL 全量、parse/as_str 双射、descriptor 非空。
        assert_eq!(AdapterKind::ALL.len(), 7, "新增适配器需同步 ALL/parse/descriptor");
        let mut seen = std::collections::HashSet::new();
        for kind in AdapterKind::ALL {
            let d = kind.descriptor();
            assert_eq!(d.key, kind.as_str());
            assert!(!d.label.is_empty());
            assert!(seen.insert(d.key), "注册表 key 重复: {}", d.key);
            assert_eq!(AdapterKind::parse(d.key).unwrap(), kind);
            assert_eq!(d.spawnable, !kind.candidate_executables().is_empty());
            assert!(!d.needs_member_api_key, "{} 不应需要成员级 key（当前 CLI 适配器均自取密钥）", d.key);
        }
    }
}
