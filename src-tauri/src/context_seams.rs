//! Ordered context seams for agent prompts: inject + ledger (no full epitaph dump).

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSection {
    pub name: String,
    pub source: String,
    pub chars: usize,
    pub body: String,
}

#[derive(Debug, Serialize)]
struct LedgerItem {
    name: String,
    source: String,
    chars: usize,
}

#[derive(Debug, Clone)]
struct ActiveRow {
    date: String,
    title: String,
    href: String,
}

/// Skip empty / whitespace-only bodies so they do not appear in the ledger.
pub fn section(name: &str, source: &str, body: impl AsRef<str>) -> Option<ContextSection> {
    let body = body.as_ref();
    if body.trim().is_empty() {
        return None;
    }
    Some(ContextSection {
        name: name.to_string(),
        source: source.to_string(),
        chars: body.chars().count(),
        body: body.to_string(),
    })
}

pub fn ledger_json(sections: &[ContextSection]) -> String {
    let items: Vec<LedgerItem> = sections
        .iter()
        .map(|s| LedgerItem {
            name: s.name.clone(),
            source: s.source.clone(),
            chars: s.chars,
        })
        .collect();
    serde_json::to_string(&items).unwrap_or_else(|_| "[]".into())
}

pub fn ledger_prompt_line(sections: &[ContextSection]) -> String {
    if sections.is_empty() {
        return String::new();
    }
    let bits: Vec<String> = sections
        .iter()
        .map(|s| format!("{}:{}", s.name, s.chars))
        .collect();
    format!("【已注入上下文】{}", bits.join(" · "))
}

/// Read Active epitaph index + newest Do-not-regress. Fail-open. Cap `max_chars`.
pub fn epitaph_handoff_block(workspace: &Path, max_chars: usize) -> String {
    let max_chars = max_chars.clamp(200, 4_000);
    let readme = workspace.join("docs").join("epitaph").join("README.md");
    let Ok(index) = std::fs::read_to_string(&readme) else {
        return String::new();
    };
    let rows = parse_active_rows(&index);
    if rows.is_empty() {
        return String::new();
    }
    let mut lines = vec!["【交接 / 墓志铭 — 调度已注入】".to_string(), "Active:".to_string()];
    for row in rows.iter().take(5) {
        lines.push(format!("- {} {}", row.date, row.title));
    }
    if let Some(newest) = rows.first() {
        if let Some(rel) = safe_epitaph_rel(&newest.href) {
            let path = workspace.join("docs").join("epitaph").join(rel);
            if let Ok(body) = std::fs::read_to_string(&path) {
                let excerpt = newest_excerpt(&body, 700);
                if !excerpt.trim().is_empty() {
                    lines.push(format!("最新约束（{}）：", newest.href.trim()));
                    lines.push(excerpt);
                }
            }
        }
    }
    lines.push("完整交接见工作区 docs/epitaph/；本块为摘要，不是全文。".into());
    truncate_chars(&lines.join("\n"), max_chars)
}

fn parse_active_rows(readme: &str) -> Vec<ActiveRow> {
    let active = slice_section(readme, "## Active");
    let mut rows = Vec::new();
    for line in active.lines() {
        let line = line.trim();
        if !line.starts_with('|') || line.contains("------") || line.contains("Date") {
            continue;
        }
        let cols: Vec<&str> = line.split('|').map(str::trim).filter(|c| !c.is_empty()).collect();
        if cols.len() < 2 {
            continue;
        }
        let date = cols[0].to_string();
        if !date.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            continue;
        }
        let (title, href) = parse_md_link(cols[1]);
        if title.is_empty() {
            continue;
        }
        rows.push(ActiveRow { date, title, href });
    }
    rows
}

fn slice_section<'a>(md: &'a str, heading: &str) -> &'a str {
    let start = match md.find(heading) {
        Some(i) => i + heading.len(),
        None => return "",
    };
    let rest = &md[start..];
    let end = rest
        .find("\n## ")
        .or_else(|| rest.find("\n##"))
        .unwrap_or(rest.len());
    &rest[..end]
}

fn parse_md_link(cell: &str) -> (String, String) {
    let cell = cell.trim();
    if let Some(lb) = cell.find('[') {
        if let Some(rb) = cell.find("](") {
            if let Some(rp) = cell[rb + 2..].find(')') {
                let title = cell[lb + 1..rb].trim().to_string();
                let href = cell[rb + 2..rb + 2 + rp].trim().to_string();
                return (title, href);
            }
        }
    }
    (cell.to_string(), String::new())
}

/// Only `foo.md` or `./foo.md` inside docs/epitaph. Reject `..` and absolute paths.
fn safe_epitaph_rel(href: &str) -> Option<PathBuf> {
    let t = href.trim();
    if t.is_empty() || t.contains("..") || t.starts_with('/') || t.contains('\\') {
        return None;
    }
    let t = t.strip_prefix("./").unwrap_or(t);
    if t.contains('/') || !t.ends_with(".md") {
        return None;
    }
    Some(PathBuf::from(t))
}

fn newest_excerpt(body: &str, max_chars: usize) -> String {
    let title = body
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim())
        .unwrap_or("");
    let regress = slice_section(body, "## Do not regress");
    let mut out = String::new();
    if !title.is_empty() {
        out.push_str(title);
        out.push('\n');
    }
    if !regress.trim().is_empty() {
        for line in regress.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with("---") {
                continue;
            }
            out.push_str(t);
            out.push('\n');
        }
    } else {
        for line in body.lines().filter(|l| !l.trim().is_empty()).take(8) {
            out.push_str(line.trim());
            out.push('\n');
        }
    }
    truncate_chars(out.trim(), max_chars)
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn empty_body_omitted_from_ledger() {
        assert!(section("wiki", "wiki", "  \n").is_none());
        let s = section("announcement", "group.announcement", "【群公告】x").unwrap();
        let json = ledger_json(&[s.clone()]);
        assert!(json.contains("\"name\":\"announcement\""));
        assert!(json.contains("\"chars\":"));
        assert!(!json.contains("【群公告】"));
        assert_eq!(ledger_prompt_line(&[s]), "【已注入上下文】announcement:6");
        assert!(ledger_prompt_line(&[]).is_empty());
    }

    #[test]
    fn missing_epitaph_dir_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(epitaph_handoff_block(dir.path(), 1200).is_empty());
    }

    #[test]
    fn parses_active_and_do_not_regress() {
        let dir = tempfile::tempdir().unwrap();
        let epi = dir.path().join("docs").join("epitaph");
        fs::create_dir_all(&epi).unwrap();
        fs::write(
            epi.join("README.md"),
            r#"# Epitaph Index

## Active

| Date | Topic | Status |
|------|-------|--------|
| 2026-08-15 | [补打 tag](./2026-08-15-git-tag-ssot.md) | active |
| 2026-08-15 | [群设置](./2026-08-15-group-settings-tab.md) | active |

## Archive

| Date | Topic | Status |
|------|-------|--------|
| 2026-07-19 | [old](./old.md) | archived |
"#,
        )
        .unwrap();
        fs::write(
            epi.join("2026-08-15-git-tag-ssot.md"),
            r#"# Epitaph: tags

## Built this session
- lots of history that must not flood the prompt

## Do not regress
- 勿把 v1.3.0 移到 HEAD
- 勿混淆双槽位 epitaph

## Open follow-ups
- ignore me
"#,
        )
        .unwrap();
        fs::write(epi.join("old.md"), "# archived\n## Do not regress\n- should not appear\n").unwrap();

        let block = epitaph_handoff_block(dir.path(), 1200);
        assert!(block.contains("【交接 / 墓志铭"));
        assert!(block.contains("补打 tag"));
        assert!(block.contains("群设置"));
        assert!(block.contains("勿把 v1.3.0 移到 HEAD"));
        assert!(!block.contains("should not appear"));
        assert!(!block.contains("lots of history"));
        assert!(block.contains("完整交接见工作区"));
    }

    #[test]
    fn rejects_path_escape_and_caps_length() {
        assert!(safe_epitaph_rel("../secret.md").is_none());
        assert!(safe_epitaph_rel("/etc/passwd.md").is_none());
        assert!(safe_epitaph_rel("nested/x.md").is_none());
        assert_eq!(
            safe_epitaph_rel("./2026-08-15-git-tag-ssot.md"),
            Some(PathBuf::from("2026-08-15-git-tag-ssot.md"))
        );

        let dir = tempfile::tempdir().unwrap();
        let epi = dir.path().join("docs").join("epitaph");
        fs::create_dir_all(&epi).unwrap();
        fs::write(
            epi.join("README.md"),
            "## Active\n\n| Date | Topic | Status |\n|------|-------|--------|\n| 2026-08-15 | [x](./x.md) | active |\n",
        )
        .unwrap();
        fs::write(epi.join("x.md"), format!("# T\n\n## Do not regress\n- {}\n", "禁".repeat(2000))).unwrap();
        let block = epitaph_handoff_block(dir.path(), 400);
        assert!(block.chars().count() <= 400);
        assert!(block.ends_with('…') || block.chars().count() < 400);
    }
}
