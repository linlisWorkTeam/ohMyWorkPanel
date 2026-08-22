//! PanelLive short-reply prompt: fetch `/v1/llm-prompt` with cache + fallback.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Must match WorkPanelLive `docs/llm-prompt-panellive.md`.
pub const PANELLIVE_LLM_PROMPT_FALLBACK: &str = "【PanelLive 语音模式 · 强制】你的最终回复将送给 CosyVoice TTS。每次最终输出必须严格少于 50 个汉字（含标点按字计；勿超过 50）。只说结论与必要动作，禁止长文、列表堆砌、代码块、多段解释。若信息较多，只保留最关键一句，其余留到下一轮语音。";

const FETCH_TIMEOUT: Duration = Duration::from_millis(1500);
const CACHE_TTL: Duration = Duration::from_secs(60);

struct PromptCache {
    fetched_at: Instant,
    prompt: String,
}

static CACHE: Mutex<Option<PromptCache>> = Mutex::new(None);

/// Live session active + chatbot or group admin agent.
pub fn should_inject_live(
    live_active: bool,
    agent_kind: &str,
    agent_id: &str,
    admin_member_id: Option<&str>,
) -> bool {
    if !live_active {
        return false;
    }
    agent_kind == "chatbot" || admin_member_id == Some(agent_id)
}

pub fn parse_llm_prompt_response(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return v
            .get("prompt")
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
    }
    Some(trimmed.to_string())
}

fn http_get_prompt(host: &str, port: u16, path: &str) -> Result<(u16, String), String> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| format!("解析地址 {addr}：{e}"))?,
        FETCH_TIMEOUT,
    )
    .map_err(|e| format!("连接 {addr} 失败：{e}"))?;
    stream
        .set_read_timeout(Some(FETCH_TIMEOUT))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(FETCH_TIMEOUT))
        .map_err(|e| e.to_string())?;
    let req = format!("GET {path} HTTP/1.0\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let sep = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "llm-prompt 响应无头部结束标记".to_string())?;
    let header = String::from_utf8_lossy(&buf[..sep]);
    let status = header
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    Ok((
        status,
        String::from_utf8_lossy(&buf[sep + 4..]).into_owned(),
    ))
}

/// Resolve PanelLive port from manifest (default 8790).
pub fn resolve_panellive_port() -> u16 {
    let root = crate::extensions::panellive_root();
    crate::extensions::load_panellive_manifest(&root)
        .map(|m| crate::extensions::panellive_upstream_port(&m))
        .unwrap_or(8790)
}

/// Fetch prompt with 60s cache; on failure return fallback (never errors).
pub fn fetch_live_prompt(port: u16) -> String {
    if let Ok(guard) = CACHE.lock() {
        if let Some(c) = guard.as_ref() {
            if c.fetched_at.elapsed() < CACHE_TTL {
                return c.prompt.clone();
            }
        }
    }
    let prompt = match http_get_prompt("127.0.0.1", port, "/v1/llm-prompt") {
        Ok((200, body)) => parse_llm_prompt_response(&body)
            .unwrap_or_else(|| PANELLIVE_LLM_PROMPT_FALLBACK.to_string()),
        _ => PANELLIVE_LLM_PROMPT_FALLBACK.to_string(),
    };
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(PromptCache {
            fetched_at: Instant::now(),
            prompt: prompt.clone(),
        });
    }
    prompt
}

/// Clear cache (tests).
#[cfg(test)]
pub fn clear_live_prompt_cache() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }
}

/// Prompt suffix for injection, or empty when should not inject.
pub fn live_prompt_suffix(
    live_active: bool,
    agent_kind: &str,
    agent_id: &str,
    admin_member_id: Option<&str>,
) -> String {
    if !should_inject_live(live_active, agent_kind, agent_id, admin_member_id) {
        return String::new();
    }
    let prompt = fetch_live_prompt(resolve_panellive_port());
    format!("\n\n{prompt}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_inject_requires_live_and_role() {
        assert!(!should_inject_live(false, "chatbot", "c1", None));
        assert!(should_inject_live(true, "chatbot", "c1", None));
        assert!(should_inject_live(true, "agent", "a1", Some("a1")));
        assert!(!should_inject_live(true, "agent", "a2", Some("a1")));
    }

    #[test]
    fn parse_prompt_json_and_plain() {
        let json = r#"{"mode":"panellive","prompt":"短回复强制","ttsMaxChars":50}"#;
        assert_eq!(parse_llm_prompt_response(json).as_deref(), Some("短回复强制"));
        assert_eq!(
            parse_llm_prompt_response("  纯文本  ").as_deref(),
            Some("纯文本")
        );
        assert!(parse_llm_prompt_response(r#"{"ttsMaxChars":50}"#).is_none());
    }

    #[test]
    fn fetch_falls_back_on_dead_port() {
        clear_live_prompt_cache();
        // Unlikely bound: use closed port
        let p = fetch_live_prompt(1);
        assert!(p.contains("50") || p.contains("PanelLive"));
        // Cache hit returns same without needing port
        let p2 = fetch_live_prompt(1);
        assert_eq!(p, p2);
        clear_live_prompt_cache();
    }

    #[test]
    fn suffix_empty_when_inactive() {
        assert!(live_prompt_suffix(false, "chatbot", "c", None).is_empty());
    }
}
