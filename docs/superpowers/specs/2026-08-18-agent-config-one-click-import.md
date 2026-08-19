---
date: 2026-08-18
topic: agent-config-one-click-import
spec: true
status: implemented
---

# Agent 配置一键导入 / 导出 / 自检 / CLI 自动安装（Spec + 实现记录）

> 背景：ECS 上各 Agent（codex / claude / opencode / cursor agent / dsh）通常由「vibecoding」
> 逐台配好（home 配置 + 登录 + 密钥 + 模型）。本地 / 新安装拿不到这些，导致 @Codex / @Cursor
> 不通。本文定义**配置包（bundle）** + **一键导入** + **自动安装** + **启动自动重放**，让
> release 做到开箱即用：**新增用户不再需要重新 vibecoding**（只保留部分可扩展性）。

## 目标 / 非目标

| 目标 | 说明 |
|---|---|
| 一键导入 | 在服务器导出配置包 → 本地粘贴/选文件「一键导入」：写 home 配置（备份后合并）+ 同步成员（agent_profiles）+ 持久化 |
| 开箱即用 | 持久化的配置在每次启动幂等重放（`auto_apply_on_startup`）；release 槽位自带 `scripts/codex-deepseek-proxy.cjs`（exe 旁），新机不再依赖 vibecoding |
| 尽量自动装 CLI | 缺失 codex/claude/opencode/dsh（npm -g）与 cursor agent（官方安装器）可一键自动安装（best-effort，失败给人工命令） |
| 环境自检 | Node / Codex shim(:18888) / 各 CLI / 密钥 / 配置落点，一眼看清 |
| 部分可扩展性 | 已知适配器用结构化字段；未知 CLI 用通用 `files`（home 相对路径）逃生口；schemaVersion 版本化 |

**非目标**：不内置第三方 CLI 二进制；不做多平台安装包分发；不做跨群 GUI 向导（就是这页）。

## 配置包（Bundle）格式（v1）

```jsonc
{
  "schemaVersion": 1,
  "exportedAt": 1724000000000,
  "exportedBy": "root",
  "source": "linlis-work-panel/export",
  "codex":   { "enabled": true,  "baseUrl": "http://127.0.0.1:18888/v1",
               "model": "deepseek-v4-flash", "apiKey": "sk-…", "authMode": "apikey" },
  "claude":  { "enabled": false, "baseUrl": "https://api.…", "authToken": "sk-…", "model": null },
  "cursor":  { "enabled": false, "executable": "agent", "model": null,
               "cliConfig": { …合并到 ~/.cursor/cli-config.json… }, "mcp": { … } },
  "opencode":{ "enabled": true, "model": "deepseek-v4-flash", "apiKey": "…" },
  "files":   { ".config/opencode/opencode.json": { … }, "…": "原始文本" },
  "agents":  [ { "adapter":"codex", "displayName":"Codex", "memberId":"seed-member-codex",
                 "model": "…", "apiKey":"…", "executable": null } ],
  "autoInstall": ["codex","cursor","claude","opencode","dsh"]
}
```

- `files`：Value 为字符串按原文写，否则按 JSON 美化写；仅允许 home 相对路径（拒绝 `..`/绝对路径）。
- 密钥语义：**不含密钥的导出会把 `apiKey`/`authToken` 置空**，导入时不会误写占位串；含密钥导出才可跨机迁移凭据。
- `autoInstall`：导入时勾选「自动安装缺失 CLI」才会执行（后端 `ImportInput.autoInstall`）。

## 后端（Rust，`src-tauri/src/agent_config.rs`）

- `build_bundle(db, include_secrets)`：读 `~/.codex/auth.json`、`~/.claude/settings.json`、
  `~/.cursor/cli-config.json|mcp.json`、成员 `agent_profiles`（含 codex 成员 key）→ 组装 bundle。
- `import(db, bundle, auto_install, overwrite)`（async）：
  1. Node 前置检查
  2. `apply_codex` / `apply_claude` / `apply_cursor` / `apply_files`（备份后合并写；`overwrite=false` 时已存在则保留）
  3. `provision_agents`：只更新既有成员（按 adapter / displayName / memberId），跳过 `system_locked`；顶层段需 `enabled=true` 才生效
  4. 持久化 `app_settings`：`agent_config`（bundle）、`agent_config_imported_at`、`agent_config_auto_apply`("1")
  5. 自动安装缺失 CLI（best-effort）
- `auto_apply_on_startup(db)`：启动幂等重放（缺失补写、不覆盖已有、不重装 CLI）。
- `status(db)`：Node / shim(:18888) / 各 CLI 在位 / codex 密钥 / 配置落点 / 导入时间 / 脱敏有效配置。
- CLI 安装：`install_spec`（codex/claude/opencode/dsh → `npm -g`；cursor → 官方安装器，按 OS 分支）；
  超时 180s，输出截断返回给前端。

### 顺带修的两个环境问题

- `a2a::dispatch_live_skill`：只有需要上游的 control-plane skill 才读 PanelLive manifest；
  transcript ack（WS-only）不再因本机缺 `/AI/WorkPanelLive` 而硬失败。
- `fs_browse::tests::rejects_missing_path`：测试改用跨平台的「绝对但不存在」路径（win 用 `C:\…`）。

### Web API（仅管理员）

| 路由 | 说明 |
|---|---|
| `GET /api/agent-config/status` | 环境自检（脱敏）|
| `POST /api/agent-config/export` `{includeSecrets}` | 导出配置包 |
| `POST /api/agent-config/import` `{bundle, autoInstall?, overwrite?}` | 一键导入，返回 `ImportReport` |
| `POST /api/agent-config/install/{cli}` | 单 CLI 自动安装 |

## 前端

- 顶部新页签「Agent 配置」（仅 `isAdmin`；已导入显示 `✓`）→ `src/AgentConfigView.tsx`。
- 四节：环境自检（卡片 + 每缺 CLI 自动安装）、一键导入（textarea/选文件 + 自动安装勾选 + 报告 steps）、
  导出（含密钥/不含密钥 → 浏览器下载）、使用说明。
- `api-web.ts` 新增 4 个方法 + 类型；`api.ts`（桌面）加同签名 stub 抛「仅 Web 服务可用」。

## 打包 / 开箱即用

- `scripts/codex-deepseek-proxy.cjs`：auth 解析改为 `CODEX_HOME || ~/.codex`（跨平台，不再硬编码 `/root`）。
- `codex_proxy::resolve_script_path` 增加「可执行文件旁 `scripts/`」候选 → 槽位自包含。
- `deploy-canary.sh` / `promote-canary.sh`：槽位同步发布 `scripts/codex-deepseek-proxy.cjs`。
- `adapters/codex::default_auth_path`：尊重 `CODEX_HOME` / Windows `USERPROFILE`（与 shim 一致）。

## 验收（已在本机跑通）

- `cargo test --no-default-features --lib`：118 通过（含新增 7 个 agent_config 单测）。
- `pnpm exec vitest run --pool=forks --maxWorkers=1`：72 通过；`tsc -b` 通过；`build:web` 通过。
- 端到端（临时 18099 + 临时 CODEX_HOME + 临时 data）：login→status→export(no secrets)→import→
  export(with secrets)→重启后 bundleImportedAt/codexKeySet 仍在；`~/.codex/auth.json` 写入；
  成员 Codex 拿到 key、Cursor 拿到 executable+model；`overwrite=false` 幂等保留并如实报告。
- 已知：`bash scripts/test-gate.sh` 在本机（Windows）因 `pnpm` 非 bash 二进制而报
  `pnpm: command not found` —— 属环境 PATH 问题，三个组成部分均已单独跑绿；ECS(Linux) 无此问题。
- release 路径：build:web → `deploy-canary.sh`（门禁在 ECS 全绿后再 promote）。

## 相关文件

- 实现：`src-tauri/src/agent_config.rs`、`src-tauri/src/web.rs`、`src-tauri/src/db.rs`、
  `src-tauri/src/main_server.rs`、`src-tauri/src/codex_proxy.rs`、`src-tauri/src/a2a.rs`、
  `src-tauri/src/adapters/{mod.rs,codex.rs}`、`scripts/codex-deepseek-proxy.cjs`、
  `scripts/{deploy-canary.sh,promote-canary.sh}`。
- 前端：`src/AgentConfigView.tsx`、`src/api-web.ts`、`src/api.ts`、`src/App.tsx`、`src/styles.css`。
- 交接：`docs/epitaph/2026-08-18-agent-config-import.md`。
