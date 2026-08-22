//! Per-adapter model catalogs (CLI / chatbot). Empty selection = provider default.

pub fn models_for_adapter(adapter: &str) -> &'static [&'static str] {
    match adapter {
        "codex" => &[
            "deepseek-v4-flash",
            "deepseek-chat",
            "deepseek-reasoner",
        ],
        // Keep in sync with `cursor-agent --list-models` / src/agentModels.ts
        "cursor" => &[
            "auto",
            "cursor-grok-4.6-high-fast",
            "cursor-grok-4.6-high",
            "cursor-grok-4.6-xhigh-fast",
            "cursor-grok-4.6-xhigh",
            "cursor-grok-4.6-medium-fast",
            "cursor-grok-4.6-medium",
            "cursor-grok-4.6-low-fast",
            "cursor-grok-4.6-low",
            "cursor-grok-4.5-high",
            "cursor-grok-4.5-high-fast",
            "cursor-grok-4.5-medium",
            "cursor-grok-4.5-medium-fast",
            "cursor-grok-4.5-low",
            "cursor-grok-4.5-low-fast",
            "composer-2.5",
            "composer-2.5-fast",
            "kimi-k3-max",
            "kimi-k3-high",
            "kimi-k3-low",
            "kimi-k2.7-code",
            "glm-5.2-high",
            "glm-5.2-max",
        ],
        "claude-code" => &["sonnet", "opus", "haiku"],
        "opencode" => &["default", "claude-sonnet-4", "gpt-5"],
        "openclaw" => &["default"],
        // dsh: 模型选择由 DeepSeek Harness 的 profile 配置决定，前端不提供下拉。
        "dsh" => &[],
        "chatbot-deepseek" | "deepseek" => &[
            "deepseek-v4-flash",
            "deepseek-chat",
            "deepseek-reasoner",
        ],
        "chatbot-opencode-go" | "opencode-go" => &["deepseek-v4-flash", "deepseek-chat"],
        "mock" => &[],
        _ => &[],
    }
}

pub fn default_model(adapter: &str) -> Option<&'static str> {
    models_for_adapter(adapter).first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chatbot_has_flash_default() {
        assert_eq!(default_model("chatbot-deepseek"), Some("deepseek-v4-flash"));
        assert_eq!(default_model("codex"), Some("deepseek-v4-flash"));
        assert!(models_for_adapter("codex").contains(&"deepseek-reasoner"));
    }

    #[test]
    fn cursor_catalog_includes_grok_and_kimi() {
        let models = models_for_adapter("cursor");
        assert!(models.contains(&"cursor-grok-4.6-high-fast"));
        assert!(models.contains(&"cursor-grok-4.6-xhigh"));
        assert!(models.contains(&"cursor-grok-4.5-high"));
        assert!(models.contains(&"kimi-k3-max"));
        assert!(models.contains(&"kimi-k3-high"));
    }
}
