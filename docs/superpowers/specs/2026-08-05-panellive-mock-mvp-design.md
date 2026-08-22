---
date: 2026-08-05
topic: panellive-mock-mvp
status: active
---

# PanelLive Mock MVP Design

## Goal

交付可运行的 **PanelLive Extend 服务骨架（方案 A）**：本地 Mock STT/TTS + 类微信语音页，并沉淀对 ohMyWorkPanel 平台的 Extension / A2A 要求。不在本 MVP 调用真实云 STT/TTS。

## Locked decisions

| 项 | 选择 |
|---|---|
| 云调用归属 | **PanelLive 直连**（真云阶段）；Connecter 不转音频 |
| MVP 云 | Mock（假识别 / 假合成） |
| 代码位置 | `/AI/WorkPanelLive` |
| 平台要求文档 | `ohMyWorkPanel/docs` + 群路线图 + A2A 群消息 |
| 协议 | 控制面 A2A skills；媒体面独立（本 MVP 仅本地 HTTP） |

## MVP scope

1. Node 单进程服务：`/health`、`/v1/session/*`、Mock STT/TTS、静态 Live UI  
2. Extension 清单：`extension.manifest.json`（页签贡献点 + skills 声明）  
3. 对 ohMyWorkPanel 的 API/协议需求清单
4. ohMyWorkPanel 群路线图写入平台待办，并用 `@` 通知 Agent

## Non-goals

- 真实 dashscope / NLS  
- ohMyWorkPanel Extension Host 完整实现（由平台侧后续做）
- Connecter 媒体中转  

## Success criteria

- `npm start` 后可打开 Live 页，按住说话 → 看到 Mock 文本 → 听到/下载 Mock 音频  
- docs 与路线图中平台要求可被 ohMyWorkPanel Agent 认领
