pub fn build_args(prompt: &str) -> Vec<String> {
    vec![
        "exec".into(),
        "--json".into(),
        "--skip-git-repo-check".into(),
        prompt.into(),
    ]
}
