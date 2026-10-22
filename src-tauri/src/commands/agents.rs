//! Agent 检测 / OCR / 预设角色 / 成员 API Key IPC 命令（P4 入口瘦身）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use super::*;

#[tauri::command]
pub async fn detect_agent(member_id: String, state: State<'_, AppState>) -> AppResult<String> {
    let conn = open_db(&state.db_path)?;
    let record = conn
        .query_row(
            "SELECT p.adapter,COALESCE(p.executable_path,''),m.group_id FROM agent_profiles p JOIN members m ON m.id=p.member_id WHERE p.member_id=?1",
            params![member_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "找不到 Agent 配置。".to_string())?;
    if crate::agents::adapters::connecter_remote::is_connecter_remote(&record.0) {
        let profile = crate::db::get_connecter_provider_profile(&conn, &member_id)?;
        crate::agents::adapters::connecter_remote::validate_stored_profile(&profile)?;
        conn.execute(
            "UPDATE agent_profiles SET runtime_status='ready',updated_at=?1 WHERE member_id=?2",
            params![now(), member_id],
        )
        .map_err(|e| e.to_string())?;
        return Ok("ready".into());
    }
    let spec = crate::adapters::manifest::resolve_adapter(&record.0)?;
    if spec.builtin_kind() == Some(AdapterKind::Mock) {
        conn.execute(
            "UPDATE agent_profiles SET runtime_status='ready',updated_at=?1 WHERE member_id=?2",
            params![now(), member_id],
        )
        .map_err(|e| e.to_string())?;
        return Ok("ready".into());
    }
    let configured = if record.1.trim().is_empty() {
        None
    } else {
        Some(record.1.as_str())
    };
    let executable = spec.resolve_executable(configured)?;
    let status = match tokio::time::timeout(
        Duration::from_secs(5),
        Command::new(&executable).arg("--version").output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => "ready",
        _ => "unavailable",
    };
    conn.execute(
        "UPDATE agent_profiles SET runtime_status=?1,updated_at=?2 WHERE member_id=?3",
        params![status, now(), member_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(status.into())
}

#[tauri::command]
pub fn ocr_image(image_path: String) -> AppResult<String> {
    crate::ocr::ocr_image(&image_path)
}

/// OCR from a base64-encoded image (e.g. clipboard paste)
#[tauri::command]
pub fn ocr_image_base64(base64_data: String) -> AppResult<String> {
    crate::ocr::ocr_image_base64(&base64_data)
}

#[tauri::command]
pub fn get_preset_roles_command(state: State<'_, AppState>) -> AppResult<Vec<PresetRole>> {
    get_preset_roles(&open_db(&state.db_path)?)
}

// === PM: Roadmap Items ===

#[tauri::command]
pub async fn set_member_api_key(
    member_id: String,
    api_key: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Member> {
    let state = state.inner().clone();
    let conn = open_db(&state.db_path)?;
    assert_member_mutable(&conn, &member_id)?;
    crate::db::set_member_api_key(&conn, &member_id, api_key.as_deref())?;
    conn.query_row(
        &format!("{} WHERE m.id=?1", crate::db::MEMBER_SELECT),
        params![member_id],
        member_from_row,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_member_api_url(
    member_id: String,
    api_url: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Member> {
    let state = state.inner().clone();
    crate::adapters::chatbot::validate_api_url(api_url.as_deref())?;
    let conn = open_db(&state.db_path)?;
    crate::db::assert_member_mutable(&conn, &member_id)?;
    crate::db::set_member_api_url(&conn, &member_id, api_url.as_deref())?;
    conn.query_row(
        &format!("{} WHERE m.id=?1", crate::db::MEMBER_SELECT),
        params![member_id],
        member_from_row,
    )
    .map_err(|e| e.to_string())
}
