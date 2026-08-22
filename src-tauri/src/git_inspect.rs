//! Read-only git tag/log inspection for Version tab (30s process cache).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use std::sync::OnceLock;

use crate::db::AppResult;

const CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTagInfo {
    pub name: String,
    pub sha: String,
    pub date: Option<String>,
    pub subject: Option<String>,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitInfo {
    pub sha: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSnapshot {
    pub is_git_repo: bool,
    pub head_sha: Option<String>,
    pub head_matches_latest_tag: bool,
    pub finish_last_round: bool,
    pub tags: Vec<GitTagInfo>,
    pub recent_commits: Vec<GitCommitInfo>,
    pub error: Option<String>,
}

struct CacheEntry {
    at: Instant,
    snap: GitSnapshot,
}

fn cache() -> &'static Mutex<HashMap<PathBuf, CacheEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn run_git(cwd: &Path, args: &[&str]) -> AppResult<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("执行 git 失败：{e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("git {} 失败", args.join(" "))
        } else {
            err
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn inspect_workspace(workspace: &str) -> GitSnapshot {
    let path = PathBuf::from(workspace);
    if let Ok(guard) = cache().lock() {
        if let Some(entry) = guard.get(&path) {
            if entry.at.elapsed() < CACHE_TTL {
                return entry.snap.clone();
            }
        }
    }
    let snap = inspect_uncached(&path);
    if let Ok(mut guard) = cache().lock() {
        guard.insert(
            path,
            CacheEntry {
                at: Instant::now(),
                snap: snap.clone(),
            },
        );
    }
    snap
}

/// Test helper — bypass cache.
pub fn inspect_uncached(path: &Path) -> GitSnapshot {
    if !path.is_dir() {
        return GitSnapshot {
            is_git_repo: false,
            head_sha: None,
            head_matches_latest_tag: false,
            finish_last_round: false,
            tags: vec![],
            recent_commits: vec![],
            error: Some("工作区目录不存在".into()),
        };
    }
    match run_git(path, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(s) if s.trim() == "true" => {}
        Ok(_) | Err(_) => {
            return GitSnapshot {
                is_git_repo: false,
                head_sha: None,
                head_matches_latest_tag: false,
                finish_last_round: false,
                tags: vec![],
                recent_commits: vec![],
                error: None,
            };
        }
    }

    let head_sha = run_git(path, &["rev-parse", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string());

    let mut tags = Vec::new();
    if let Ok(raw) = run_git(
        path,
        &[
            "tag",
            "--sort=-creatordate",
            "--format=%(refname:short)\t%(objectname:short)\t%(creatordate:iso-strict)\t%(subject)",
        ],
    ) {
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let mut parts = line.splitn(4, '\t');
            let name = parts.next().unwrap_or("").to_string();
            let sha = parts.next().unwrap_or("").to_string();
            let date = parts.next().map(|s| s.to_string()).filter(|s| !s.is_empty());
            let subject = parts.next().map(|s| s.to_string()).filter(|s| !s.is_empty());
            if !name.is_empty() {
                tags.push(GitTagInfo {
                    name,
                    sha,
                    date,
                    subject,
                    is_virtual: false,
                });
            }
        }
    }

    // Fallback: tags without creatordate format support
    if tags.is_empty() {
        if let Ok(raw) = run_git(path, &["tag", "--sort=-version:refname"]) {
            for name in raw.lines().filter(|l| !l.trim().is_empty()) {
                let sha = run_git(path, &["rev-list", "-n", "1", name])
                    .ok()
                    .map(|s| s.trim().chars().take(12).collect())
                    .unwrap_or_default();
                tags.push(GitTagInfo {
                    name: name.to_string(),
                    sha,
                    date: None,
                    subject: None,
                    is_virtual: false,
                });
            }
        }
    }

    let mut recent_commits = Vec::new();
    if let Ok(raw) = run_git(path, &["log", "--pretty=format:%h\t%s", "-n", "20"]) {
        for line in raw.lines() {
            if let Some((sha, subject)) = line.split_once('\t') {
                recent_commits.push(GitCommitInfo {
                    sha: sha.to_string(),
                    subject: subject.to_string(),
                });
            }
        }
    }

    if tags.is_empty() {
        if let Some(c) = recent_commits.first() {
            tags.push(GitTagInfo {
                name: "v0.0.0-draft".into(),
                sha: c.sha.clone(),
                date: None,
                subject: Some(c.subject.clone()),
                is_virtual: true,
            });
        } else if let Some(ref head) = head_sha {
            tags.push(GitTagInfo {
                name: "v0.0.0-draft".into(),
                sha: head.chars().take(12).collect(),
                date: None,
                subject: Some("(empty history)".into()),
                is_virtual: true,
            });
        }
    }

    let latest = tags.first();
    let head_matches = match (&head_sha, latest) {
        (Some(h), Some(t)) => {
            let tag_commit = run_git(path, &["rev-list", "-n", "1", &t.name])
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| t.sha.clone());
            h == &tag_commit
                || h.starts_with(&t.sha)
                || tag_commit.starts_with(h)
                || h.starts_with(&tag_commit.chars().take(7).collect::<String>())
        }
        _ => false,
    };
    // finish-last-round: HEAD on latest real tag (not virtual-only draft without release)
    let finish_last_round = head_matches && latest.map(|t| !t.is_virtual).unwrap_or(false);

    GitSnapshot {
        is_git_repo: true,
        head_sha,
        head_matches_latest_tag: head_matches,
        finish_last_round,
        tags,
        recent_commits,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn init_repo(dir: &Path) {
        let _ = fs::create_dir_all(dir);
        assert!(Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
        let _ = Command::new("git")
            .args(["config", "user.email", "t@t.com"])
            .current_dir(dir)
            .status();
        let _ = Command::new("git")
            .args(["config", "user.name", "t"])
            .current_dir(dir)
            .status();
        fs::write(dir.join("a.txt"), "a").unwrap();
        assert!(Command::new("git")
            .args(["add", "."])
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn non_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        let snap = inspect_uncached(dir.path());
        assert!(!snap.is_git_repo);
        assert!(snap.tags.is_empty());
    }

    #[test]
    fn virtual_tag_when_no_tags() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        let snap = inspect_uncached(dir.path());
        assert!(snap.is_git_repo);
        assert_eq!(snap.tags.len(), 1);
        assert!(snap.tags[0].is_virtual);
        assert_eq!(snap.tags[0].name, "v0.0.0-draft");
        assert!(!snap.finish_last_round);
    }

    #[test]
    fn real_tag_and_finish_last_round() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        assert!(Command::new("git")
            .args(["tag", "-a", "v1.0.0", "-m", "r1"])
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success());
        let snap = inspect_uncached(dir.path());
        assert!(snap.tags.iter().any(|t| t.name == "v1.0.0" && !t.is_virtual));
        assert!(snap.head_matches_latest_tag);
        assert!(snap.finish_last_round);
    }
}
