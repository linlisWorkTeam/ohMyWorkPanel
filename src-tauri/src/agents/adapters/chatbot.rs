use crate::db::AppResult;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::process::Command;

/// 官方 API 的安全默认模型（deepseek 官方 / opencode zen 网关均可用）。
/// 历史遗留 `deepseek-v4-flash` 在两个官方 API 上不存在（400），统一归一到这里。
pub const CHATBOT_MODEL: &str = "deepseek-chat";

pub fn provider_base_url(provider: &str) -> Result<&'static str, String> {
    match provider {
        "opencode-go" | "chatbot-opencode-go" => Ok("https://opencode.ai/zen/go/v1"),
        "deepseek" | "chatbot-deepseek" => Ok("https://api.deepseek.com/v1"),
        other => Err(format!("不支持的 chatbot 提供方：{other}")),
    }
}

/// 最终 base URL：成员自定义 api_url 优先（provider=custom 时必须提供），否则官方地址。
pub fn resolve_base_url(provider: &str, api_url: Option<&str>) -> Result<String, String> {
    if let Some(url) = api_url.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(url.trim_end_matches('/').to_string());
    }
    if matches!(provider, "custom" | "chatbot-custom") {
        return Err("自定义 chatbot 必须填写 API 地址（apiUrl）。".into());
    }
    provider_base_url(provider).map(str::to_string)
}

pub fn is_chatbot_adapter(adapter: &str) -> bool {
    matches!(
        adapter,
        "chatbot-opencode-go" | "chatbot-deepseek" | "chatbot-custom" | "opencode-go" | "deepseek" | "custom"
    )
}

pub fn normalize_adapter(provider: &str) -> Result<&'static str, String> {
    match provider {
        "opencode-go" | "chatbot-opencode-go" => Ok("chatbot-opencode-go"),
        "deepseek" | "chatbot-deepseek" => Ok("chatbot-deepseek"),
        "custom" | "chatbot-custom" => Ok("chatbot-custom"),
        other => Err(format!("不支持的 chatbot 提供方：{other}")),
    }
}

/// 每 provider 的模型归一：空 → 默认；`deepseek-v4-flash`（旧默认，官方 API 不存在）→ 默认。
pub fn default_model_for(provider: &str, model: Option<&str>) -> String {
    let m = model.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("");
    let _ = provider; // 目前所有 provider 统一用 deepseek-chat 作为安全默认
    if m.is_empty() || m == "deepseek-v4-flash" {
        return CHATBOT_MODEL.to_string();
    }
    m.to_string()
}

/// 把滚动窗口文本（"名字: 内容" 行）解析成多轮消息：
/// 机器人自己的行 → assistant，其他成员 → user；「历史摘要」块作为开头的 user 轮；
/// 末尾附当前 root 作为最终 user 消息。仅明确匹配机器人显示名时标记 assistant。
pub fn build_chat_messages(
    system: &str,
    window: &str,
    bot_display_name: &str,
    root: &str,
) -> Vec<Value> {
    let mut turns: Vec<(String, String)> = Vec::new(); // (role, content)

    // 摘出「最近群聊」标记之前的历史摘要块（如有）
    let (summary_block, lines_part) = match window.rfind("最近群聊") {
        Some(i) => {
            let before = window[..i].trim();
            let after = window[i..].lines().skip(1).collect::<Vec<_>>().join("\n");
            (if before.is_empty() { None } else { Some(before.to_string()) }, after)
        }
        None => (None, window.to_string()),
    };

    // 先解析历史行（同角色相邻合并），再在最前面插入摘要轮，避免被合并掉。
    for line in lines_part.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (name, text) = match line.split_once(':') {
            Some((n, t)) if !n.trim().is_empty() && !t.trim().is_empty() => {
                (n.trim().to_string(), t.trim().to_string())
            }
            _ => continue,
        };
        let role = if name == bot_display_name {
            "assistant"
        } else {
            "user"
        };
        if let Some(last) = turns.last_mut() {
            if last.0 == role {
                last.1.push('\n');
                last.1.push_str(&text);
                continue;
            }
        }
        turns.push((role.to_string(), text));
    }

    if let Some(summary) = summary_block {
        turns.insert(0, ("user".to_string(), format!("（历史摘要）\n{summary}")));
    }

    let mut messages = vec![json!({ "role": "system", "content": system })];
    for (role, content) in turns {
        messages.push(json!({ "role": role, "content": content }));
    }
    let root_text = root.trim();
    if !root_text.is_empty() {
        messages.push(json!({ "role": "user", "content": root_text }));
    }
    messages
}

/// 请求候选 URL：主地址 + （未以 /v1 结尾时）自动补 /v1 回退。
pub fn completion_urls(base: &str) -> Vec<String> {
    let trim = base.trim_end_matches('/');
    let mut urls = vec![format!("{trim}/chat/completions")];
    if !trim.ends_with("/v1") && !trim.contains("/v1/") {
        urls.push(format!("{trim}/v1/chat/completions"));
    }
    urls
}

/// Non-streaming chat via system `curl` (avoids heavy HTTP crate compile/OOM on 2GB hosts).
/// `messages` 为完整对话数组（含 system），实现真正的多轮对话。
pub async fn run_chatbot_completion(
    provider: &str,
    api_url: Option<&str>,
    api_key: &str,
    messages: &[Value],
    max_tokens: u32,
    token: &Arc<AtomicBool>,
    model: Option<&str>,
) -> AppResult<String> {
    if token.load(Ordering::SeqCst) {
        return Err("已取消".into());
    }
    let base = resolve_base_url(provider, api_url)?;
    let candidates = completion_urls(&base);
    let model_id = default_model_for(provider, model);
    let body = json!({
        "model": model_id,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": 0.4,
        "stream": false
    });
    let body_str = body.to_string();
    let mut html_seen: Vec<String> = Vec::new();
    let mut last_html: String = String::new();
    for url in &candidates {
        if token.load(Ordering::SeqCst) {
            return Err("已取消".into());
        }
        let output = Command::new("curl")
            .args([
                "-sS",
                "--max-time",
                "30",
                "-H",
                &format!("Authorization: Bearer {}", api_key.trim()),
                "-H",
                "Content-Type: application/json",
                "-d",
                &body_str,
                url,
            ])
            .output()
            .await
            .map_err(|e| format!("chatbot curl 启动失败：{e}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("chatbot curl 失败（{url}）：{}", truncate(err.trim(), 300)));
        }
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        let head = text.trim_start();
        if head.starts_with("<!DOCTYPE") || head.starts_with("<html") || head.starts_with("<!doctype") {
            html_seen.push(url.clone());
            last_html = truncate(head, 160).to_string();
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
            format!("chatbot 响应解析失败（{url}）：{e}; body={}", truncate(&text, 200))
        })?;
        if let Some(msg) = value.pointer("/error/message").and_then(|v| v.as_str()) {
            return Err(format!("chatbot API 错误（{url}）：{}", truncate(msg, 300)));
        }
        let content = value
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if content.is_empty() {
            return Err(format!("chatbot 返回空内容（{url}）"));
        }
        return Ok(content);
    }
    Err(format!(
        "chatbot 返回的是网页而非 JSON（已尝试：{}）。API 地址可能填成了网站首页/文档页——应填 OpenAI 兼容端点（形如 https://api.xxx.com/v1，程序自动拼 /chat/completions），Gemini/Claude 原生 API 不适用。收到：{}",
        html_seen.join("、"),
        truncate(&last_html, 160)
    ))
}

/// 校验自定义 API 地址：必须 http(s):// 开头（避免填成网站/路径片段）。
pub fn validate_api_url(api_url: Option<&str>) -> Result<(), String> {
    let raw = api_url.map(str::trim).filter(|s| !s.is_empty());
    if let Some(url) = raw {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("API 地址必须以 http:// 或 https:// 开头。".into());
        }
    }
    Ok(())
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
        assert_eq!(normalize_adapter("custom").unwrap(), "chatbot-custom");
        assert!(normalize_adapter("openai").is_err());
    }

    #[test]
    fn provider_urls() {
        assert!(provider_base_url("chatbot-opencode-go").unwrap().contains("opencode.ai"));
        assert!(provider_base_url("chatbot-deepseek").unwrap().contains("deepseek.com"));
        // 自定义 api_url 优先；custom 无 api_url 报错
        assert_eq!(
            resolve_base_url("chatbot-deepseek", Some("https://my.example.com/v1")).unwrap(),
            "https://my.example.com/v1"
        );
        assert_eq!(
            resolve_base_url("chatbot-deepseek", Some("https://my.example.com/v1/")).unwrap(),
            "https://my.example.com/v1"
        );
        assert!(resolve_base_url("custom", None).is_err());
    }

    #[test]
    fn model_legacy_flash_normalized_to_chat() {
        assert_eq!(default_model_for("deepseek", Some("deepseek-v4-flash")), "deepseek-chat");
        assert_eq!(default_model_for("deepseek", None), "deepseek-chat");
        assert_eq!(default_model_for("custom", Some("my-model")), "my-model");
    }

    #[test]
    fn chat_messages_build_multi_turn_with_roles() {
        let window = "【历史摘要】\n用户前文\n\n最近群聊（从旧到新）：\n小A: 你好\n机器人: 你好呀\n小A: 今天天气如何\n机器人: 多云转晴";
        let msgs = build_chat_messages("sys", window, "机器人", "那么明天呢？");
        let roles: Vec<&str> = msgs.iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["system", "user", "user", "assistant", "user", "assistant", "user"]);
        assert!(msgs[1]["content"].as_str().unwrap().contains("历史摘要"));
        assert_eq!(msgs[2]["content"].as_str().unwrap(), "你好");
        assert_eq!(msgs.last().unwrap()["content"].as_str().unwrap(), "那么明天呢？");
    }

    #[test]
    fn chat_messages_without_window_keeps_root_only() {
        let msgs = build_chat_messages("sys", "", "机器人", "单独一句");
        assert_eq!(msgs.len(), 2); // system + user(root)
        assert_eq!(msgs[1]["content"].as_str().unwrap(), "单独一句");
    }

    #[test]
    fn api_url_validation_requires_http_scheme() {
        assert!(validate_api_url(Some("https://api.xxx.com/v1")).is_ok());
        assert!(validate_api_url(Some("https://api.xxx.com/v1/")).is_ok());
        assert!(validate_api_url(Some("http://127.0.0.1:8000/v1")).is_ok());
        assert!(validate_api_url(None).is_ok());
        assert!(validate_api_url(Some("api.xxx.com/v1")).is_err());
        assert!(validate_api_url(Some("javascript:alert(1)")).is_err());
    }

    #[test]
    fn completion_urls_add_v1_fallback_only_when_missing() {
        assert_eq!(
            completion_urls("https://api.deepseek.com/v1"),
            vec!["https://api.deepseek.com/v1/chat/completions"]
        );
        assert_eq!(
            completion_urls("https://api.deepseek.com"),
            vec![
                "https://api.deepseek.com/chat/completions",
                "https://api.deepseek.com/v1/chat/completions"
            ]
        );
        assert_eq!(
            completion_urls("https://api.deepseek.com/v1/"),
            vec!["https://api.deepseek.com/v1/chat/completions"]
        );
        assert_eq!(
            completion_urls("http://127.0.0.1:8000"),
            vec![
                "http://127.0.0.1:8000/chat/completions",
                "http://127.0.0.1:8000/v1/chat/completions"
            ]
        );
    }
}