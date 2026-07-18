# AGENTS.md

AI agent 贡献者请遵循以下项目约定。

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 | React 19, Vite 7, TypeScript 5.8 |
| 桌面壳 | Tauri 2 |
| 后端 | Rust 2021, rusqlite 0.32 |
| 包管理 | pnpm（`pnpm-workspace.yaml`） |
| 测试 | 前端 Vitest / Rust `cargo test` |

## 项目结构

```text
src/                    # React 前端
  App.tsx               # 入口组件（群侧栏/聊天面板/成员面板/设置）
  api.ts                # Tauri invoke 封装
  types.ts              # 前后端共享类型
  mentions.ts           # @ 提及解析
  mentions.test.ts
  styles.css
src-tauri/src/          # Rust 后端
  lib.rs                # 应用启动、AppState、模块声明
  models.rs             # serde 数据模型（Group/Member/Message/TaskRun/...）
  db.rs                 # SQLite 初始化、查询、写入辅助
  commands.rs           # 全部 #[tauri::command]
  scheduler.rs          # 任务调度/执行/委派/完成
  adapters/             # Agent 适配器
    mod.rs              # AdapterKind 枚举 + run_streaming 共享执行
    parse.rs            # JSON 行解析 + 单测
    mock.rs             # 本地模拟
    codex.rs            # Codex CLI 参数
    claude.rs           # Claude Code CLI 参数
    opencode.rs         # OpenCode CLI 参数
    cursor.rs           # Cursor CLI 参数 + 回退候选
scripts/
  smoke-adapters.ps1    # 尽力适配器 smoke（不阻塞交付）
```

## 编码约定

- **IPC 兼容**：不改动 `tauri::command` 签名、前端 API 调用、SQLite schema。破坏性 API 变更需明确讨论。
- **新增适配器**：实现 `adapters::mod.rs` 的 `AdapterKind`，提供 `build_args`；cli 候选排在 `candidate_executables` 里。
- **Rust 风格**：`AppResult<T> = Result<T, String>`；行内链式 `map_err(|e| e.to_string())?` 与现有风格一致。
- **前端适配器标签**：在 `App.tsx:155` 的 `<select>` 里更新文案；`types.ts:21` 的 adapter union 保持同步。
- **Commit 风格**：中文或英文均可，建议类型前缀（`chore:`/`refactor:`/`feat:`/`test:`/`fix:`）。
- **已忽略文件**：`node_modules/`、`dist/`、`src-tauri/target/`、`src-tauri/gen/`、`.pnpm-store/`、`*.sqlite3*`。

## 开发命令

| 命令 | 说明 |
|---|---|
| `pnpm install` | 安装依赖 |
| `pnpm tauri dev` | 启动桌面应用（开发） |
| `pnpm test` | 前端 Vitest |
| `cd src-tauri && cargo test` | Rust 单测 |
| `cd src-tauri && cargo build` | Rust 编译检查 |
| `powershell -File scripts/smoke-adapters.ps1` | 适配器 smoke |

## 注意事项

- 本机需 Node 20+、Rust stable、WebView2（Windows）。
- Agent 运行依赖本机已登录的 CLI（codex/claude/opencode/agent）。
- `cargo test` 需在 `src-tauri/` 目录运行。
- `AppState` 持有 `db_path` 与任务取消/调度锁，`commands` 消费 `State<AppState>`。
- `schedule_group` 为同步函数（内部仅入锁+spawn），不要加 `.await`。
- `append_delta` 为同步函数，避免异步闭包中持有 `MutexGuard` 导致 `!Send`。

## Handoff notes

This project uses `docs/epitaph/` for session handoff notes. New agents:
1. Read `docs/epitaph/README.md` for index.
2. Read the latest active epitaph before modifying related code.
3. Follow the epitaph skill workflow to write new handoffs.
