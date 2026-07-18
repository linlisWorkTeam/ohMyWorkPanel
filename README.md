# LinlisWorkPanel

本地优先的多 Agent 群聊桌面应用。创建协作群、绑定本机工作目录、添加 Agent，通过 `@` 提及触发任务；Agent 在本机已登录的 CLI 中运行，结果流式回写到群聊。

## 技术栈

- 前端：React 19 + Vite 7 + TypeScript
- 桌面壳：Tauri 2
- 后端：Rust + SQLite（`rusqlite`）

## 环境要求

- Node.js 20+
- [pnpm](https://pnpm.io/)
- Rust（stable）与 Tauri 2 系统依赖（Windows 需 WebView2）
- 可选 Agent CLI：`codex`、`claude`、`opencode`、Cursor CLI（`agent`）

## 快速开始

```bash
pnpm install
pnpm tauri dev
```

仅前端：

```bash
pnpm dev
```

## 脚本

| 命令 | 说明 |
|---|---|
| `pnpm dev` | Vite 开发服务器（`127.0.0.1:1420`） |
| `pnpm build` | 前端生产构建 |
| `pnpm tauri dev` | 启动桌面应用（开发） |
| `pnpm test` | 前端 Vitest |
| `cd src-tauri && cargo test` | Rust 单测 |
| `pwsh scripts/smoke-adapters.ps1` | 本机适配器 smoke（尽力，失败不挡合并） |

## Agent 适配器

| 适配器 | 默认可执行文件 | 说明 |
|---|---|---|
| `mock` | — | 本地模拟流式回复，适合体验 UI |
| `codex` | `codex` | `codex exec --json --skip-git-repo-check` |
| `claude-code` | `claude` | `claude -p --output-format stream-json --verbose` |
| `opencode` | `opencode` | `opencode run <prompt> --format json` |
| `cursor` | `agent`（回退 `cursor-agent`） | `agent -p <prompt> --output-format stream-json` |

### 安装 Cursor CLI

Windows（PowerShell）：

```powershell
irm 'https://cursor.com/install?win32=true' | iex
```

macOS / Linux：

```bash
curl https://cursor.com/install -fsS | bash
```

安装后确认 `agent` 在 PATH 中。详见 [Cursor CLI](https://cursor.com/cli)。

## 验证说明

- **必过**：`pnpm test`、`cargo test`（命令拼装与流解析）
- **尽力 smoke**：`scripts/smoke-adapters.ps1` 对本机已安装且已登录的 CLI 做 `--version` 与短提示；未安装记为 `SKIPPED`，不算失败

## 数据存储

应用数据目录下的 `linlis-work-panel.sqlite3`（由 Tauri `app_data_dir` 决定）。启动时会将未完成的 `queued`/`running` 任务标记为 `interrupted`。

## 许可证

Private / 未声明。
