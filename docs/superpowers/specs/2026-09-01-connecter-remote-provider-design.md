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

每个 `connecter-remote` 成员保存：

- `baseUrl`：Connecter Host/provider API 基地址，仅接受 `http://` 或 `https://`；
- `bearer`：WorkPanel 专用 service token，加密落库且永不返回前端；
- `env`：明确保存，首个实例为 `canary`，不得静默依赖 Connecter 默认值；
- `groupRef`：Directory v2 group reference；
- `targetSubjectId`：远端 Runner subject id。

配置保存在独立表中，不复用 chatbot/Codex CLI 的 `agent_profiles.api_key`。

## 调度协议

1. WorkPanel 保持现有调度：创建本地 `task_runs` 和 streaming Agent 消息。
2. `run_agent` 识别 `connecter-remote` 后调用 `POST /v2/dispatches`：
   - `Authorization: Bearer <member service token>`；
   - `Idempotency-Key: <WorkPanel task_runs.id>`；
   - body 明确包含 `env`、`groupRef`、`targetSubjectId`、`prompt`、`writeBack:false`。
3. 保存 `dispatchId`，按固定间隔调用 `GET /v2/dispatches/:id`。
4. `completed` 时只把 `result.content` 投影成一次 `final` delta，然后复用 WorkPanel 现有 completion/review/A2A 收口。
5. `failed`、`dead` 映射为本地失败；远端 `cancelled` 不伪装为成功。
6. 本地取消先完成本地 fencing，再 best-effort 调 `POST /v2/dispatches/:id/cancel`。

## 幂等与恢复

- 本地 run UUID 是唯一幂等键；重试 POST 必须返回同一个 dispatch。
- `task_runs.provider_dispatch_id` 持久化远端 id，支持取消、诊断和进程重启后的恢复。
- 本地 `finish_completed` 的 `status='running'` 条件更新是最终写入守卫；不得另设 Connecter 写回路径。

## 安全约束

- service bearer 不进入 argv、日志、Agent prompt、Member DTO 或浏览器回显。
- HTTP 客户端不得引入会显著增加 2GB ECS 编译压力的依赖；允许以 stdin 配置驱动系统 `curl`。
- Connecter service token 必须仅授予 `dispatch:create/read/cancel`，并限制 `groupRef` 与 `targetSubjectId` allowlist。
- 现场使用明文 HTTP 时仅使用短期 canary token，验收后轮换；正式可用前补 TLS。

## 验收

1. SQLite fresh/legacy/second-boot 迁移通过，bearer 原始列不含明文。
2. 前端可创建 remote 成员；动态 adapter catalog 不丢 provider；非 remote 成员不发送 provider 字段。
3. mock Connecter 验证 POST、轮询、completed/failed/dead/cancel、幂等键和 `writeBack=false`。
4. `pnpm test`、`cargo test --no-default-features --lib`、`pnpm run test:gate` 通过。
5. ECS canary UI 直接 `@Codex-Windows11`，结果来自本机工作区证明值，并且 WorkPanel 只产生一条 completed Agent 消息。
