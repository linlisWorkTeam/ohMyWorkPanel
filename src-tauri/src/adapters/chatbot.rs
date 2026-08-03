use crate::db::AppResult;
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::process::Command;

pub const CHATBOT_MODEL: &str = "deepseek-v4-flash";

pub fn provider_base_url(provider: &str) -> Result<&'static str, String> {
    match provider {
        "opencode-go" | "chatbot-opencode-go" => Ok("https://opencode.ai/zen/go/v1"),
        "deepseek" | "chatbot-deepseek" => Ok("https://api.deepseek.com/v1"),
        other => Err(format!("不支持的 chatbot 提供方：{other}")),
    }
}

pub fn is_chatbot_adapter(adapter: &str) -> bool {
    matches!(
        adapter,
        "chatbot-opencode-go" | "chatbot-deepseek" | "opencode-go" | "deepseek"
    )
}

pub fn normalize_adapter(provider: &str) -> Result<&'static str, String> {
    match provider {
        "opencode-go" | "chatbot-opencode-go" => Ok("chatbot-opencode-go"),
        "deepseek" | "chatbot-deepseek" => Ok("chatbot-deepseek"),
        other => Err(format!("不支持的 chatbot 提供方：{other}")),
    }
}

/// Non-streaming chat via system `curl` (avoids heavy HTTP crate compile/OOM on 2GB hosts).
pub async fn run_chatbot_completion(
    provider: &str,
    api_key: &str,
    system: &str,
    user: &str,
    max_tokens: u32,
    token: &Arc<AtomicBool>,
    model: Option<&str>,
) -> AppResult<String> {
    if token.load(Ordering::SeqCst) {
        return Err("已取消".into());
    }
    let base = provider_base_url(provider)?;
    let url = format!("{base}/chat/completions");
    let model_id = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(CHATBOT_MODEL);
    let body = json!({
        "model": model_id,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "max_tokens": max_tokens,
        "temperature": 0.4,
        "stream": false
    });
    let body_str = body.to_string();
    let output = Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "15",
            "-H",
            &format!("Authorization: Bearer {}", api_key.trim()),
            "-H",
            "Content-Type: application/json",
            "-d",
            &body_str,
            &url,
        ])
        .output()
        .await
        .map_err(|e| format!("chatbot curl 启动失败：{e}"))?;
    if token.load(Ordering::SeqCst) {
        return Err("已取消".into());
    }
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "chatbot curl 失败：{}",
            truncate(err.trim(), 300)
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("chatbot 响应解析失败：{e}; body={}", truncate(&text, 200)))?;
    if let Some(msg) = value.pointer("/error/message").and_then(|v| v.as_str()) {
        return Err(format!("chatbot API 错误：{}", truncate(msg, 300)));
    }
    let content = value
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if content.is_empty() {
        return Err("chatbot 返回空内容".into());
    }
    Ok(content)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_maps_providers() {
        assert_eq!(normalize_adapter("opencode-go").unwrap(), "chatbot-opencode-go");
        assert_eq!(normalize_adapter("deepseek").unwrap(), "chatbot-deepseek");
        assert_eq!(normalize_adapter("chatbot-deepseek").unwrap(), "chatbot-deepseek");
        assert!(normalize_adapter("openai").is_err());
    }

    #[test]
    fn provider_urls() {
        assert!(provider_base_url("chatbot-opencode-go").unwrap().contains("opencode.ai"));
        assert!(provider_base_url("chatbot-deepseek").unwrap().contains("deepseek.com"));
    }
}
