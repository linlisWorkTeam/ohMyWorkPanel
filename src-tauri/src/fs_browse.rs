use serde::Serialize;
use std::path::{Path, PathBuf};

/// Resolve and validate a workspace / browse path on the server machine.
/// Requires an absolute path that exists and is a directory.
pub fn resolve_server_dir(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("路径不能为空。".into());
    }
    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return Err("工作目录必须是服务器上的绝对路径。".into());
    }
    if !path.exists() {
        return Err("工作目录不存在或不可访问。".into());
    }
    if !path.is_dir() {
        return Err("路径不是目录。".into());
    }
    path.canonicalize()
        .map_err(|e| format!("无法解析路径：{e}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntryInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirListing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<DirEntryInfo>,
}

/// List one level of a server directory. Empty/`/` starts at filesystem root.
pub fn list_server_dir(raw: &str) -> Result<DirListing, String> {
    let trimmed = raw.trim();
    let target = if trimmed.is_empty() || trimmed == "/" {
        PathBuf::from("/")
    } else {
        resolve_server_dir(trimmed)?
    };
    if !target.is_dir() {
        return Err("路径不是目录。".into());
    }
    let mut entries = Vec::new();
    let read = std::fs::read_dir(&target).map_err(|e| format!("无法读取目录：{e}"))?;
    for ent in read {
        let ent = ent.map_err(|e| format!("读取目录项失败：{e}"))?;
        let name = ent.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let meta = match ent.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let path = ent.path();
        entries.push(DirEntryInfo {
            name,
            path: path.to_string_lossy().into_owned(),
            is_dir: meta.is_dir(),
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    let parent = target
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| !p.is_empty());
    Ok(DirListing {
        path: target.to_string_lossy().into_owned(),
        parent,
        entries,
    })
}

/// Write group announcement as a Cursor project rule under the workspace.
pub fn sync_announcement_rule(workspace: &Path, announcement: &str) -> Result<(), String> {
    let rules_dir = workspace.join(".cursor").join("rules");
    std::fs::create_dir_all(&rules_dir).map_err(|e| format!("创建 rules 目录失败：{e}"))?;
    let file = rules_dir.join("group-announcement.mdc");
    if announcement.trim().is_empty() {
        let _ = std::fs::remove_file(&file);
        return Ok(());
    }
    let body = format!(
        "---\ndescription: 群公告 — 所有 Agent 必须遵守的项目级规则\nalwaysApply: true\n---\n\n# 群公告 / 项目级规则\n\n{}\n",
        announcement.trim()
    );
    std::fs::write(&file, body).map_err(|e| format!("写入群公告 rule 失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_relative_path() {
        let err = resolve_server_dir("relative/path").unwrap_err();
        assert!(err.contains("绝对"));
    }

    #[test]
    fn rejects_missing_path() {
        let err = resolve_server_dir("/tmp/linlis-definitely-missing-dir-xyz").unwrap_err();
        assert!(err.contains("不存在") || err.contains("不可访问"));
    }

    #[test]
    fn accepts_existing_dir() {
        let dir = tempfile::tempdir().unwrap();
        let resolved = resolve_server_dir(dir.path().to_str().unwrap()).unwrap();
        assert!(resolved.is_dir());
    }

    #[test]
    fn lists_dir_entries() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("a.txt"), b"x").unwrap();
        let listing = list_server_dir(dir.path().to_str().unwrap()).unwrap();
        assert!(listing.entries.iter().any(|e| e.name == "sub" && e.is_dir));
        assert!(listing.entries.iter().any(|e| e.name == "a.txt" && !e.is_dir));
    }
}
