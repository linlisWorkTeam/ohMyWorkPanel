---
date: 2026-08-18
topic: agent-config-import
epitaph: true
status: active
---

# 交接：Agent 配置一键导入（v1.3.0+ 增量）

## 一句话
新增「Agent 配置」页（仅管理员）：**服务器导出配置包 → 本地一键导入**（写 `~/.codex` / `~/.claude` / `~/.cursor` / 通用 `files`，同步成员，持久化 + 启动自动重放），
缺失 CLI 可**一键自动安装**（best-effort），并自带**环境自检**。目标：release 开箱即用，新用户不再需要 vibecoding。

## 变更文件
- 后端：`src-tauri/src/agent_config.rs`（新增，核心）、`web.rs`（4 条 /api/agent-config 路由，仅管理员）、
  `db.rs`（`set_member_api_key`/`set_member_executable`/`list_agent_profiles`/`get_setting_str`/`set_setting_str`）、
  `main_server.rs`（启动 `auto_apply_on_startup`）、`codex_proxy.rs`（exe 旁找 shim 脚本）、
  `a2a.rs`（manifest 懒加载修复）、`adapters/{mod.rs,codex.rs}`（PATH 助手 / `CODEX_HOME|USERPROFILE`）、
  `lib.rs`（`pub mod agent_config`）。
- 脚本：`scripts/codex-deepseek-proxy.cjs`（auth home 跨平台）、`scripts/{deploy-canary.sh,promote-canary.sh}`（槽位带 shim 脚本）。
- 前端：`src/AgentConfigView.tsx`（新增）、`api-web.ts`（4 方法 + 类型）、`api.ts`（桌面 stub）、
  `App.tsx`（顶部「Agent 配置」页签 + 渲染分支）、`styles.css`。
- 文档：本 epitaph + `docs/superpowers/specs/2026-08-18-agent-config-one-click-import.md` + README + version-pipeline。

## 关键约束 / 注意
- **仅管理员**可调（web 路由 `require_admin`）；密钥只在「导出（含密钥）」里下发，`Member` 模型仍只露 `apiKeySet`。
- 导入只**更新**既有成员、跳过 `system_locked`；顶层段需 `enabled=true` 才生效。
- `overwrite=false`（默认 true）时已存在文件保留并如实报告「已存在（保留）」。
- `files` 逃生口仅接受 home 相对路径（拒绝 `..`/绝对），供未知 CLI（如 opencode.json）。
- 含密钥导出才能跨机迁移凭据；**不含密钥导出会把 key 置空**，导入不会误写占位串。
- 自动安装：codex/claude/opencode/dsh→`npm -g`（需 node）；cursor→官方安装器（win 用 PowerShell，非 win 用 curl|bash）。失败不阻塞，返回人工命令。
- shim 端口冲突时自动复用（`codex_proxy::start_embedded`），`8080/8081` 勿在本地冲突已有实例。

## 验证
本机（Windows）：`cargo test --lib` 118 通过；Vitest 72 通过；`tsc -b`、`build:web` 通过；
端到端 smoke（临时 18099 + CODEX_HOME）已验证导入/导出/重启重放/成员同步/密钥保留；真实 `~/.cursor`、`~/.codex` 未受影响。
**注意**：`bash scripts/test-gate.sh` 在本机报 `pnpm: command not found`（git-bash 无 pnpm），三部分单独跑均绿；ECS(Linux) 门禁正常。

## 下一步建议
1. ECS canary `:8081` 部署验证 → 群公告 →（批）promote 生产；再做一次前端壳冒烟（`docs/release-checklist.md §F`）。
2. 可选：`files` 里给 opencode 预置官方 schema 模板；codex `config.toml` 深度合并（当前仅缺省新建）。
3. 若发小版本，`package.json`/Cargo 版本与 `v1.3.0` 对齐策略不变（见 version-pipeline 顶部说明）。
