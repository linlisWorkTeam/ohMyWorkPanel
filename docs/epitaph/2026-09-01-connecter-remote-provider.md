---
date: 2026-09-01
topic: connecter-remote-provider
branch: codex/connecter-provider-main
status: active
---

# Epitaph: Connecter 远程 Agent Provider

## 已完成证据

- 早期基线实现了非 CLI provider `connecter-remote`，通过 Connecter `/v2/dispatches` create/get/cancel 调度远端 Runner。
- 真实本地 E2E 已验证 WorkPanel → Connecter → Windows 本机 Codex Runner → WorkPanel 单次回写；证明值为 `WORKPANEL_CONNECTER_REMOTE_E2E_a9965ff6|workpanel-connecter|0.2.3`。
- service bearer 通过现有 AES-GCM secret 机制加密，Member DTO、日志和 argv 均不返回密钥；dispatch 固定 `writeBack=false`。
- `Idempotency-Key` 使用 WorkPanel run UUID，`task_runs.provider_dispatch_id` 支持取消、诊断和重启恢复。

## 最新 main 移植边界

- Rust adapter 移入 `src-tauri/src/agents/adapters/`，前端 catalog/测试移入 `src/agents/`，不回退代码目录治理。
- 保留 v2.1.2 custom chatbot `api_url`、Shell/UI atoms、更新检查和现有 IPC；Provider 字段只做兼容加法。
- schema v6 同时兼容 main-v4、早期 provider-v4，以及已占用 v5 但缺少 provider schema 的数据库；v5 版本号碰撞由 ECS canary 实测发现。

## 最新 main 验证（2026-09-02）

- Git for Windows Bash 直接执行 `scripts/test-gate.sh`：通过。
- 前端：36 files / 131 tests；TypeScript、桌面 build、Web build、颜色门禁均通过。
- Rust：157 tests；server 与 GUI `cargo check` 均通过。
- AI Harness fast checks、Markdown links 与 Extension Host purity：通过。
- schema v6 的 fresh、legacy、second-boot、main-v4、provider-v4、foreign-main-v5 汇合测试均通过。
- ECS Cloud Assistant 只读调用 ECS Codex 成功：`ECS_CODEX_READY|branch=master|head=ef6c741306c5668b17a01f137be27037162607cb`。

## 当前待办

- 创建 PR 并锁定 immutable commit。
- ECS Codex 从 PR/immutable commit 自举 WorkPanel canary 与 Connecter Host canary。
- 在 ECS `:8081` UI 直接 `@Codex-Windows11` 验证本机回复且只产生一条 Agent 消息。

## 不得回退

- 不得把 `connecter-remote` 塞入 CLI manifest 或在 ECS 本机 spawn 被称为“远端”的 Agent。
- 不得复用 `agent_profiles.api_key` 保存 service bearer，不得把 bearer 放入 curl argv、日志或 Member DTO。
- 不得允许 `writeBack=true`；WorkPanel 必须是群消息唯一写入者。
- 不得直接 promote `:8080`；先完成 `:8081` UI 直调与单消息验收。
