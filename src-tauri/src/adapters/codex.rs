pub fn build_args(prompt: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "exec".into(),
        "--json".into(),
        "--skip-git-repo-check".into(),
    ];
    if let Some(m) = model.map(str::trim).filter(|s| !s.is_empty() && *s != "default") {
        args.push("-m".into());
        args.push(m.to_string());
    }
    args.push(prompt.into());
    args
}
