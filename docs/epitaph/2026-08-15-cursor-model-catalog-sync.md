---
date: 2026-08-15
topic: cursor-model-catalog-sync
branch: master
status: active
---

# Epitaph: Cursor 模型目录定时同步（方案 A）

## Built this session
- `model_catalog`：解析 `cursor-agent --list-models`；启动刷一次 + 默认 6h（`OHMYWORKPANEL_CURSOR_MODEL_SYNC_SECS`）
- API：`GET /api/agent-models`、`POST /api/agent-models/refresh`（admin）
- FE：登录 bootstrap 拉取并 `applyAgentModelsPayload`；静态目录作 fallback
- 其它适配器 live sync 写入 response `todos` / 设计文档 TODO

## Key files
| 文件 | 说明 |
|---|---|
| `src-tauri/src/model_catalog.rs` | 解析/缓存/定时 |
| `src-tauri/src/main_server.rs` | 启动 loop |
| `src/agentModels.ts` | live overlay |
| `docs/superpowers/specs/2026-08-15-cursor-model-catalog-sync-design.md` | 设计 |

## Do not regress
- CLI 失败必须 fallback，不得清空下拉
- 勿在 1.8G 机上把同步间隔调到分钟级

## How to verify
```bash
pnpm run test:gate
# canary:
curl -sS -H "Authorization: Bearer $TOK" http://127.0.0.1:8081/api/agent-models | head
```
