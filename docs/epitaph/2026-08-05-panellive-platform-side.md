---
date: 2026-08-05
topic: panellive-platform-side
branch: master
status: active
---

# Epitaph: PanelLive 平台侧 T3 同源代理 + A2A events

## Aligned defaults

- **A1**：`live.transcribe.result` → WS `live_event` only，不进群消息
- **A2**：`live.session.stop` → PanelLive `/v1/session/cancel`
- **A3**：Live 页签群成员可见；开关仅 admin

## Built

- 同源代理 `GET/POST /api/extensions/panellive/{*path}` → `127.0.0.1:8790`（无 JWT）
- `POST .../events` + `X-Panellive-Token`（默认 `panellive-dev-token` / env `OHMYWORKPANEL_PANELLIVE_TOKEN`）
- `baseUrl` 改为 `/api/extensions/panellive`；PanelLive `live.html` 自动加 API_BASE 前缀
- enable 未就绪 → **409**

## Do not regress

- 禁止 iframe 直连 127.0.0.1:8790
- A2A/events 禁 PCM；synthesize 不把 audioBase64 回传控制面

## Verify

```bash
curl -sS http://127.0.0.1:8081/api/extensions/panellive/live.html | head
curl -sS -X POST http://127.0.0.1:8081/api/extensions/panellive/events \
  -H 'X-Panellive-Token: panellive-dev-token' -H 'Content-Type: application/json' \
  -d '{"skill":"live.transcribe.result","payload":{"text":"hi","isFinal":true}}'
```
