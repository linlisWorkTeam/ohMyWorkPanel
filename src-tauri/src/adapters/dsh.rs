//! DeepSeek Harness (`dsh`) adapter.
//!
//! Runs the **headless** profile: `dsh --profile headless "task"`.
//! One-shot session — it prints the final answer (plain text) to stdout, then
//! exits. No interactive follow-up / resume, so `session_id` / `model` are
//! ignored (model selection belongs to the profile's own config).

pub fn candidate_executables() -> &'static [&'static str] {
    &["dsh"]
}

pub fn build_args(prompt: &str, _session_id: Option<&str>, _model: Option<&str>) -> Vec<String> {
    vec!["--profile".into(), "headless".into(), prompt.into()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_uses_headless_profile() {
        assert_eq!(
            build_args("do work", None, None),
            vec!["--profile", "headless", "do work"]
        );
    }

    #[test]
    fn build_args_ignores_session_and_model() {
        assert_eq!(
            build_args("do work", Some("sess-1"), Some("gpt-5")),
            vec!["--profile", "headless", "do work"]
        );
    }

    #[test]
    fn candidate_is_dsh() {
        assert_eq!(candidate_executables(), &["dsh"]);
    }
}
