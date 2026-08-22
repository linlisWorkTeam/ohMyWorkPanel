# ohMyWorkPanel v2.0.0 — Cursor Agent 环境包

把 **Cursor Agent** 在本机可运行所需的依赖打成可导入配置包（**不含登录凭据 / 不含 CLI 二进制**）。

## 包了什么

| 项 | 内容 |
|---|---|
| 配置包 | [`cursor-agent.bundle.json`](./cursor-agent.bundle.json) — `schemaVersion=1`，仅 `cursor.enabled` |
| CLI | 需本机已装 Cursor CLI：`agent`（回退 `cursor-agent`）。本机快照版本 `2026.08.11-e8db854` |
| 模型 | 默认 `grok-4.6`（High Fast） |
| 成员 | `adapter=cursor`，显示名 `Cursor Agent`，`executable=agent` |
| 自动安装 | `autoInstall: ["cursor"]` → 官方安装器（非 win：`curl -fsSL https://cursor.com/install \| bash`） |

**故意不打进 Git 的**：`authInfo` / `authId` / `authCacheKey` / 邮箱 / 任何 API key；`cursor-agent` 二进制本身（体积与许可）。导入后仍需在该机 **登录 Cursor CLI**。

## 怎么用

1. 管理员打开 ohMyWorkPanel「Agent 配置」→ 导入上述 JSON（可勾选自动安装缺失 CLI）。
2. 或从本机重新生成脱敏包：`./scripts/pack-cursor-agent.sh > /tmp/cursor-agent.bundle.json`（不会写入密钥字段）。
3. 导入只更新**已有**成员；`system_locked` 跳过。种子成员 id 需为 `seed-member-cursor` 或显示名匹配「Cursor Agent」。

## 环境依赖清单

- Node.js 20+
- Cursor CLI 在 `PATH`（`~/.local/bin` 或 `~/.local/share/cursor-agent/versions/<ver>/cursor-agent`）
- 本机已登录 Cursor 账号（配置包不携带 session）
- ohMyWorkPanel 二进制能 spawn `agent` / `cursor-agent`

## 风险

本机快照含 `approvalMode=unrestricted`、`sandbox.mode=disabled`。导入到个人笔记本会复制这套宽松策略；生产/个人机请自行改严。
