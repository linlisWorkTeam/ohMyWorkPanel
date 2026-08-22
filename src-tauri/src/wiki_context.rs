//! Pull shared Wiki snippets via WorkPanelWiki `cli.py retrieve` (adapter-neutral).
//! Fail-open: timeout / missing CLI → empty string (never block the agent run).

use serde::Deserialize;
use std::process::{Command, Stdio};

#[derive(Debug, Deserialize)]
pub struct WikiRetrieveResult {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    hits: Vec<WikiHit>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WikiHit {
    #[serde(default)]
    path: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    score: f64,
}

fn wiki_enabled() -> bool {
    match std::env::var("OHMYWORKPANEL_WIKI_RETRIEVE") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !(t.is_empty() || t == "0" || t == "false" || t == "off" || t == "no")
        }
        Err(_) => true, // default on when root exists
    }
}

pub fn wiki_root() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("OHMYWORKPANEL_WIKI_ROOT") {
        let pb = std::path::PathBuf::from(p.trim());
        if pb.join("agent/cli.py").is_file() {
            return Some(pb);
        }
    }
    let default = std::path::PathBuf::from("/AI/WorkPanelWiki");
    if default.join("agent/cli.py").is_file() {
        Some(default)
    } else {
        None
    }
}

fn timeout_ms() -> u64 {
    std::env::var("OHMYWORKPANEL_WIKI_RETRIEVE_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(800)
        .clamp(100, 5_000)
}

fn top_k() -> usize {
    std::env::var("OHMYWORKPANEL_WIKI_RETRIEVE_TOP_K")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
        .clamp(1, 8)
}

/// Build a short retrieval query from task root + group hint.
pub fn build_wiki_query(group_name: &str, root_task: &str, announcement: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let g = group_name.trim();
    if !g.is_empty() {
        parts.push(g.to_string());
    }
    let root: String = root_task
        .chars()
        .take(240)
        .collect::<String>()
        .split_whitespace()
        .take(24)
        .collect::<Vec<_>>()
        .join(" ");
    if !root.is_empty() {
        parts.push(root);
    } else {
        let ann: String = announcement.chars().take(120).collect();
        if !ann.trim().is_empty() {
            parts.push(ann.trim().to_string());
        }
    }
    let q = parts.join(" ");
    if q.trim().is_empty() {
        "ohmyworkpanel 运作规则 灰度".into()
    } else {
        q
    }
}

fn run_retrieve(cli: &std::path::Path, query: &str) -> Result<WikiRetrieveResult, String> {
    let secs = ((timeout_ms() + 999) / 1000).max(1);
    // Prefer GNU timeout so a hung jieba/python cannot stall the scheduler.
    let output = Command::new("timeout")
        .arg(format!("{secs}s"))
        .arg("python3")
        .arg(cli)
        .arg("retrieve")
        .arg(query)
        .arg("--top-k")
        .arg(top_k().to_string())
        .arg("--tags")
        .arg("ohmyworkpanel,ops,collab,rules")
        .arg("--excerpt-chars")
        .arg("360")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .or_else(|_| {
            Command::new("python3")
                .arg(cli)
                .arg("retrieve")
                .arg(query)
                .arg("--top-k")
                .arg(top_k().to_string())
                .arg("--tags")
                .arg("ohmyworkpanel,ops,collab,rules")
                .arg("--excerpt-chars")
                .arg("360")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
        })
        .map_err(|e| format!("spawn wiki retrieve: {e}"))?;

    if !output.status.success() && output.stdout.is_empty() {
        return Err(format!("wiki retrieve exit {:?}", output.status.code()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let json_line = text
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or(text.trim());
    serde_json::from_str(json_line).map_err(|e| format!("parse wiki json: {e}"))
}

pub fn format_wiki_block(result: &WikiRetrieveResult) -> String {
    if result.hits.is_empty() {
        return String::new();
    }
    let mut lines = vec!["\n【全局知识·Wiki】（平台检索，各适配器共通；非 CLI 私货）".to_string()];
    if let Some(note) = result.note.as_ref().filter(|n| !n.is_empty()) {
        lines.push(format!("note: {note}"));
    }
    for (i, h) in result.hits.iter().enumerate() {
        let path = if h.path.is_empty() { "?" } else { h.path.as_str() };
        let title = if h.title.is_empty() { path } else { h.title.as_str() };
        let excerpt = h.text.replace('\n', " ").chars().take(360).collect::<String>();
        lines.push(format!(
            "{}. [{}] {} (score={:.2})\n   {}",
            i + 1,
            path,
            title,
            h.score,
            excerpt
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Retrieve + format; empty on disable / miss / error.
pub fn wiki_context_block(group_name: &str, root_task: &str, announcement: &str) -> String {
    if !wiki_enabled() {
        return String::new();
    }
    let Some(root) = wiki_root() else {
        return String::new();
    };
    let cli = root.join("agent/cli.py");
    let query = build_wiki_query(group_name, root_task, announcement);
    match run_retrieve(&cli, &query) {
        Ok(r) if r.ok || !r.hits.is_empty() => format_wiki_block(&r),
        Ok(_) => String::new(),
        Err(_e) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_prefers_root() {
        let q = build_wiki_query("ohMyWorkPanel", "请灰度部署并跑 test:gate", "公告很长");
        assert!(q.contains("ohMyWorkPanel"));
        assert!(q.contains("灰度") || q.contains("test:gate") || q.contains("部署"));
    }

    #[test]
    fn format_block_lists_paths() {
        let r = WikiRetrieveResult {
            ok: true,
            note: None,
            hits: vec![WikiHit {
                path: "ohmyworkpanel/rules.md".into(),
                title: "规则".into(),
                text: "先灰度再 commit".into(),
                score: 1.2,
            }],
        };
        let s = format_wiki_block(&r);
        assert!(s.contains("【全局知识·Wiki】"));
        assert!(s.contains("ohmyworkpanel/rules.md"));
        assert!(s.contains("先灰度再 commit"));
    }

    #[test]
    fn live_retrieve_when_wiki_present() {
        if wiki_root().is_none() {
            return;
        }
        // May be slow first jieba load; allow env skip in tiny CI
        if std::env::var("OHMYWORKPANEL_WIKI_SKIP_LIVE_TEST").ok().as_deref() == Some("1") {
            return;
        }
        let block = wiki_context_block("ohMyWorkPanel", "灰度 test:gate commit 前", "");
        // Soft assert: if retrieve works we expect the ops rule; if timeout, empty is ok.
        if !block.is_empty() {
            assert!(block.contains("【全局知识·Wiki】"));
        }
    }
}
