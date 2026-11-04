use std::path::{Path, PathBuf};

/// Ensure group/agent `.ohmyworkpanel` memory layout under the group workspace.
pub fn ensure_ohmyworkpanel_layout(
    group_ws: &Path,
    agent_member_id: Option<&str>,
) -> Result<(), String> {
    let root = group_ws.join(".ohmyworkpanel");
    std::fs::create_dir_all(root.join("memory")).map_err(|e| e.to_string())?;
    if let Some(aid) = agent_member_id {
        let agent_dir = default_agent_workspace(group_ws, aid);
        std::fs::create_dir_all(agent_dir.join("scratch")).map_err(|e| e.to_string())?;
        let mem = agent_dir.join("memory.md");
        if !mem.exists() {
            std::fs::write(&mem, "# Agent memory\n").map_err(|e| e.to_string())?;
        }
    }
    let group_mem = root.join("memory").join("group.md");
    if !group_mem.exists() {
        std::fs::write(&group_mem, "# Group memory\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn default_agent_workspace(group_ws: &Path, agent_member_id: &str) -> PathBuf {
    group_ws
        .join(".ohmyworkpanel")
        .join("agents")
        .join(agent_member_id)
}

pub fn append_group_memory(group_ws: &Path, title: &str, content: &str) -> Result<(), String> {
    ensure_ohmyworkpanel_layout(group_ws, None)?;
    let path = group_ws
        .join(".ohmyworkpanel")
        .join("memory")
        .join("group.md");
    let snippet = format!(
        "\n## {}\n{}\n",
        title.trim(),
        truncate(content.trim(), 1200)
    );
    append_file(&path, &snippet)
}

pub fn append_agent_memory(
    group_ws: &Path,
    agent_member_id: &str,
    title: &str,
    content: &str,
) -> Result<(), String> {
    ensure_ohmyworkpanel_layout(group_ws, Some(agent_member_id))?;
    let path = default_agent_workspace(group_ws, agent_member_id).join("memory.md");
    let snippet = format!(
        "\n## {}\n{}\n",
        title.trim(),
        truncate(content.trim(), 1200)
    );
    append_file(&path, &snippet)
}

pub fn read_memory_excerpt(
    group_ws: &Path,
    agent_member_id: Option<&str>,
    max_chars: usize,
) -> String {
    let mut out = String::new();
    let group_path = group_ws
        .join(".ohmyworkpanel")
        .join("memory")
        .join("group.md");
    if let Ok(g) = std::fs::read_to_string(&group_path) {
        let t = truncate(g.trim(), max_chars / 2);
        if !t.is_empty() {
            out.push_str("群记忆摘要：\n");
            out.push_str(&t);
            out.push('\n');
        }
    }
    if let Some(aid) = agent_member_id {
        let agent_path = default_agent_workspace(group_ws, aid).join("memory.md");
        if let Ok(a) = std::fs::read_to_string(&agent_path) {
            let t = truncate(a.trim(), max_chars / 2);
            if !t.is_empty() {
                out.push_str("Agent 记忆摘要：\n");
                out.push_str(&t);
            }
        }
    }
    out
}

fn append_file(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(text.as_bytes()).map_err(|e| e.to_string())
}

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max).collect::<String>())
}

/// Resolve agent workspace under group root; create default if empty.
/// When `allow_cross_workspace` is true (seed/system groups only), an explicit
/// absolute `configured` path may leave the group directory.
pub fn resolve_agent_workspace_under_group(
    group_ws: &Path,
    agent_member_id: &str,
    configured: Option<&str>,
) -> Result<PathBuf, String> {
    resolve_agent_workspace(group_ws, agent_member_id, configured, false)
}

pub fn resolve_agent_workspace(
    group_ws: &Path,
    agent_member_id: &str,
    configured: Option<&str>,
    allow_cross_workspace: bool,
) -> Result<PathBuf, String> {
    let group_canon = group_ws
        .canonicalize()
        .map_err(|e| format!("群工作区无效：{e}。请在群设置中重新选择本机上的目录（跨系统迁移的群需要更新工作区路径）"))?;
    let target = if let Some(raw) = configured.map(str::trim).filter(|s| !s.is_empty()) {
        let p = Path::new(raw);
        // 绝对路径原样使用；相对路径按群工作区解析（如 `agents/a1`）。
        let candidate = if p.is_absolute() || p.has_root() {
            normalize_lexical(p)
        } else {
            normalize_lexical(&group_canon.join(p))
        };
        // 先做词法越界检查，避免在群目录之外建目录（`..` 逃逸）。
        if !candidate.starts_with(&group_canon) && !allow_cross_workspace {
            return Err("Agent 工作区必须位于群工作区目录之内。".into());
        }
        std::fs::create_dir_all(&candidate)
            .map_err(|e| format!("无法创建 Agent 工作区 {}：{e}", candidate.display()))?;
        let canon = candidate.canonicalize().map_err(|e| e.to_string())?;
        // 符号链接仍可能指到群外，最终再校验一次。
        if !canon.starts_with(&group_canon) && !allow_cross_workspace {
            return Err("Agent 工作区必须位于群工作区目录之内。".into());
        }
        canon
    } else {
        let d = default_agent_workspace(&group_canon, agent_member_id);
        std::fs::create_dir_all(d.join("scratch")).map_err(|e| e.to_string())?;
        d.canonicalize().unwrap_or(d)
    };
    Ok(target)
}

/// 纯词法归一化：折叠 `.` 与 `..`，不访问文件系统。
fn normalize_lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_ws_must_stay_under_group() {
        let dir = tempfile::tempdir().unwrap();
        let group = dir.path().join("group");
        std::fs::create_dir_all(&group).unwrap();
        let ok = resolve_agent_workspace_under_group(&group, "a1", None).unwrap();
        assert!(ok.starts_with(group.canonicalize().unwrap()));
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let err =
            resolve_agent_workspace_under_group(&group, "a1", Some(outside.to_str().unwrap()))
                .unwrap_err();
        assert!(err.contains("之内"));
    }

    #[test]
    fn relative_agent_workspace_resolves_under_group() {
        let dir = tempfile::tempdir().unwrap();
        let group = dir.path().join("group");
        std::fs::create_dir_all(&group).unwrap();

        let ws = resolve_agent_workspace_under_group(&group, "a1", Some("agents/a1")).unwrap();

        let group_canon = group.canonicalize().unwrap();
        assert_eq!(ws, group_canon.join("agents").join("a1"));
        assert!(ws.is_dir());
    }

    #[test]
    fn relative_agent_workspace_cannot_escape_group() {
        let dir = tempfile::tempdir().unwrap();
        let group = dir.path().join("group");
        std::fs::create_dir_all(&group).unwrap();

        let err =
            resolve_agent_workspace_under_group(&group, "a1", Some("../outside")).unwrap_err();

        assert!(err.contains("之内"));
        // 词法检查先于建目录：群外目录不应被顺手创建出来。
        assert!(!dir.path().join("outside").exists());
    }

    #[test]
    fn seed_group_may_use_explicit_absolute_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let group = dir.path().join("group");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&group).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let ok =
            resolve_agent_workspace(&group, "a1", Some(outside.to_str().unwrap()), true).unwrap();
        assert_eq!(ok, outside.canonicalize().unwrap());
    }
}
