pub fn candidate_executables() -> &'static [&'static str] {
    &["openclaw"]
}

pub fn build_args(prompt: &str) -> Vec<String> {
    vec!["run".into(), prompt.into()]
}
