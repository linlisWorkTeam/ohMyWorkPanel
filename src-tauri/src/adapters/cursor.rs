pub fn build_args(prompt: &str) -> Vec<String> {
    vec![
        "-p".into(),
        prompt.into(),
        "--output-format".into(),
        "stream-json".into(),
    ]
}

pub fn candidate_executables() -> &'static [&'static str] {
    &["agent", "cursor-agent"]
}
