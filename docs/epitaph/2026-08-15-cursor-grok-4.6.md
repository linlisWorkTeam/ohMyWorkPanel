---
date: 2026-08-15
topic: cursor-grok-4.6-catalog
branch: master
status: active
---

# Epitaph: Cursor Agent 模型目录加入 Grok 4.6

## Built this session
- 对照本机 `cursor-agent --list-models`，在前后端目录加入 `cursor-grok-4.6-*`（含 xhigh / high / medium / low 及 `-fast`）
- 保留 4.5 条目，避免已选成员失效；默认仍为 `auto`
- 单测：`agentModels.test.ts` + `adapters::models::cursor_catalog_includes_grok_and_kimi`

## Key files
| 文件 | 说明 |
|---|---|
| `src/agentModels.ts` | 前端下拉 |
| `src-tauri/src/adapters/models.rs` | 后端目录（与 FE 同步） |

## Do not regress
- FE/Rust 目录必须同步；以 `cursor-agent --list-models` 为准（账号可见集可能不同）
- 勿删已有 4.5 id（存量 `agent_profiles.model`）

## How to verify
```bash
cursor-agent --list-models | grep 4.6
pnpm run test:gate
# 灰度成员面板选 cursor-grok-4.6-high-fast 发一条 @
```
