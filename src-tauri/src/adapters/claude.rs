pub fn build_args(prompt: &str) -> Vec<String> {
    vec![
        "-p".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        prompt.into(),
    ]
}
