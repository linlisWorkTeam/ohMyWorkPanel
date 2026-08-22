# AGENTS.md

AI agent 贡献者请遵循以下项目约定。

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 | React 19, Vite 7, TypeScript 5.8 |
| 桌面壳 | Tauri 2 |
| 后端 | Rust 2021, rusqlite 0.32 |
| 包管理 | pnpm（`pnpm-workspace.yaml`） |
| 测试 | 前端 Vitest / Rust `cargo test`；策略见 `docs/testing-strategy.md`；canary 门禁 `scripts/test-gate.sh` |

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
- **新增 CLI 适配器**：见 [`docs/superpowers/specs/2026-08-21-cli-adapter-manifest.md`](docs/superpowers/specs/2026-08-21-cli-adapter-manifest.md)（终态 `*.adapter.json`）。**P0 未落地前**仍可改 `AdapterKind` + `build_args` + `candidate_executables`；落地后内部新 CLI 只加 json，禁止 `sh -c`。chatbot / mock 不是 CLI 插件。
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
| `pnpm run test:gate` / `./scripts/test-gate.sh` | 部署前门禁（Vitest + `cargo test --lib`） |
| `cd src-tauri && cargo test --no-default-features --lib` | Rust 单测（与门禁一致） |
| `cd src-tauri && cargo build` | Rust 编译检查 |
| `powershell -File scripts/smoke-adapters.ps1` | 适配器 smoke（不进门禁） |
| `./scripts/deploy-canary.sh` | 先跑门禁再构建/部署灰度（不得动生产） |
| `./scripts/approve-prod-release.sh` | root 一次性批准生产变更（15 分钟） |
| `./scripts/promote-canary.sh` | 灰度→生产（需批准令牌；不覆盖 DB） |

发版勾选清单（含前端壳/白屏/实时 UI）：[`docs/release-checklist.md`](docs/release-checklist.md)。

## 注意事项

- **发布流程（群公告）**：行为变更须先更新 docs → 部署/验证灰度（`:8081`）→ 灰度通过后再 commit。**晋升生产必须 root 批准**：`./scripts/approve-prod-release.sh "…" && ./scripts/promote-canary.sh`。Agent 不得伪造批准令牌或擅自 `systemctl restart` 生产。禁止用 `LINLIS_SKIP_TEST_GATE` 绕过测试门禁。灰度/生产均须按 `docs/release-checklist.md` 做前端壳冒烟（§F），避免「前台 React 崩了」类假死漏检。
- **槽位隔离**：生产 Codex shim `:18888`；灰度 `:18889`。`deploy-canary` 禁止 `fuser -k 18888`、禁止改写生产 systemd unit。
- **Commit 前**：遵守 `.cursor/rules/pre-commit-test-gate.mdc`——复核自动化测试设计，并跑通 `pnpm run test:gate`；与上条群公告一并满足。
- **工作区路径**：建群/改工作区选**服务器绝对路径**（`ServerPathPicker` / `GET /api/fs/list`），不是浏览器本机路径；可在当前目录下 `POST /api/fs/mkdir` 新建文件夹后再选用（不可在 `/` 下直接建）。路由索引见 `docs/api-web.md`。
- **Extend 页签入口**：PanelLive 等扩展 UI 必须走平台同源代理（如 `/api/extensions/panellive/...`），**禁止** iframe 直连 `http://127.0.0.1:端口`（浏览器会打到用户本机，且 HTTPS 会混合内容拦截）。
- **Live / Host 仓边界**：STT/TTS/`live.html` **只改** `/AI/WorkPanelLive`（独立 git）；代理/`LivePanel`/短回复/A2A **只改**本仓。正式 Live 群 workspace=`/AI/WorkPanelLive`；错名群 `WorPanelLive（废弃·错名）` 已归档勿解档。见 `docs/superpowers/specs/2026-08-05-workspace-boundary-live-host.md`。
- **群公告**：等同全员项目级 rule，写入后注入 Agent prompt，并尝试同步工作区 `.cursor/rules/group-announcement.mdc`。
- **Live 豆包语音 UX**：主聊天「按住说话 / 气泡播放」在 Host（`src/liveVoice.ts`）；媒体走 `/api/extensions/panellive`；契约见 `docs/superpowers/specs/2026-08-05-doubao-voice-ux-host.md`。
- 本机需 Node 20+、Rust stable、WebView2（Windows）。
- Agent 运行依赖本机已登录的 CLI（codex/claude/opencode/agent）。
- `cargo test` 需在 `src-tauri/` 目录运行。
- `AppState` 持有 `db_path` 与任务取消/调度锁，`commands` 消费 `State<AppState>`。
- `schedule_group` 为同步函数（内部仅入锁+spawn），不要加 `.await`。
- `append_delta` 为同步函数，避免异步闭包中持有 `MutexGuard` 导致 `!Send`。

## 文档规范

文档改动遵循最简落地原则：只使用 UTF-8 Markdown，不引入 MkDocs、Docusaurus 等重型构建工具；文档随代码提交到 Git。

- 按 Diátaxis 最小子集分类：`docs/tutorials/`、`docs/how-to/`、`docs/explanation/`、`docs/reference/`。教程、操作步骤、概念解释和参考信息不要揉在同一个文件。
- 根目录 `README.md` 只做项目门面、快速上手和文档导航；复杂内容放到 `docs/`。
- `CHANGELOG.md` 遵循 Keep a Changelog；顶部始终保留 `[Unreleased]`。
- `docs/explanation/roadmap.md` 使用预估季度，区分已排期计划与 Backlog 待评估需求，不写死发布日期。
- 文档面向项目使用者，优先给出可复制命令；不清楚的事实使用 `<!-- TODO: 根据项目实际补充 -->`，禁止编造功能。
- 发布博客、技术复盘、踩坑总结、选型心路等面向普通读者的内容，归档到 `wpKnowledge`，不要扩张本仓库用户文档。
- 修改文档前保留既有用户内容；新增或迁移文档后检查 Markdown 链接、命令和 UTF-8 编码。

## Handoff notes

This project uses `docs/epitaph/` for session handoff notes. New agents:
1. Read `docs/version-pipeline.md`（版本流水线 / 发展方向 SSOT；先占位再改码）.
2. Read `docs/epitaph/README.md` for index.
3. Read the latest active epitaph before modifying related code.
4. Follow the epitaph skill workflow to write new handoffs.
