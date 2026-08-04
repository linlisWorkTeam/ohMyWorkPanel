/// Codex talks Responses API; we bridge via local shim to DeepSeek-model upstream
/// (OpenCode Zen Go by default — hosts deepseek-* with the host API key).
/// Override with `LINLIS_CODEX_BASE_URL` (e.g. `https://api.deepseek.com/v1` when you have a DeepSeek key).
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:18888/v1";
pub const DEFAULT_MODEL: &str = "deepseek-v4-flash";

pub fn base_url() -> String {
    std::env::var("LINLIS_CODEX_BASE_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

/// Codex CLI args. Force DeepSeek-compatible provider via local Responses proxy (or override URL).
pub fn build_args(prompt: &str, model: Option<&str>) -> Vec<String> {
    let base = base_url();
    let mut args = vec![
        "exec".into(),
        "--json".into(),
        "--skip-git-repo-check".into(),
        "-c".into(),
        "model_provider=\"deepseek\"".into(),
        "-c".into(),
        format!("model_providers.deepseek.base_url=\"{base}\""),
        "-c".into(),
        "model_providers.deepseek.env_key=\"OPENAI_API_KEY\"".into(),
        "-c".into(),
        "model_providers.deepseek.name=\"deepseek\"".into(),
    ];
    let model = model
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "default")
        .unwrap_or(DEFAULT_MODEL);
    // Map legacy OpenAI catalog picks to DeepSeek defaults.
    let model = match model {
        "gpt-5" | "o3" | "o4-mini" | "gpt-4.1" | "deepseek-chat" => DEFAULT_MODEL,
        other => other,
    };
    args.push("-m".into());
    args.push(model.to_string());
    // Explicit argv prompt; keep stdin null at spawn so Codex does not block on stdin.
    args.push(prompt.into());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forces_deepseek_provider_and_default_model() {
        let args = build_args("hi", None);
        assert!(args.windows(2).any(|w| w[0] == "-c" && w[1].contains("model_provider")));
        assert!(args.windows(2).any(|w| w[0] == "-c" && w[1].contains("18888")));
        assert!(args.windows(2).any(|w| w[0] == "-m" && w[1] == "deepseek-v4-flash"));
        assert_eq!(args.last().map(String::as_str), Some("hi"));
    }

    #[test]
    fn remaps_legacy_openai_model_ids() {
        let args = build_args("x", Some("gpt-5"));
        assert!(args.windows(2).any(|w| w[0] == "-m" && w[1] == "deepseek-v4-flash"));
    }
}
