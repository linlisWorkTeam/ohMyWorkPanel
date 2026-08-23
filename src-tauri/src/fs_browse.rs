use serde::Serialize;
use std::path::{Path, PathBuf};

/// Resolve and validate a workspace / browse path on the server machine.
/// Requires an absolute path that exists and is a directory.
/// 注意：canonicalize 仅用于校验（Windows 会产生 `\\?\` 前缀，不适合入库/展示），
/// 返回值是去掉尾分隔符的原始绝对路径，跨平台一致。
pub fn resolve_server_dir(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Err("路径不能为空。".into());
    }
    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return Err("工作目录必须是服务器上的绝对路径。".into());
    }
    let canon = path
        .canonicalize()
        .map_err(|e| format!("工作目录不存在或不可访问：{e}"))?;
    if !canon.is_dir() {
        return Err("路径不是目录。".into());
    }
    Ok(PathBuf::from(trimmed))
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

/// Validate a single path segment for a new folder (no separators / traversal).
pub fn validate_folder_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("文件夹名称不能为空。".into());
    }
    if name == "." || name == ".." {
        return Err("非法的文件夹名称。".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err("文件夹名称不能包含路径分隔符。".into());
    }
    if name.len() > 200 {
        return Err("文件夹名称过长。".into());
    }
    Ok(())
}

/// Create a new directory under an existing absolute server directory.
/// Returns the canonical path of the created folder.
pub fn create_server_dir(parent_raw: &str, name: &str) -> Result<PathBuf, String> {
    let parent = resolve_server_dir(parent_raw)?;
    validate_folder_name(name)?;
    let name = name.trim();
    let dest = parent.join(name);
    if dest.exists() {
        return Err(format!("已存在同名路径：{}", dest.display()));
    }
    std::fs::create_dir(&dest).map_err(|e| format!("创建文件夹失败：{e}"))?;
    Ok(dest)
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
        #[cfg(windows)]
        let missing = r"C:\linlis-definitely-missing-dir-xyz";
        #[cfg(not(windows))]
        let missing = "/tmp/linlis-definitely-missing-dir-xyz";
        let err = resolve_server_dir(missing).unwrap_err();
        assert!(
            err.contains("不存在") || err.contains("不可访问"),
            "unexpected error: {err}"
        );
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

    #[test]
    fn create_server_dir_makes_folder() {
        let dir = tempfile::tempdir().unwrap();
        let created =
            create_server_dir(dir.path().to_str().unwrap(), "new-workspace").unwrap();
        assert!(created.is_dir());
        assert!(created.ends_with("new-workspace"));
        let err = create_server_dir(dir.path().to_str().unwrap(), "new-workspace").unwrap_err();
        assert!(err.contains("已存在"));
    }

    #[test]
    fn rejects_unsafe_folder_names() {
        assert!(validate_folder_name("").is_err());
        assert!(validate_folder_name("..").is_err());
        assert!(validate_folder_name("a/b").is_err());
        assert!(validate_folder_name("ok-name").is_ok());
    }
}
