//! Ops（发布状态 / 门禁 / 灰度）IPC 命令（P4 入口瘦身）。
//!
//! 从 commands.rs 拆出；对外路径仍为 commands::<name>（由 mod.rs 重导出）。

use super::*;

#[tauri::command]
pub async fn ops_release_status() -> crate::ops::ReleaseStatus {
    crate::ops::release_status().await
}

#[tauri::command]
pub fn ops_job_status() -> crate::ops::OpsJobState {
    crate::ops::ops_job_snapshot()
}

#[tauri::command]
pub fn ops_run_test_gate() -> AppResult<()> {
    crate::ops::kickoff_test_gate()
}

#[tauri::command]
pub fn ops_deploy_canary() -> AppResult<()> {
    crate::ops::kickoff_deploy_canary()
}
