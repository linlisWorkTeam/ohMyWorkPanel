use super::models::{
    CommitSummary, Evidence, MarketingConfig, RepositorySnapshot, REQUIRED_CHANNELS,
};
use crate::db::{now, AppResult};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_GIT_OUTPUT: usize = 80_000;
const MAX_FILE_BYTES: usize = 24_000;
const MAX_EVIDENCE: usize = 40;
const MAX_CHANGED_DOCS: usize = 12;
const MAX_CHANGED_SOURCE_FILES: usize = 12;
const GIT_TIMEOUT: Duration = Duration::from_secs(12);

const DEFAULT_BANNED: [&str; 9] = [
    "颠覆",
    "革命性",
    "行业第一",
    "全球领先",
    "完美",
    "彻底解决",
    "零风险",
    "史诗级",
    "遥遥领先",
];

fn truncate(input: &str, max_chars: usize) -> (String, bool) {
    if input.chars().count() <= max_chars {
        return (input.to_string(), false);
    }
    (
        input.chars().take(max_chars).collect::<String>() + "\n…(truncated)",
        true,
    )
}

fn stable_hash(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn drain_bounded<R: Read>(mut reader: R) -> std::io::Result<Vec<u8>> {
    let mut stored = Vec::with_capacity(MAX_GIT_OUTPUT.min(8_192));
    let mut buffer = [0_u8; 8_192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = MAX_GIT_OUTPUT.saturating_sub(stored.len());
        stored.extend_from_slice(&buffer[..read.min(remaining)]);
    }
    Ok(stored)
}

fn run_git(workspace: &Path, args: &[&str]) -> AppResult<String> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法运行 git：{e}"))?;
    let stdout = child.stdout.take().ok_or("无法读取 git stdout")?;
    let stderr = child.stderr.take().ok_or("无法读取 git stderr")?;
    let stdout_reader = thread::spawn(move || drain_bounded(stdout));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("等待 git 失败：{e}"))?
        {
            break status;
        }
        if started.elapsed() >= GIT_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(format!(
                "git {} 超过 {} 秒，已终止",
                args.join(" "),
                GIT_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "读取 git stdout 失败".to_string())?
        .map_err(|e| format!("读取 git stdout 失败：{e}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "读取 git stderr 失败".to_string())?
        .map_err(|e| format!("读取 git stderr 失败：{e}"))?;
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr);
        return Err(format!("git {} 失败：{}", args.join(" "), stderr.trim()));
    }
    let raw = String::from_utf8_lossy(&stdout);
    Ok(truncate(&raw, MAX_GIT_OUTPUT).0.trim().to_string())
}

fn safe_repo_file(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || relative.contains('\0') {
        return None;
    }
    let candidate = root.join(relative);
    let canonical = candidate.canonicalize().ok()?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return None;
    }
    Some(canonical)
}

fn read_bounded(path: &Path) -> Option<(String, bool)> {
    let bytes = fs::read(path).ok()?;
    let truncated = bytes.len() > MAX_FILE_BYTES;
    let slice = &bytes[..bytes.len().min(MAX_FILE_BYTES)];
    if slice.iter().take(1024).any(|byte| *byte == 0) {
        return None;
    }
    Some((String::from_utf8_lossy(slice).to_string(), truncated))
}

fn evidence(
    next: usize,
    kind: &str,
    source: impl Into<String>,
    excerpt: String,
    release_state: &str,
) -> Evidence {
    Evidence {
        id: format!("ev-{next:03}"),
        kind: kind.into(),
        source: source.into(),
        content_hash: stable_hash(&excerpt),
        excerpt,
        release_state: release_state.into(),
    }
}

fn is_sensitive_or_generated(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let normalized = lower.replace('\\', "/");
    let filename = normalized.rsplit('/').next().unwrap_or(&normalized);
    let denied_segment = normalized.split('/').any(|segment| {
        matches!(
            segment,
            ".git" | "node_modules" | "target" | "dist" | "build" | "vendor" | ".pnpm-store"
        )
    });
    denied_segment
        || filename.starts_with(".env")
        || filename.contains("credential")
        || filename.contains("secret")
        || filename.contains("private-key")
        || filename.ends_with(".pem")
        || filename.ends_with(".key")
        || matches!(
            filename,
            "pnpm-lock.yaml" | "package-lock.json" | "yarn.lock" | "cargo.lock"
        )
}

fn is_document_path(path: &str) -> bool {
    if is_sensitive_or_generated(path) {
        return false;
    }
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md")
        || lower.ends_with(".mdx")
        || lower.ends_with(".txt")
        || lower.ends_with(".rst")
        || lower == "readme"
        || lower == "changelog"
}

fn is_safe_source_path(path: &str) -> bool {
    if is_sensitive_or_generated(path) {
        return false;
    }
    let lower = path.to_ascii_lowercase();
    [
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".kt", ".swift", ".css",
        ".html", ".json", ".toml", ".yaml", ".yml", ".md", ".mdx", ".rst",
    ]
    .iter()
    .any(|extension| lower.ends_with(extension))
}

fn load_config(root: &Path) -> MarketingConfig {
    let read_config = |relative: &str| {
        safe_repo_file(root, relative).and_then(|path| read_bounded(&path).map(|value| value.0))
    };
    let project_context = read_config("docs/marketing/project-context.md").unwrap_or_default();
    let brand_guide = read_config("docs/marketing/brand-guide.md").unwrap_or_default();
    let mut channel_templates = BTreeMap::new();
    for channel in REQUIRED_CHANNELS {
        let filename = match channel {
            "x" => "x-twitter.md",
            "bilibili" => "bilibili-script.md",
            "github_release" => "github-release.md",
            other => other,
        };
        let filename = if filename.ends_with(".md") {
            filename.to_string()
        } else {
            format!("{filename}.md")
        };
        if let Some(body) = read_config(&format!("docs/marketing/channels/{filename}")) {
            channel_templates.insert(channel.to_string(), body);
        }
    }
    let mut banned_phrases = DEFAULT_BANNED
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    for line in brand_guide.lines() {
        let trimmed = line.trim().trim_start_matches(['-', '*', ' ']);
        if let Some(value) = trimmed
            .strip_prefix("禁用词：")
            .or_else(|| trimmed.strip_prefix("禁用词:"))
        {
            banned_phrases.extend(
                value
                    .split([',', '，', '、'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            );
        }
    }
    banned_phrases.sort();
    banned_phrases.dedup();
    MarketingConfig {
        project_context,
        brand_guide,
        channel_templates,
        banned_phrases,
    }
}

pub fn collect_repository_snapshot(
    workspace: &Path,
    source_mode: &str,
    requested_base: Option<&str>,
) -> AppResult<RepositorySnapshot> {
    if !workspace.is_dir() {
        return Err("Self-Marketing 只支持存在的项目工作区。".into());
    }
    let root_raw = run_git(workspace, &["rev-parse", "--show-toplevel"])?;
    let root = PathBuf::from(root_raw)
        .canonicalize()
        .map_err(|e| format!("无法解析 Git 根目录：{e}"))?;
    let head_ref = run_git(&root, &["rev-parse", "HEAD"])?;
    let base_ref = if let Some(base) = requested_base.map(str::trim).filter(|s| !s.is_empty()) {
        run_git(
            &root,
            &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
        )?;
        Some(base.to_string())
    } else {
        run_git(&root, &["describe", "--tags", "--abbrev=0", "HEAD"]).ok()
    };

    let log_range = base_ref
        .as_ref()
        .map(|base| format!("{base}..HEAD"))
        .unwrap_or_else(|| "HEAD".into());
    let log_raw = run_git(&root, &["log", "--format=%H%x09%s", "-n", "20", &log_range])?;
    let commits = log_raw
        .lines()
        .filter_map(|line| {
            let (sha, subject) = line.split_once('\t')?;
            Some(CommitSummary {
                sha: sha.into(),
                subject: subject.into(),
            })
        })
        .collect::<Vec<_>>();

    let changed_raw = if let Some(base) = &base_ref {
        run_git(&root, &["diff", "--name-only", &format!("{base}..HEAD")])?
    } else {
        run_git(
            &root,
            &["log", "--format=", "--name-only", "-n", "20", "HEAD"],
        )?
    };
    let mut changed_files = changed_raw
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !is_sensitive_or_generated(s))
        .map(str::to_string)
        .collect::<Vec<_>>();
    changed_files.sort();
    changed_files.dedup();

    let include_uncommitted = source_mode == "include_uncommitted";
    let status_raw = if include_uncommitted {
        run_git(
            &root,
            &["status", "--porcelain", "--untracked-files=normal"],
        )?
    } else {
        String::new()
    };
    let mut uncommitted_files = status_raw
        .lines()
        // run_git trims the whole stdout, so the first porcelain line may lose
        // its leading index-space; slicing after the two status columns works
        // for both `M file` and `?? file`, then trim removes the separator.
        .filter_map(|line| line.get(2..))
        .map(|path| {
            path.rsplit(" -> ")
                .next()
                .unwrap_or(path)
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty() && !is_sensitive_or_generated(s))
        .collect::<Vec<_>>();
    uncommitted_files.sort();
    uncommitted_files.dedup();

    let mut all_evidence = Vec::new();
    let mut truncated = false;
    for commit in &commits {
        all_evidence.push(evidence(
            all_evidence.len() + 1,
            "commit",
            &commit.sha,
            commit.subject.clone(),
            "committed",
        ));
    }

    let mut document_candidates = vec![
        "README.md".to_string(),
        "README.MD".to_string(),
        "CHANGELOG.md".to_string(),
        "CHANGELOG.MD".to_string(),
    ];
    document_candidates.extend(
        changed_files
            .iter()
            .filter(|path| is_document_path(path))
            .take(MAX_CHANGED_DOCS)
            .cloned(),
    );
    if include_uncommitted {
        document_candidates.extend(
            uncommitted_files
                .iter()
                .filter(|path| is_document_path(path))
                .take(MAX_CHANGED_DOCS)
                .cloned(),
        );
    }
    let mut seen = HashSet::new();
    for relative in document_candidates {
        if all_evidence.len() >= MAX_EVIDENCE || !seen.insert(relative.to_ascii_lowercase()) {
            continue;
        }
        let Some(path) = safe_repo_file(&root, &relative) else {
            continue;
        };
        let Some((body, was_truncated)) = read_bounded(&path) else {
            continue;
        };
        truncated |= was_truncated;
        let state = if include_uncommitted && uncommitted_files.iter().any(|p| p == &relative) {
            "unreleased"
        } else {
            "committed"
        };
        all_evidence.push(evidence(
            all_evidence.len() + 1,
            "document",
            relative,
            body,
            state,
        ));
    }

    for relative in changed_files
        .iter()
        .filter(|path| is_safe_source_path(path))
        .take(MAX_CHANGED_SOURCE_FILES)
    {
        if all_evidence.len() >= MAX_EVIDENCE {
            break;
        }
        let raw = if let Some(base) = &base_ref {
            run_git(
                &root,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--unified=2",
                    &format!("{base}..HEAD"),
                    "--",
                    relative,
                ],
            )?
        } else {
            run_git(
                &root,
                &["show", "--format=", "--unified=2", "HEAD", "--", relative],
            )?
        };
        let (diff, was_truncated) = truncate(&raw, 8_000);
        truncated |= was_truncated;
        if !diff.trim().is_empty() {
            all_evidence.push(evidence(
                all_evidence.len() + 1,
                "diff",
                relative,
                diff,
                "committed",
            ));
        }
    }
    if include_uncommitted {
        for relative in uncommitted_files
            .iter()
            .filter(|path| is_safe_source_path(path))
            .take(MAX_CHANGED_SOURCE_FILES)
        {
            if all_evidence.len() >= MAX_EVIDENCE {
                break;
            }
            let mut raw = run_git(
                &root,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--unified=2",
                    "HEAD",
                    "--",
                    relative,
                ],
            )?;
            if raw.trim().is_empty() {
                if let Some(path) = safe_repo_file(&root, relative) {
                    raw = read_bounded(&path).map(|value| value.0).unwrap_or_default();
                }
            }
            let (diff, was_truncated) = truncate(&raw, 6_000);
            truncated |= was_truncated;
            if !diff.trim().is_empty() {
                all_evidence.push(evidence(
                    all_evidence.len() + 1,
                    "diff",
                    relative,
                    diff,
                    "unreleased",
                ));
            }
        }
    }

    Ok(RepositorySnapshot {
        schema_version: 1,
        repository_root: root.to_string_lossy().into_owned(),
        base_ref,
        head_ref,
        source_mode: if include_uncommitted {
            "include_uncommitted".into()
        } else {
            "committed".into()
        },
        commits,
        changed_files,
        uncommitted_files,
        evidence: all_evidence,
        config: load_config(&root),
        truncated,
        collected_at: now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn committed_snapshot_is_bounded_and_traceable() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init"]);
        git(dir.path(), &["config", "user.email", "t@example.com"]);
        git(dir.path(), &["config", "user.name", "tester"]);
        fs::write(dir.path().join("README.md"), "# Demo\nLocal-first panel").unwrap();
        git(dir.path(), &["add", "README.md"]);
        git(dir.path(), &["commit", "-m", "feat: add demo"]);
        let snapshot = collect_repository_snapshot(dir.path(), "committed", None).unwrap();
        assert_eq!(snapshot.commits.len(), 1);
        assert!(snapshot.evidence.iter().any(|e| e.kind == "commit"));
        assert!(snapshot.evidence.iter().any(|e| e.source == "README.md"));
        assert!(snapshot
            .evidence
            .iter()
            .all(|e| e.content_hash.starts_with("fnv1a64:")));
    }

    #[test]
    fn uncommitted_content_requires_explicit_mode() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init"]);
        git(dir.path(), &["config", "user.email", "t@example.com"]);
        git(dir.path(), &["config", "user.name", "tester"]);
        let mut file = fs::File::create(dir.path().join("README.md")).unwrap();
        writeln!(file, "first").unwrap();
        drop(file);
        git(dir.path(), &["add", "README.md"]);
        git(dir.path(), &["commit", "-m", "init"]);
        fs::write(dir.path().join("README.md"), "unreleased").unwrap();
        let committed = collect_repository_snapshot(dir.path(), "committed", None).unwrap();
        assert!(committed.uncommitted_files.is_empty());
        let dirty = collect_repository_snapshot(dir.path(), "include_uncommitted", None).unwrap();
        assert!(dirty.uncommitted_files.iter().any(|p| p == "README.md"));
        assert!(dirty
            .evidence
            .iter()
            .any(|e| e.release_state == "unreleased"));
    }

    #[test]
    fn sensitive_and_generated_paths_are_never_collected() {
        for path in [
            ".env",
            ".env.local",
            "config/credentials.json",
            "secrets/private-key.pem",
            "node_modules/pkg/index.js",
            "src-tauri/target/debug/app.exe",
            "pnpm-lock.yaml",
        ] {
            assert!(
                is_sensitive_or_generated(path),
                "expected {path} to be denied"
            );
            assert!(!is_safe_source_path(path), "expected {path} to be unsafe");
        }
        assert!(!is_sensitive_or_generated("src/marketing/types.ts"));
        assert!(is_safe_source_path("src/marketing/types.ts"));

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.md");
        fs::write(&secret, "do not read").unwrap();
        assert!(safe_repo_file(root.path(), &secret.to_string_lossy()).is_none());
    }
}
