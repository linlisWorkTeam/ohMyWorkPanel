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

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Resolve OPENAI_API_KEY for Codex child processes (systemd often lacks shell env).
/// Order: member/explicit → `LINLIS_CODEX_API_KEY` → `OPENAI_API_KEY` → `~/.codex/auth.json`.
pub fn resolve_api_key(explicit: Option<&str>) -> Option<String> {
    resolve_api_key_with(
        explicit,
        || {
            non_empty(std::env::var("LINLIS_CODEX_API_KEY").ok())
                .or_else(|| non_empty(std::env::var("OPENAI_API_KEY").ok()))
        },
        || read_openai_key_from_auth_path(&default_auth_path()),
    )
}

pub(crate) fn resolve_api_key_with(
    explicit: Option<&str>,
    env_key: impl FnOnce() -> Option<String>,
    auth_file_key: impl FnOnce() -> Option<String>,
) -> Option<String> {
    non_empty(explicit.map(|s| s.to_string()))
        .or_else(env_key)
        .or_else(auth_file_key)
}

fn default_auth_path() -> std::path::PathBuf {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/root"));
    home.join(".codex").join("auth.json")
}

pub(crate) fn read_openai_key_from_auth_path(path: &std::path::Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    non_empty(
        value
            .get("OPENAI_API_KEY")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    )
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
    use std::io::Write;

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

    #[test]
    fn resolve_api_key_prefers_explicit_then_env_then_auth_file() {
        assert_eq!(
            resolve_api_key_with(
                Some("  member-key  "),
                || Some("env-key".into()),
                || Some("file-key".into()),
            )
            .as_deref(),
            Some("member-key")
        );
        assert_eq!(
            resolve_api_key_with(None, || Some("env-key".into()), || Some("file-key".into()))
                .as_deref(),
            Some("env-key")
        );
        assert_eq!(
            resolve_api_key_with(Some("  "), || None, || Some("file-key".into())).as_deref(),
            Some("file-key")
        );
        assert_eq!(resolve_api_key_with(None, || None, || None), None);
    }

    #[test]
    fn reads_openai_key_from_auth_json() {
        let dir = std::env::temp_dir().join(format!("linlis-codex-auth-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, r#"{{"OPENAI_API_KEY":"sk-from-file","auth_mode":"apikey"}}"#).unwrap();
        assert_eq!(read_openai_key_from_auth_path(&path).as_deref(), Some("sk-from-file"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
