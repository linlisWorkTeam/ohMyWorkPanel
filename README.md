# LinlisWorkPanel

本地优先的多 Agent 协作面板：工作群绑定服务器工作区、`@` 触发本机 CLI Agent；也支持**聊天群** + 轻量 **chatbot**（默认响应者、原生窗口上下文与滚动摘要）。桌面（Tauri）与 Web 双模；本机灰度/生产以 Web + systemd 双槽位为主。

当前 Git 标签：**[`v1.3.0`](https://github.com/linlisWorkTeam/workPanel/releases/tag/v1.3.0)**（工作流实现点）；前序 [`v1.2.0`](https://github.com/linlisWorkTeam/workPanel/releases/tag/v1.2.0)、[`v1.1.0`](https://github.com/linlisWorkTeam/workPanel/releases/tag/v1.1.0)。HEAD 上 drain / 群设置 / DSH P0 等为 **1.3.0+** 补丁，尚未另打小版本。`package.json` / Cargo 已对齐 `1.3.0`。  
发展方向 SSOT：[`docs/version-pipeline.md`](docs/version-pipeline.md)（勿把历史 epitaph「v1.3 双槽位」当成 tag `v1.3.0`）。  
远端仓库：`https://github.com/linlisWorkTeam/workPanel`（`origin`）。

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
| [`docs/superpowers/specs/2026-08-16-dsh-self-bootstrap-runtime.md`](docs/superpowers/specs/2026-08-16-dsh-self-bootstrap-runtime.md) | **DSH 自举接入总设计（轨道 G）** |
| [`docs/superpowers/specs/2026-08-16-dsh-ui-language-workpanel.md`](docs/superpowers/specs/2026-08-16-dsh-ui-language-workpanel.md) | **借鉴 DSH UI 设计语言（三栏：工作区=群聊，右栏=Agent）** |
| [`docs/superpowers/specs/2026-08-16-widget-capability-placement.md`](docs/superpowers/specs/2026-08-16-widget-capability-placement.md) | **小组件形态判定与收敛路线（widget=页签/能力，不单独建群）** |
| [`docs/release-runbook-2026-08-16-dsh-self-bootstrap.md`](docs/release-runbook-2026-08-16-dsh-self-bootstrap.md) | **发布 Runbook：本地验证 → GitHub → ECS 灰度 :8081 → 生产 :8080** |
| [`docs/release-manifest-2026-08-16.md`](docs/release-manifest-2026-08-16.md) | **本次发布变更清单（逐文件核对 + 达成标准）** |
| [`docs/superpowers/specs/2026-08-18-agent-config-one-click-import.md`](docs/superpowers/specs/2026-08-18-agent-config-one-click-import.md) | **Agent 配置一键导入 / 导出 / 自检 / CLI 自动安装（开箱即用）** |
| [`docs/api-web.md`](docs/api-web.md) | Web API 薄索引 |
| [`docs/testing-strategy.md`](docs/testing-strategy.md) | 测试金字塔与门禁 |
| [`docs/release-checklist.md`](docs/release-checklist.md) | 发版检查（含前端壳） |
| [`docs/epitaph/README.md`](docs/epitaph/README.md) | 会话交接墓志铭 |
| [`AGENTS.md`](AGENTS.md) | Agent 贡献约定 |

## 当前版本要点

| 标签 | 含义 |
|---|---|
| **v1.3.0** | 版本页 + Ask / Wave 工作流（控制面；Wave 执行仍走管理员 kickoff） |
| **v1.2.0** | 豆包语音 UX + Live 会话/聊天一致（生产基线 `0750306`） |
| **v1.1.0** | 聊天默认响应者、成员排队可见、PanelLive 宿主、promote 审批 |
| **1.3.0+**（未另打 tag） | 发版 drain、交接运行时注入（epitaph 摘要）、群设置入口、DSH headless P0、**Agent 配置一键导入（开箱即用）** |

- 发版：灰度 `:8081` → docs → commit → **人批准** 才能 promote `:8080`；Agent 不得伪造令牌或绕过审批。
- DSH：外挂运行时（锁版本、进程隔离），**不是**本仓编译期内核；群聊不是唯一事实源。
- 详情以 `docs/version-pipeline.md` 与 `docs/epitaph/` 为准。

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
| `dsh` | `dsh`（npm `@deepseek-ai/dsh`） | DeepSeek Harness headless：`dsh --profile headless "task"`；成员栏可「跳转 DSH Web」嵌入 `:3080` Web UI |
| `chatbot-*` | HTTP（curl） | 聊天群轻量机器人，无工具 |

## Agent 配置一键导入（v1.3.0+ 增量，release 开箱即用）

服务器（已 vibecoding 配好 Agent）在顶部「Agent 配置」页 **导出配置包** → 本地 / 新安装
顶部「Agent 配置」页 **一键导入**：自动写 `~/.codex`（auth.json + 最小 provider）、
`~/.claude/settings.json`、`~/.cursor`（cli-config / mcp）、通用 `files`（备份后合并）→
同步成员（agent_profiles）→ 持久化并随启动**幂等重放**；缺失 CLI 可一键**自动安装**
（codex / claude / opencode / dsh 走 `npm -g`，cursor 走官方安装器；best-effort 不阻塞），并带**环境自检**。
新增用户从此**无需重新 vibecoding**。仅管理员可见。详见
[spec](docs/superpowers/specs/2026-08-18-agent-config-one-click-import.md) 与
[epitaph](docs/epitaph/2026-08-18-agent-config-import.md)。

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
- 启动时将未完成的 `queued`/`running` **重入队**（`phase=recovering`），不再永久标为 `interrupted`

## 许可证

Private / 未声明。
