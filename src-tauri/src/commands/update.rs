//! 自动更新 IPC 命令（P4 入口瘦身）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use super::*;

#[tauri::command]
pub async fn check_for_update(
    manifest_url: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<UpdateCheckResult> {
    let state = state.inner().clone();
    let current = env!("CARGO_PKG_VERSION").to_string();
    let pinned = manifest_url
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());
    let candidates: Vec<String> = match &pinned {
        Some(url) => vec![url.clone()],
        // 未配置：自动发现（依次尝试默认源，静默跳过失败源）
        None => DEFAULT_MANIFEST_CANDIDATES
            .iter()
            .map(|s| s.to_string())
            .collect(),
    };
    let mut last_error: Option<String> = None;
    for url in &candidates {
        let output = match tokio::process::Command::new("curl")
            .args(["-sS", "--max-time", "5", "-L", url])
            .output()
            .await
        {
            Ok(o) => o,
            Err(e) => {
                last_error = Some(format!("无法请求清单：{e}"));
                continue;
            }
        };
        if !output.status.success() {
            last_error = Some(format!("HTTP 非 2xx（{url}）"));
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                last_error = Some(format!("清单不是合法 JSON（{url}）：{e}"));
                continue;
            }
        };
        let Some(latest) = value
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
        else {
            last_error = Some(format!("清单缺少 version 字段（{url}）"));
            continue;
        };
        let notes = value
            .get("notes")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let download_url = value
            .get("url")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let sha256 = value
            .get("sha256")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let has_update = crate::models::is_newer_version(&latest, &current);
        if let Ok(conn) = open_db(&state.db_path) {
            logger::info(
                &conn,
                "updater",
                &format!(
                    "check: source={url} current={current} latest={latest} has_update={has_update}"
                ),
                None,
            );
        }
        return Ok(UpdateCheckResult {
            enabled: true,
            current_version: current,
            latest_version: Some(latest),
            has_update,
            notes,
            download_url,
            sha256,
            error: None,
        });
    }
    // 全部候选源失败：静默禁用（不打扰用户），保留首个错误供设置页排查
    if let Ok(conn) = open_db(&state.db_path) {
        logger::info(
            &conn,
            "updater",
            &format!(
                "check: all sources failed{}",
                last_error
                    .as_ref()
                    .map(|e| format!(" -> {e}"))
                    .unwrap_or_default()
            ),
            None,
        );
    }
    Ok(UpdateCheckResult {
        enabled: false,
        current_version: current,
        latest_version: None,
        has_update: false,
        notes: None,
        download_url: None,
        sha256: None,
        error: last_error,
    })
}

/// 下载安装包到 %TEMP%\ohmyworkpanel-update（可选 sha256 校验，失败即删除并报错）。
#[tauri::command]
pub async fn download_update(url: String, expected_sha256: Option<String>) -> AppResult<String> {
    let dir = std::env::temp_dir().join("ohmyworkpanel-update");
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建下载目录：{e}"))?;
    let path = dir.join(format!("ohMyWorkPanel-{}.exe", id()));
    let out = tokio::process::Command::new("curl")
        .args([
            "-sSL",
            "--max-time",
            "600",
            "-o",
            path.to_str().unwrap_or(""),
            &url,
        ])
        .output()
        .await
        .map_err(|e| format!("下载启动失败：{e}"))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&path);
        return Err(format!(
            "下载失败（HTTP/网络）：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if size < 1_000_000 {
        let _ = std::fs::remove_file(&path);
        return Err("下载文件异常（过小）".into());
    }
    if let Some(expected) = expected_sha256
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
    {
        let got = sha256_of_file(&path)?;
        if got != expected {
            let _ = std::fs::remove_file(&path);
            return Err(format!(
                "下载校验失败：哈希不匹配（期望 {expected}，实际 {got}）"
            ));
        }
    }
    Ok(path.to_string_lossy().into_owned())
}

/// 计算文件 SHA256（Windows certutil，系统自带，避免新增依赖）。
fn sha256_of_file(path: &std::path::Path) -> Result<String, String> {
    let out = std::process::Command::new("certutil")
        .args(["-hashfile", path.to_str().unwrap_or(""), "SHA256"])
        .output()
        .map_err(|e| format!("certutil 启动失败：{e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let t = line.trim();
        if t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(t.to_ascii_lowercase());
        }
    }
    Err(format!(
        "certutil 输出无法解析：{}",
        text.chars().take(120).collect::<String>()
    ))
}

/// 退出应用并静默安装：以独立进程启动安装器（/S + 显式 /D= 安装位），随后本进程退出。
#[tauri::command]
pub fn exit_and_install(installer_path: String, app: tauri::AppHandle) -> AppResult<()> {
    #[cfg(not(windows))]
    {
        let _ = (installer_path, app);
        Err("仅 Windows 桌面版支持自动安装".into())
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        let target = std::env::var("LOCALAPPDATA")
            .map(|d| std::path::Path::new(&d).join("ohMyWorkPanel"))
            .unwrap_or_else(|_| std::path::PathBuf::from(r"C:\ohMyWorkPanel"));
        let mut cmd = std::process::Command::new(&installer_path);
        cmd.args(["/S", &format!("/D={}", target.display())]);
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP：独立运行，不挂本进程控制台
        cmd.creation_flags(0x00000008 | 0x00000200);
        cmd.spawn().map_err(|e| format!("启动安装器失败：{e}"))?;
        app.exit(0);
        Ok(())
    }
}
