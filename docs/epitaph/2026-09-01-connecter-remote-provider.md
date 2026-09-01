---
date: 2026-09-01
topic: connecter-remote-provider
branch: codex/connecter-provider-canary
status: active
---

# Epitaph: Connecter 远程 Agent Provider

## Built this session

- 新增非 CLI provider `connecter-remote`，在 WorkPanel scheduler 内通过 Connecter `/v2/dispatches` create/get/cancel 调度远端 Runner。
- 成员配置独立保存 `baseUrl`、`env`、`groupRef`、`targetSubjectId` 和 service bearer；bearer 使用现有 AES-GCM secret 机制加密，Member DTO 与日志不返回密钥。
- `Idempotency-Key` 固定为 WorkPanel run UUID；`task_runs.provider_dispatch_id` 支持取消、诊断和进程重启恢复。
- Connecter dispatch 固定 `writeBack=false`；完成后只向 WorkPanel streaming message 追加一次 `final`，继续复用现有 review/A2A/terminal 收口。
- desktop/web add-member、detect 和 cancel 同步支持 provider；活跃 worker 由 cancellation token 单次发远端 cancel，无 worker 时 API best-effort 直发。
- 前端动态 catalog 始终保留 `connecter-remote`；远端表单与 CLI model/executable 互斥，bearer 使用不回显 password input。

设计：`docs/superpowers/specs/2026-09-01-connecter-remote-provider-design.md`。

## Verified

- `pnpm test`：27 files / 100 tests passed。
- `pnpm exec tsc -b`：通过。
- `cargo test --no-default-features --lib`：151 passed。
- `cargo check --no-default-features --bin linlis-work-panel-server`：通过。
- `pnpm run build:web`：通过。
- Extension Host purity 三组禁用模式扫描：无命中。
- Windows 上 `pnpm run test:gate` 因系统 `bash.exe` 指向未安装的 WSL 发行版而无法执行包装脚本；上述命令已逐项执行相同门禁内容。

真实本地 E2E（非 mock Codex）：

- WorkPanel API 创建 `connecter-remote` 成员并通过 `@Codex-Windows11-Remote` 发起 run；
- Connecter dispatch `09d6632a-0b8f-5d1c-a4ae-e023ee7f3ee0`；
- WorkPanel run `303327e5-b494-4208-bdc1-d1db303eb950`；
- 结果 `WORKPANEL_CONNECTER_REMOTE_E2E_a9965ff6|workpanel-connecter|0.2.3`；
- WorkPanel reply count = 1，remote `writeBack=false`，provider bearer 密文落库。

## Not done / blocker

- 尚未部署 ECS canary `:8081`，因此 ECS UI 直接 `@Codex-Windows11` 的最终现场验收仍未完成。
- 现有 `POST /api/ops/deploy-canary` 只构建 ECS 当前 `LINLIS_ROOT`，不接受 Git ref；需要一个可审计的 ECS 写入执行者先把本分支 immutable commit 放入 `/AI/ohMyWorkPanel`。
- ECS Connecter Host 还需升级到包含 Federation Directory 与 WorkPanel service dispatch API 的版本，并配置最小权限 service token/allowlist。

## Do not regress

- 不得把 `connecter-remote` 塞入 CLI manifest 或在 ECS 本机 spawn 被称为“远端”的 Agent。
- 不得复用 `agent_profiles.api_key` 保存 service bearer，不得把 bearer 放入 curl argv、日志或 Member DTO。
- 不得允许 `writeBack=true`；WorkPanel 必须是群消息唯一写入者。
- 不得直接 promote `:8080`；先完成 `:8081` UI 直调与单消息验收。
