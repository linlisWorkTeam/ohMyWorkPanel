# LinlisWorkPanel

本地优先的多 Agent 协作面板：工作群绑定服务器工作区、`@` 触发本机 CLI Agent；也支持**聊天群** + 轻量 **chatbot**（默认响应者、原生窗口上下文与滚动摘要）。桌面（Tauri）与 Web 双模；本机灰度/生产以 Web + systemd 双槽位为主。

当前发布标签：**[`v1.1.0`](https://github.com/linli0/LinlisWorkPanel/releases/tag/v1.1.0)**（相对 Base V1.0.0 的增量见下）。发展方向 SSOT：[`docs/version-pipeline.md`](docs/version-pipeline.md)。

## 技术栈

- 前端：React 19 + Vite 7 + TypeScript
- 桌面壳：Tauri 2
- 后端：Rust + SQLite（`rusqlite`）
- 包管理：pnpm

## 环境要求

- Node.js 20+
- [pnpm](https://pnpm.io/)
- Rust（stable）；桌面还需 Tauri 2 系统依赖（Windows 需 WebView2）
- 可选 Agent CLI：`codex`、`claude`、`opencode`、`openclaw`、Cursor CLI（`agent`）
- Chatbot：需配置 API Key（OpenCode Go / DeepSeek 等）

## 快速开始

```bash
pnpm install
pnpm tauri dev          # 桌面开发
# 或
pnpm dev                # 仅前端 Vite :1420
```

### Web / 灰度 / 生产（推荐运维路径）

```bash
pnpm run test:gate
export CARGO_BUILD_JOBS=1 NODE_OPTIONS=--max-old-space-size=1024
./scripts/deploy-canary.sh                 # :8081 + data-canary（不动生产）
./scripts/canary-announce-a2a.sh           # 灰度群 A2A 公告改动点
./scripts/approve-prod-release.sh "who: why"  # root 一次性批准（15 分钟）
./scripts/promote-canary.sh                # → :8080，不覆盖生产 DB；勿中断 stop→start
```

| 槽位 | 端口 | 数据 |
|---|---|---|
| 生产 | `:8080` | `/AI/LinlisWorkPanel/data` |
| 灰度 | `:8081` | `/AI/LinlisWorkPanel/data-canary` |

默认登录（种子）：`root` / `root`。群公告规则：先灰度验证与 docs，再晋升生产。

## 文档索引

| 文档 | 用途 |
|---|---|
| [`docs/version-pipeline.md`](docs/version-pipeline.md) | **版本流水线 / 轨道 / 下一站（SSOT）** |
| [`docs/api-web.md`](docs/api-web.md) | Web API 薄索引 |
| [`docs/testing-strategy.md`](docs/testing-strategy.md) | 测试金字塔与门禁 |
| [`docs/release-checklist.md`](docs/release-checklist.md) | 发版检查（含前端壳） |
| [`docs/epitaph/README.md`](docs/epitaph/README.md) | 会话交接墓志铭 |
| [`AGENTS.md`](AGENTS.md) | Agent 贡献约定 |

## v1.1.0 相对 Base 的要点

- 聊天群：可设 **默认响应者**（Agent/chatbot）；chatbot **最近 N 条**（默认 12）+ 时间戳 + **滚动摘要**
- 成员栏：同 Agent **执行中 · 排队 N**，可展开取消
- 发版：releasing/心跳/metrics、种子群 `is_system`、promote 审批门禁
- PanelLive：Extension Host + 同源代理 + A2A（禁 PCM）
- 详情以 `docs/version-pipeline.md` 与 `docs/epitaph/` 为准

## 脚本

| 命令 | 说明 |
|---|---|
| `pnpm dev` / `pnpm build` | Vite 开发 / 前端生产构建 |
| `pnpm tauri dev` | 桌面应用（开发） |
| `pnpm test` / `pnpm run test:gate` | Vitest / 部署前门禁（+ Rust lib 测） |
| `./scripts/deploy-canary.sh` | 门禁 → 构建 → 灰度 `:8081` |
| `./scripts/canary-announce-a2a.sh` | 灰度改动点 A2A 公告 |
| `./scripts/approve-prod-release.sh` | 生产操作一次性批准 |
| `./scripts/promote-canary.sh` | 灰度 → 生产（bin+dist） |
| `powershell -File scripts/smoke-adapters.ps1` | 适配器 smoke（尽力，不进门禁） |

## Agent 适配器

| 适配器 | 默认可执行文件 | 说明 |
|---|---|---|
| `mock` | — | 本地模拟流式回复 |
| `codex` | `codex` | `codex exec --json …` |
| `claude-code` | `claude` | stream-json |
| `opencode` | `opencode` | JSON run |
| `openclaw` | （HTTP/配置） | 产品/运维向 Agent |
| `cursor` | `agent`（回退 `cursor-agent`） | Cursor CLI |
| `chatbot-*` | HTTP（curl） | 聊天群轻量机器人，无工具 |

### 安装 Cursor CLI

```bash
# macOS / Linux
curl https://cursor.com/install -fsS | bash
# Windows PowerShell: irm 'https://cursor.com/install?win32=true' | iex
```

确认 `agent` 在 PATH。详见 [Cursor CLI](https://cursor.com/cli)。

## 验证说明

- **必过**：`pnpm run test:gate`（或分别 Vitest + `cargo test --no-default-features --lib`）
- **尽力 smoke**：`scripts/smoke-adapters.ps1`；未安装 CLI 记 `SKIPPED`

## 数据存储

- 桌面：Tauri `app_data_dir` 下 `linlis-work-panel.sqlite3`
- Web 生产/灰度：见上表数据目录（**切勿混用**）
- 启动时将未完成的 `queued`/`running` 标为 `interrupted`

## 许可证

Private / 未声明。
