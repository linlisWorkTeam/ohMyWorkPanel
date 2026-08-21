//! Declarative CLI adapter manifests (`*.adapter.json`).
//! P0: scan dirs, argv templates, jsonl/plain/cursor-stream-json. Builtins remain fallback.

use super::{
    find_executable_path,
    parse::{parse_agent_event, DeltaMode, ParsedEvent},
    AdapterKind,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::RwLock,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamDialect {
    Jsonl,
    Plain,
    #[serde(rename = "cursor-stream-json")]
    CursorStreamJson,
}

impl Default for StreamDialect {
    fn default() -> Self {
        Self::Jsonl
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterManifest {
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    pub executables: Vec<String>,
    pub args: Vec<String>,
    #[serde(default)]
    pub stream: StreamDialect,
    #[serde(default)]
    pub resume_flag: Option<String>,
    #[serde(default)]
    pub model_flag: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterCatalogItem {
    pub id: String,
    pub display_name: String,
    pub source: String, // "builtin" | "manifest"
}

#[derive(Debug, Clone)]
pub enum SpawnSpec {
    Builtin(AdapterKind),
    Manifest(AdapterManifest),
}

static MANIFESTS: RwLock<Option<HashMap<String, AdapterManifest>>> = RwLock::new(None);

fn id_ok(id: &str) -> bool {
    let b = id.as_bytes();
    if b.is_empty() {
        return false;
    }
    let mut start = true;
    for &c in b {
        if start {
            if !c.is_ascii_lowercase() && !c.is_ascii_digit() {
                return false;
            }
            start = false;
            continue;
        }
        if c == b'-' {
            start = true;
            continue;
        }
        if !c.is_ascii_lowercase() && !c.is_ascii_digit() {
            return false;
        }
    }
    !start && id.chars().all(|ch| ch.is_ascii())
}

fn looks_like_shell(arg: &str) -> bool {
    let lower = arg.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "sh" | "bash" | "zsh" | "cmd" | "cmd.exe" | "powershell" | "powershell.exe" | "-c" | "/c"
    ) || arg.contains("&&")
        || arg.contains("||")
        || arg.contains(';')
        || arg.contains('|')
        || arg.contains('`')
        || arg.contains("$(")
}

pub fn validate_manifest(m: &AdapterManifest) -> Result<(), String> {
    if !id_ok(&m.id) {
        return Err(format!("适配器 id 非法：{}", m.id));
    }
    if m.executables.is_empty() {
        return Err("executables 不能为空".into());
    }
    for exe in &m.executables {
        let base = Path::new(exe)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(exe);
        if exe.trim().is_empty() || looks_like_shell(exe) || looks_like_shell(base) {
            return Err(format!("拒绝以 shell 作为可执行文件：{exe}"));
        }
    }
    if m.args.iter().all(|a| a != "{prompt}") {
        return Err("args 必须包含独立的 {prompt} 元素".into());
    }
    for a in &m.args {
        if looks_like_shell(a) {
            return Err(format!("拒绝 shell argv：{a}"));
        }
        if a == "{prompt}" || a == "{model}" || a == "{session}" {
            continue;
        }
        if a.contains('{') || a.contains('}') {
            return Err("占位符必须单独占一个 argv 元素".into());
        }
    }
    Ok(())
}

pub fn parse_manifest_bytes(raw: &[u8]) -> Result<AdapterManifest, String> {
    let mut m: AdapterManifest =
        serde_json::from_slice(raw).map_err(|e| format!("解析 adapter.json：{e}"))?;
    if m.display_name.trim().is_empty() {
        m.display_name = m.id.clone();
    }
    validate_manifest(&m)?;
    Ok(m)
}

pub fn expand_args(
    m: &AdapterManifest,
    prompt: &str,
    model: Option<&str>,
    session: Option<&str>,
) -> Vec<String> {
    let model = model.map(str::trim).filter(|s| !s.is_empty() && *s != "default");
    let session = session.map(str::trim).filter(|s| !s.is_empty());
    let mut out = Vec::new();
    for a in &m.args {
        match a.as_str() {
            "{prompt}" => out.push(prompt.to_string()),
            "{model}" => {
                if let Some(model) = model {
                    out.push(model.to_string());
                } else if out.last().map(|s| s.starts_with('-')).unwrap_or(false) {
                    out.pop();
                }
            }
            "{session}" => {
                if let Some(session) = session {
                    out.push(session.to_string());
                } else if out.last().map(|s| s.starts_with('-')).unwrap_or(false) {
                    out.pop();
                }
            }
            other => out.push(other.to_string()),
        }
    }
    if let Some(flag) = m.model_flag.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if model.is_some() && !m.args.iter().any(|a| a == "{model}") {
            out.push(flag.to_string());
            out.push(model.unwrap().to_string());
        }
    }
    if let Some(flag) = m.resume_flag.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if session.is_some() && !m.args.iter().any(|a| a == "{session}") {
            // insert after first arg-ish: prepend flag+session like cursor --resume
            let sid = session.unwrap().to_string();
            if !out.iter().any(|x| x == flag) {
                out.insert(0, flag.to_string());
                out.insert(1, sid);
            }
        }
    }
    out
}

pub fn load_dir(dir: &Path) -> Result<Vec<AdapterManifest>, String> {
    let mut out = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(out),
    };
    for ent in rd {
        let ent = ent.map_err(|e| e.to_string())?;
        let path = ent.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.ends_with(".adapter.json") {
            continue;
        }
        let raw = std::fs::read(&path).map_err(|e| format!("{}：{e}", path.display()))?;
        out.push(parse_manifest_bytes(&raw)?);
    }
    Ok(out)
}

fn scan_roots() -> HashMap<String, AdapterManifest> {
    let mut map = HashMap::new();
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(raw) = std::env::var("LINLIS_ADAPTER_ROOTS") {
        for part in raw.split(|c| c == ':' || c == ';') {
            let p = part.trim();
            if !p.is_empty() {
                roots.push(PathBuf::from(p));
            }
        }
    }
    if let Ok(root) = std::env::var("LINLIS_ROOT") {
        roots.push(PathBuf::from(root).join("adapters"));
    }
    for root in roots {
        if let Ok(list) = load_dir(&root) {
            for m in list {
                map.insert(m.id.clone(), m);
            }
        }
    }
    map
}

pub fn reload_manifests() {
    let table = scan_roots();
    if let Ok(mut g) = MANIFESTS.write() {
        *g = Some(table);
    }
}

fn manifest_table() -> HashMap<String, AdapterManifest> {
    {
        let g = MANIFESTS.read().ok();
        if let Some(g) = g {
            if let Some(t) = g.as_ref() {
                return t.clone();
            }
        }
    }
    reload_manifests();
    MANIFESTS
        .read()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default()
}

pub fn resolve_adapter(name: &str) -> Result<SpawnSpec, String> {
    resolve_adapter_with(&manifest_table(), name)
}

pub fn resolve_adapter_with(
    table: &HashMap<String, AdapterManifest>,
    name: &str,
) -> Result<SpawnSpec, String> {
    let name = name.trim();
    if let Some(m) = table.get(name) {
        return Ok(SpawnSpec::Manifest(m.clone()));
    }
    AdapterKind::parse(name).map(SpawnSpec::Builtin)
}

pub fn catalog() -> Vec<AdapterCatalogItem> {
    catalog_with(&manifest_table())
}

pub fn catalog_with(table: &HashMap<String, AdapterManifest>) -> Vec<AdapterCatalogItem> {
    let mut items: Vec<AdapterCatalogItem> = builtin_catalog();
    for item in items.iter_mut() {
        if let Some(m) = table.get(&item.id) {
            item.display_name = m.display_name.clone();
            item.source = "manifest".into();
        }
    }
    let mut extra: Vec<AdapterCatalogItem> = table
        .values()
        .filter(|m| items.iter().all(|i| i.id != m.id))
        .map(|m| AdapterCatalogItem {
            id: m.id.clone(),
            display_name: m.display_name.clone(),
            source: "manifest".into(),
        })
        .collect();
    extra.sort_by(|a, b| a.id.cmp(&b.id));
    items.extend(extra);
    items
}

pub fn builtin_catalog() -> Vec<AdapterCatalogItem> {
    [
        ("mock", "模拟 Agent（推荐体验）"),
        ("codex", "Codex CLI"),
        ("openclaw", "OpenClaw"),
        ("cursor", "Cursor CLI（agent/cursor-agent）"),
        ("claude-code", "Claude Code"),
        ("opencode", "OpenCode"),
        ("dsh", "DeepSeek Harness（dsh）"),
    ]
    .into_iter()
    .map(|(id, display_name)| AdapterCatalogItem {
        id: id.into(),
        display_name: display_name.into(),
        source: "builtin".into(),
    })
    .collect()
}

impl SpawnSpec {
    pub fn id(&self) -> &str {
        match self {
            Self::Builtin(k) => k.as_str(),
            Self::Manifest(m) => &m.id,
        }
    }

    pub fn builtin_kind(&self) -> Option<AdapterKind> {
        match self {
            Self::Builtin(k) => Some(*k),
            Self::Manifest(_) => None,
        }
    }

    pub fn persists_session(&self) -> bool {
        match self {
            Self::Builtin(AdapterKind::Cursor) => true,
            Self::Manifest(m) => m
                .resume_flag
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_some(),
            _ => false,
        }
    }

    pub fn timeout_secs(&self, fallback: u64) -> u64 {
        match self {
            Self::Manifest(m) => m.timeout_secs.unwrap_or(fallback),
            _ => fallback,
        }
    }

    pub fn build_args(&self, prompt: &str, session_id: Option<&str>, model: Option<&str>) -> Vec<String> {
        match self {
            Self::Builtin(k) => k.build_args(prompt, session_id, model),
            Self::Manifest(m) => expand_args(m, prompt, model, session_id),
        }
    }

    pub fn parse_event(&self, line: &str) -> ParsedEvent {
        match self {
            Self::Builtin(k) => k.parse_event(line),
            Self::Manifest(m) => match m.stream {
                StreamDialect::Plain => ParsedEvent {
                    channel: "final".into(),
                    text: line.to_string(),
                    session_id: None,
                    mode: DeltaMode::Append,
                },
                StreamDialect::Jsonl | StreamDialect::CursorStreamJson => parse_agent_event(line),
            },
        }
    }

    pub fn resolve_executable(&self, configured: Option<&str>) -> Result<String, String> {
        match self {
            Self::Builtin(k) => k.resolve_executable(configured),
            Self::Manifest(m) => resolve_from_candidates(&m.executables, configured),
        }
    }
}

pub fn resolve_from_candidates(
    candidates: &[String],
    configured: Option<&str>,
) -> Result<String, String> {
    if let Some(path) = configured
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| !s.starts_with("http://") && !s.starts_with("https://"))
    {
        return Ok(path.to_string());
    }
    if candidates.is_empty() {
        return Err("此适配器暂未提供运行器。".into());
    }
    for name in candidates {
        if let Some(full) = find_executable_path(name) {
            return Ok(full);
        }
        #[cfg(windows)]
        {
            if let Some(npm_path) = super::find_in_npm(name) {
                return Ok(npm_path);
            }
        }
    }
    Ok(candidates[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_json() -> String {
        r#"{
          "id": "acme-cli",
          "displayName": "Acme CLI",
          "executables": ["acme", "acme-cli"],
          "args": ["run", "{prompt}", "--json"],
          "stream": "jsonl"
        }"#
        .into()
    }

    #[test]
    fn parse_accepts_p0_shape() {
        let m = parse_manifest_bytes(sample_json().as_bytes()).unwrap();
        assert_eq!(m.id, "acme-cli");
        assert_eq!(m.executables[0], "acme");
    }

    #[test]
    fn prompt_is_own_argv_element() {
        let m = parse_manifest_bytes(sample_json().as_bytes()).unwrap();
        let args = expand_args(&m, "do work", None, None);
        assert_eq!(args, vec!["run", "do work", "--json"]);
        assert_eq!(args.iter().filter(|a| *a == "do work").count(), 1);
    }

    #[test]
    fn interpolated_placeholder_rejected() {
        let raw = r#"{
          "id": "bad-cli",
          "executables": ["bad"],
          "args": ["run {prompt}", "{prompt}"]
        }"#;
        let err = parse_manifest_bytes(raw.as_bytes()).unwrap_err();
        assert!(err.contains("占位符"), "{err}");
    }

    #[test]
    fn shell_c_rejected() {
        let raw = r#"{
          "id": "evil-cli",
          "executables": ["sh"],
          "args": ["-c", "{prompt}"]
        }"#;
        let err = parse_manifest_bytes(raw.as_bytes()).unwrap_err();
        assert!(err.contains("shell") || err.contains("拒绝"), "{err}");
    }

    #[test]
    fn unknown_id_still_errors() {
        let table = HashMap::new();
        let err = resolve_adapter_with(&table, "unknown").unwrap_err();
        assert!(err.contains("不支持"), "{err}");
    }

    #[test]
    fn builtin_cursor_still_resolves() {
        let table = HashMap::new();
        match resolve_adapter_with(&table, "cursor").unwrap() {
            SpawnSpec::Builtin(AdapterKind::Cursor) => {}
            other => panic!("expected builtin cursor, got {}", other.id()),
        }
    }

    #[test]
    fn load_dir_and_override_catalog() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("acme.adapter.json"), sample_json()).unwrap();
        let list = load_dir(dir.path()).unwrap();
        assert_eq!(list.len(), 1);
        let mut table = HashMap::new();
        table.insert(list[0].id.clone(), list[0].clone());
        let spec = resolve_adapter_with(&table, "acme-cli").unwrap();
        assert_eq!(spec.id(), "acme-cli");
        let cat = catalog_with(&table);
        assert!(cat.iter().any(|i| i.id == "acme-cli" && i.source == "manifest"));
        assert!(cat.iter().any(|i| i.id == "cursor" && i.source == "builtin"));
    }

    #[test]
    fn omits_optional_model_placeholder() {
        let raw = r#"{
          "id": "acme-cli",
          "executables": ["acme"],
          "args": ["run", "{prompt}", "--model", "{model}"]
        }"#;
        let m = parse_manifest_bytes(raw.as_bytes()).unwrap();
        assert_eq!(expand_args(&m, "x", None, None), vec!["run", "x"]);
        assert_eq!(
            expand_args(&m, "x", Some("grok"), None),
            vec!["run", "x", "--model", "grok"]
        );
    }
}
