---
date: 2026-10-22
topic: agent-playground-program
branch: main
status: active
---

# Epitaph: Agent 游乐园程序（插件内核 / Goal 自治 / 像素前台）

## Built this session

从 `origin/main`（`1bf4c58`）出发，本地 68 个提交，覆盖四条轨道：

**P0 契约**
- `openspec/`：`project.md`（愿景/原则/架构地图）、`specs/`（kernel、extensions 契约）、`changes/0001-agent-playground/`（proposal / design / tasks + scene、goals delta）
- `docs/version-pipeline.md` 新增「阶段 9 — Agent 游乐园」

**P1 插件内核（Rust + TS 双同构）**
- `src-tauri/src/kernel/`：`Context`（服务注册表 / 类型安全取回 / 释放后拒绝注册 / `fork()` 作用域）、`EffectStack`（逆序回收、幂等）、`EventBus`（emit / parallel / serial / bail / **waterfall**）、`PluginSpec` + `load_plugins`（拓扑排序、声明兑现校验、失败整体回滚）、`plugin.manifest.json` v2（解析 / 目录发现 / 注册表映射 / **磁盘清单接入启动**）
- `src/kernel/`：前端同构内核（context / events / plugin / host / builtins），23 个契约测试与 Rust 侧逐条对应
- `src-tauri/src/plugins/`：进程级内核 `OnceLock` + 四个内置插件；所有调用方（Web / scheduler / 桌面 IPC / agent 自检）只走服务，无第二套实现

**P2 像素前台**
- `src/scene/`：`sceneModel`（member/run → 角色，确定性布局、候场区换行、**分群房间种子**、**adapter 造型**、**功能区布局**）、`PixelScene`（Canvas 2D、DPR 适配、30fps 上限、地板离屏缓存、四态动画、选中高亮、房间色带、分区虚线、品牌色块）、`SceneView`（HUD / 空态 / 返回聊天）
- 插件化：`scenePlugin` 提供 `svc.scene`，`App` 通过 `SceneSlot` 从内核容器取用；插件缺失 fail-open 提示
- 群作用域：`openGroupScope(groupId)` 用 `ctx.fork()` 承载群专属房间配置，切群即释放

**P3 Goal 自治**
- `src-tauri/src/goals/`：领域模型与五道护栏、schema v8/v9/v10（goals / goal_rounds.summary / goals.auto_advance）、store（事务化 update/persist）、service（校验、权限、角色选择——**必须群管理员 Agent**）、driver（runner/judge 注入 + 步数上限 + **判决标记协议**）、runtime（单驱动锁 + tick）、dispatch（轮询式 runner，**摘要取 final 文本**）、scheduler_adapter（写消息/建 run/状态映射/标记优先判定）、autopilot（扫描 + 冷却 + 失败隔离）、autopilot_loop（Tauri/Web 后台 10s 扫描）
- IPC：Tauri 9 命令 + Web 8 路由；前端 `GoalStrip`（新建/启动/暂停/批准/停止/推进一步/自动推进勾选/明细账本）

**门禁修复（前置）**
- 全仓 `cargo fmt` 基线、恢复 lint/format 门禁、`run-bash.mjs`（Windows Git Bash 启动器）

**P4 插件化（✅ 单一路径）**
- 四个内置插件：`extension_host`（`svc.extensions`）、`adapter_catalog`（`svc.adapters`）、`workflow`（`svc.workflow`，读写全覆盖）、`marketing`（`svc.marketing`，读写全覆盖）
- 调用方收敛：Web 14 处 + 桌面 IPC 14 处 + scheduler/agent 自检全部只走 `require_*_service`；内核未装载返回明确错误（Web 503），**不再有自由函数回退**
- `commands.rs` 按领域拆为 `commands/{chat,goals,workflow,marketing,members,pm,observability,ops,update,agents}.rs` + `mod.rs`（1962 行 → 15KB 组装层）
- 新增 capability 契约：`openspec/specs/workflow/spec.md`、`openspec/specs/marketing/spec.md`、`changes/.../specs/plugin-migrations/spec.md`

**发布与验收**
- PR #20：分支 `codex/agent-playground-2026-10-22`，68 个提交；PR stability gate（Rust + Windows Tauri package）多次通过
- 像素场景验收截图：`output/scene-acceptance2.png`、`scene-room-scope.png`、`scene-sprites2.png`、`scene-zones.png`
- Goal 账本验收截图：`output/goal-ledger.png`
- 最终冒烟：adapters 7 项 / version-board / 创建版本（draft+planning）/ mock goal 一轮 verdict=progress

## Key paths

- `openspec/`：行为契约；`docs/version-pipeline.md`：方向 SSOT
- `src-tauri/src/kernel/`、`src-tauri/src/plugins/`、`src-tauri/src/goals/`
- `src/scene/`、`src/goals/GoalStrip.tsx`
- `src-tauri/src/db_migrations.rs`（v8–v10）、`src-tauri/src/web.rs`（goal 路由）、`src-tauri/src/commands.rs`（goal 命令）

## Locked product decisions

- 插件内核：Rust + TS 双同构，两端行为用例逐条对应；v0 只支持编译期注册，不做动态库
- 信任模型：v1 只支持本机自写插件，进程内无沙箱（已在 design.md 记录风险）
- 像素前台是「壳插件」：只读投影、功能开关、可回退，不改 schema/IPC
- Goal 写权限：平台管理员或群主；普通成员只读
- 自动推进默认关闭（`auto_advance=0`），必须显式开启
- 判定优先采用 Agent 的显式判决标记（`[GOAL:MET]` 等），无标记才回退关键词启发式

## Known pitfalls

- **SQLite 写锁**：`runtime::tick` 不能把 `driver.step` 包在目标事务里——runner 会自己写库（插消息/建 run），两个连接抢锁会失败。现在是「先执行、后事务写回」。
- **tokio 上下文**：`schedule_group` 内部 `tokio::spawn`，自动推进循环必须跑在 tokio 运行时里；裸线程会 panic。
- **PowerShell 文本往返会毁 UTF-8**：本会话两次把中文注释写成 mojibake（`service.rs`）。所有文本改动只用编辑器工具。
- **内核是进程级 `OnceLock`**：桌面与 Web 共享同一容器；按群隔离通过 `ctx.fork()` 显式派生，不自动按群隔离。
- **Web 前端与桌面 API 双份**：新增接口要同时改 `api.ts` / `api-web.ts` / `types.ts`，否则 Web 构建缺方法（`build:web` 不跑 tsc，只有 `test:gate` 的 `tsc -b` 会抓）。
- **Web 出包必须 `pnpm run build:web`**：误用桌面版 `pnpm run build` 会覆盖 `dist/`，浏览器打开会报 `Cannot read properties of undefined (reading 'invoke')`。
- **CI 浮动 stable ≠ 本地工具链**：CI 用 `dtolnay/rust-toolchain@stable`，2026-10 已是 1.99；`cfg(not(windows))` 分支的 lint 本地（Windows）看不到。建议钉 `rust-toolchain.toml` 或保持本地 stable 同步。
- **提交 subject ≤72 字符**：AI Harness 会按字符数校验，中文长主题容易超限。
- **本机无可用真实 Agent 凭据**：codex 403 unsupported_country、cursor-agent 未登录、claude 401、opencode 订阅过期、dsh credentials.yaml 格式错误——Goal 真实 CLI E2E 需换机器。

## How to run / verify

```bash
pnpm run test:gate        # vitest 42 + cargo 302 + clippy + fmt + lint + colors + build:web
cd src-tauri && cargo test --no-default-features --lib
pnpm run check:ai
```

- 像素场景：`pnpm run build:web` → 起服务 → 进项目群 → 标题栏 ◧
- Goal：群顶部 Goal 条新建 → 启动 → 「推进一步」（会真实发消息给 Agent）→「明细」看轮次账本

## Do not regress

- 不改既有 `tauri::command` 签名、Web 路由与 SQLite 既有表；新增迁移只加列/加表
- 场景开关关闭时零行为差异；Goal 条只出现在项目群
- 内核装载失败必须 fail-open（退回空上下文），不能让应用起不来

## Open follow-ups

- **已推送**：PR #20（`codex/agent-playground-2026-10-22`，68 个提交）；备份 tag `backup/pre-vision-2026-10-21`
- **P4 收尾**：workflow / marketing / adapters / extension_host 已全部单一路径；剩 `App.tsx` 入口可按领域继续下沉，以及第三方插件实装样例
- **真实 CLI E2E**：本机五条 CLI 均因凭据/地区不可用（见 pitfalls），需换机器补跑（task 3.7b）
- **场景**：视觉验收、分群房间、造型区分、功能区已完成；tileset / 房间内家具 / 点击进房间未做
- **生产仍跑旧壳**（`:8080` 标题仍是 Linli's Workpanel），`update.json` 两端都未部署；晋升生产需 root 批准
