pub fn build_args(prompt: &str) -> Vec<String> {
    vec!["run".into(), prompt.into(), "--format".into(), "json".into()]
}
