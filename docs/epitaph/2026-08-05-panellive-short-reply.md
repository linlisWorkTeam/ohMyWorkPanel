# Epitaph: PanelLive 短回复注入（§6）

**Date**: 2026-08-05  
**Status**: active（灰度）

## What

对齐 `docs/superpowers/plans/2026-08-05-live-short-reply-injection.md`：

- **T1** `SchedulerState.live_sessions`：`live.session.start/stop/cancel` 钩子
- **T2** `live_prompt.rs`：1.5s 超时 + 60s 缓存 + fallback
- **T3** 仅会话激活群 + chatbot/管理员 Agent 注入

扩展 enable ≠ 会话激活；关 session 后恢复长回复。

## Files

- `src-tauri/src/live_prompt.rs`
- `src-tauri/src/a2a.rs` / `scheduler.rs` / `web.rs`
- `docs/panellive-platform-requirements.md` §6

## Risks

- 内存态重启丢失（与 PanelLive 会话一致）
- TTS 硬截断在 PanelLive；平台只注入提示词
- Key / COSYVOICE_VOICE 勿入库
