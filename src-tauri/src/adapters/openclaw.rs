pub fn candidate_executables() -> &'static [&'static str] {
    &["openclaw"]
}

/// OpenClaw CLI 2026.3+: `agent --message` (legacy `run` removed).
/// Requires `--agent`, `--session-id`, or `--to` to select a session.
pub fn build_args(prompt: &str, session_id: Option<&str>) -> Vec<String> {
    let mut args = vec!["agent".into()];
    match session_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(sid) => {
            args.push("--session-id".into());
            args.push(sid.to_string());
        }
        None => {
            args.push("--agent".into());
            args.push("main".into());
        }
    }
    args.push("--message".into());
    args.push(prompt.into());
    args.push("--json".into());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_defaults_to_main_agent() {
        assert_eq!(
            build_args("do work", None),
            vec![
                "agent",
                "--agent",
                "main",
                "--message",
                "do work",
                "--json"
            ]
        );
    }

    #[test]
    fn build_args_uses_session_id_when_present() {
        assert_eq!(
            build_args("do work", Some("sess-1")),
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
}
