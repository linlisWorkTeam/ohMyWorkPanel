pub fn build_args(prompt: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "-p".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
    ];
    if let Some(m) = model.map(str::trim).filter(|s| !s.is_empty() && *s != "default") {
        args.push("--model".into());
        args.push(m.to_string());
    }
    args.push(prompt.into());
    args
}
