---
date: 2026-08-05
topic: extension-host-panellive
branch: master
status: active
---

# Epitaph: Extension Host + PanelLive / A2A（平台侧）

## Built

- `extensions`：读 PanelLive manifest；`group_extensions` 表；load 前 health 探活 `:8790`
- API：`GET/PUT` 群扩展；`POST /api/a2a/dispatch`（禁 PCM）
- UI：运行设置 Live 开关；主视图 Live 页签 → iframe PanelLive
- 单测：manifest / enable 持久化 / A2A 拒 PCM / 前端 helpers

## Do not regress

- 音频不进 A2A / 群消息体
- load 失败不得误标 enabled
- HTTPS 页嵌 `http://127.0.0.1:8790` 会混合内容拦截 — 真云阶段需同域反代

## Verify

```bash
cd /AI/WorkPanelLive && npm start &
npm run smoke
# ohMyWorkPanel canary: 开启 Live → 页签可进；dispatch live.transcribe.result 无 pcm
```
