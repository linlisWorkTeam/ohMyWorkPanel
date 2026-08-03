pub fn build_args(prompt: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec!["run".into(), prompt.into(), "--format".into(), "json".into()];
    if let Some(m) = model.map(str::trim).filter(|s| !s.is_empty() && *s != "default") {
        args.push("--model".into());
        args.push(m.to_string());
    }
    args
}
