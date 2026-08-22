# Design: Cursor 模型目录定时同步（方案 A）

**日期**: 2026-08-15  
**状态**: 实现中  
**范围**: 仅 Cursor；其它适配器标 TODO

## 目标

新模型上线后无需手改 `agentModels.ts` / `models.rs`：服务端定时跑 `cursor-agent --list-models`，缓存供 API / 前端下拉使用；失败则回退静态目录。

## 行为

| 项 | 约定 |
|---|---|
| 触发 | 进程启动立即刷一次；之后默认每 **6h**（`OHMYWORKPANEL_CURSOR_MODEL_SYNC_SECS`，`0`=只启动刷一次） |
| 命令 | `cursor-agent` / `agent` `--list-models`，超时 45s，`spawn_blocking` |
| 解析 | `id - label` 行；跳过标题/Tip |
| 合并 | live 非空 → 用 live（保留 CLI 顺序）；否则静态 fallback |
| API | `GET /api/agent-models`（需登录）；`POST /api/agent-models/refresh`（admin） |
| FE | 登录后拉取并覆盖 cursor 目录；其它 adapter 仍用静态 |
| 内存 | 长间隔；同时只允许一次 refresh |

## TODO（非本切片）

- Codex / Claude / OpenClaw / OpenCode 的 list-models 同步
- 将 live 目录持久化到 SQLite（跨重启冷启动更快）
- 账号差异告警（本机 CLI 可见集 ≠ 成员账号）

## 风险

- 本机 1.8G：list-models 瞬时 RSS 可能 ~100–200Mi，故间隔宜长
- CLI 未登录时 live 失败 → 静默 fallback
