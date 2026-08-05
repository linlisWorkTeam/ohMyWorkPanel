# Epitaph: Live 会话态 + 聊天记录一致

**Date**: 2026-08-05  
**Status**: active（灰度）

## Problem

1. iframe 直连 PanelLive `session/start`，平台 `live_sessions` 不更新 → 短回复注入永不生效  
2. Live UI 自建气泡，与群聊脱节  
3. 灰度 `http://公网IP` → `getUserMedia` TypeError（非安全上下文）

## Fix

- 代理识别 `X-Linlis-Group-Id` 更新 `live_sessions`
- `LivePanel` + `liveBridge`：同步聊天 / STT 写入群聊 / 回复 TTS
- PanelLive `live.html`：mic 探测、去 Mock 短路 chatbot、与宿主 postMessage

## Risks

- 非 HTTPS 仍无法录音（需运维上 TLS 或本机 localhost）
- STT 云失败会回落 mock 文本
- PanelLive 静态文件改在 `/AI/WorkPanelLive`，需 PanelLive 进程能读到新 `public/live.html`
