pub fn build_args(prompt: &str, session_id: Option<&str>, model: Option<&str>) -> Vec<String> {
    let mut args = vec!["--trust".into()];
    if let Some(id) = session_id.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--resume".into());
        args.push(id.to_string());
    }
    if let Some(m) = model.map(str::trim).filter(|s| !s.is_empty() && *s != "default") {
        args.push("--model".into());
        args.push(m.to_string());
    }
    args.extend([
        "-p".into(),
        prompt.into(),
        "--output-format".into(),
        "stream-json".into(),
        "--stream-partial-output".into(),
    ]);
    args
}

pub fn candidate_executables() -> &'static [&'static str] {
    &["agent", "cursor-agent"]
}
