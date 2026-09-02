---
date: 2026-09-01
topic: connecter-remote-provider
status: active
track: A
environment: canary-only
---

# Connecter 远程 Agent Provider 设计

## 目标

让 WorkPanel 群成员使用 `connecter-remote` provider，经 Connecter 的 Directory/Federation 调度其他设备上的 Runner。首个验收对象是 Windows 设备上的 Codex Runner，目标环境仅为 ECS canary `:8081`。

## 非目标与边界

- 本切片不把 HTTP provider 伪装成 CLI manifest，也不在 ECS 本机 spawn Codex。
- WorkPanel 是群消息与本地 run 终态的唯一写入者；Connecter dispatch 必须固定 `writeBack=false`。
- 不部署或 promote 生产 `:8080`。
- Provider 不解决公网 TLS、Connecter Host HA 或 Federation 策略分发；这些由 Connecter 平台负责。

## 成员配置

每个 `connecter-remote` 成员保存 `baseUrl`、加密 `bearer`、显式 `env`、Directory v2 `groupRef` 与远端 `targetSubjectId`。配置保存在独立表中，不复用 chatbot/Codex CLI 的 `agent_profiles.api_key`。

## 调度协议

1. WorkPanel 创建本地 `task_runs` 和 streaming Agent 消息。
2. `run_agent` 识别 `connecter-remote` 后调用 `POST /v2/dispatches`，使用 WorkPanel run UUID 作为 `Idempotency-Key`，body 固定 `writeBack:false`。
3. 持久化 `dispatchId` 并轮询 `GET /v2/dispatches/:id`。
4. `completed` 只把结果投影成一次 `final`；`failed`、`dead`、`cancelled` 不伪装成功。
5. 本地取消完成 fencing 后 best-effort 调用远端 cancel；活跃 worker 与 API 不能重复发送取消。

## 数据库兼容

- 最新 `main` 的 schema v4 已用于 `agent_profiles.api_url`。
- Provider 使用 schema v6，幂等确保 `api_url`、`connecter_provider_profiles`、`task_runs.provider_dispatch_id` 与唯一索引同时存在。
- v6 必须让 main-v4、早期 provider-v4，以及已占用 `user_version=5` 但尚无 provider schema 的已部署数据库汇合到相同终态，避免版本号碰撞跳过迁移。

## 安全约束

- service bearer 不进入 argv、日志、Agent prompt、Member DTO 或浏览器回显；系统 `curl` 仅从 stdin 读取配置。
- token 仅授予 dispatch create/read/cancel，并限制 `groupRef` 与 `targetSubjectId` allowlist。
- 明文 HTTP 只允许短期 canary token，验收后轮换；正式可用前补 TLS。

## 验收

1. fresh、legacy、main-v4、provider-v4 与 second-boot 迁移通过，bearer 原始列不含明文。
2. 前端 remote/CLI payload 互斥，动态 adapter catalog 不丢 provider。
3. mock Connecter 验证 create/poll/completed/failed/dead/cancel、幂等键和 `writeBack=false`。
4. 全量前端、Rust、build、AI Harness 与 test gate 通过。
5. ECS canary UI 直接 `@Codex-Windows11`，结果来自本机工作区证明值，且 WorkPanel 只产生一条 completed Agent 消息。
