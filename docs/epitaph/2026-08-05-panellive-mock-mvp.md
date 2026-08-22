---
date: 2026-08-05
topic: panellive-mock-mvp
branch: master
status: active
---

# Epitaph: PanelLive Mock MVP + 平台路线图 A2A

## Built this session

- **PanelLive Mock MVP** at `/AI/WorkPanelLive`：HTTP mock STT/TTS + Live 页；`npm run smoke` 通过；默认 `127.0.0.1:8790`
- **平台契约**：`docs/panellive-platform-requirements.md`；`docs/roadmap.md` 新增 v0.5
- **设计/计划**：`docs/superpowers/specs|plans/2026-08-05-panellive-mock-mvp*`
- **群通知（prod :8080 / ohMyWorkPanel）**：3 条 roadmap todo；A2A 消息 `@Codex @Cursor Agent @OpenClaw`（message `00980bb7-…`，3 个 runIds）

## Locked product decisions

| 项 | 选择 |
|---|---|
| MVP | 方案 A Mock，非真云 |
| 云 STT/TTS | PanelLive 直连（真云阶段）；Connecter 不转音频 |
| A2A | 仅文本控制面，禁 PCM |

## How to run / verify

```bash
cd /AI/WorkPanelLive && npm start
# other terminal
cd /AI/WorkPanelLive && npm run smoke
```

## Do not regress

- 勿把音频转发塞回 Connecter
- 勿在未灰度情况下把真云 Key 写进仓库

## Open follow-ups

- ohMyWorkPanel Extension Host / Live 页签 / A2A skills 由平台 Agent 认领
- 下一刀：dashscope 真 STT/TTS
