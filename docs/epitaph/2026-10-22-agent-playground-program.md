---
date: 2026-10-22
topic: agent-playground-program
branch: main
status: active
---

# Epitaph: Agent 游乐园程序（插件内核 / Goal 自治 / 像素前台）

## Built this session

从 `origin/main`（`1bf4c58`）出发，本地 34 个提交，覆盖四条轨道：

**P0 契约**
- `openspec/`：`project.md`（愿景/原则/架构地图）、`specs/`（kernel、extensions 契约）、`changes/0001-agent-playground/`（proposal / design / tasks + scene、goals delta）
- `docs/version-pipeline.md` 新增「阶段 9 — Agent 游乐园」

**P1 插件内核（Rust）**
- `src-tauri/src/kernel/`：`Context`（服务注册表 / 类型安全取回 / 释放后拒绝注册）、`EffectStack`（逆序回收、幂等）、`EventBus`（emit / parallel / serial / bail）、`PluginSpec` + `load_plugins`（拓扑排序、声明兑现校验、失败整体回滚）、`plugin.manifest.json` v2（解析 / 目录发现 / 注册表映射）
- `src-tauri/src/plugins/`：进程级内核 `OnceLock` + 第一个内置插件 `extension_host`（`svc.extensions`），Web 反代与 A2A 技能校验已走该服务，缺失时回退自由函数

**P2 像素前台**
- `src/scene/`：`sceneModel`（member/run → 角色，确定性布局、候场区换行）、`PixelScene`（Canvas 2D、DPR 适配、30fps 上限、地板离屏缓存、四态动画、选中高亮）、`SceneView`（HUD / 空态 / 返回聊天）
- 接入方式：`mainView === "scene"`，header ◧ 开关，输入框隐藏；数据只读，不改后端

**P3 Goal 自治**
- `src-tauri/src/goals/`：领域模型与五道护栏、schema v8/v9/v10（goals / goal_rounds.summary / goals.auto_advance）、store（事务化 update/persist）、service（校验、权限、角色选择）、driver（runner/judge 注入 + 步数上限）、runtime（单驱动锁 + tick）、dispatch（轮询式 runner）、scheduler_adapter（写消息/建 run/状态映射/启发式 judge）、autopilot（扫描 + 冷却 + 失败隔离）、autopilot_loop（Tauri/Web 后台 10s 扫描）
- IPC：Tauri 8 命令 + Web 7 路由；前端 `GoalStrip`（新建/启动/暂停/批准/停止/推进一步/自动推进勾选）

**门禁修复（前置）**
- 全仓 `cargo fmt` 基线、恢复 lint/format 门禁、`run-bash.mjs`（Windows Git Bash 启动器）

**P4 插件化（进行中）**
- 第二个内置插件 `adapter_catalog`：`svc.adapters`（list / resolve / candidates），Web `/api/adapters` 与 scheduler 执行路径、agent 自检均已接入并保留回退
- 第三个 `workflow`：`svc.workflow`（board / create_version / update_roadmap / play_wave / pause_wave），Web 版本板、创建、Roadmap、Wave 播放/暂停全部接入
- 第四个 `marketing`：`svc.marketing`（list / get），Web campaign 列表与详情接入
- 新增 capability 契约：`openspec/specs/workflow/spec.md`、`openspec/specs/marketing/spec.md`、`changes/.../specs/plugin-migrations/spec.md`

## Key paths

- `openspec/`：行为契约；`docs/version-pipeline.md`：方向 SSOT
- `src-tauri/src/kernel/`、`src-tauri/src/plugins/`、`src-tauri/src/goals/`
- `src/scene/`、`src/goals/GoalStrip.tsx`
- `src-tauri/src/db_migrations.rs`（v8–v10）、`src-tauri/src/web.rs`（goal 路由）、`src-tauri/src/commands.rs`（goal 命令）

## Locked product decisions

- 插件内核：Rust + TS 双同构是终态，但当前只做了 Rust；v0 只支持编译期注册，不做动态库
- 信任模型：v1 只支持本机自写插件，进程内无沙箱（已在 design.md 记录风险）
- 像素前台是「壳插件」：只读投影、功能开关、可回退，不改 schema/IPC
- Goal 写权限：平台管理员或群主；普通成员只读
- 自动推进默认关闭（`auto_advance=0`），必须显式开启
- 判定先用确定性关键词（`HeuristicJudge`），Agent 判定留后续

## Known pitfalls

- **SQLite 写锁**：`runtime::tick` 不能把 `driver.step` 包在目标事务里——runner 会自己写库（插消息/建 run），两个连接抢锁会失败。现在是「先执行、后事务写回」。
- **tokio 上下文**：`schedule_group` 内部 `tokio::spawn`，自动推进循环必须跑在 tokio 运行时里；裸线程会 panic。
- **PowerShell 文本往返会毁 UTF-8**：本会话两次把中文注释写成 mojibake（`service.rs`）。所有文本改动只用编辑器工具。
- **内核是进程级 `OnceLock`**：桌面与 Web 共享同一容器；按群 fork 子上下文尚未实现。
- **Web 前端与桌面 API 双份**：新增接口要同时改 `api.ts` / `api-web.ts` / `types.ts`，否则 Web 构建缺方法。

## How to run / verify

```bash
pnpm run test:gate        # vitest 140 + cargo 281 + clippy + fmt + lint + colors
cd src-tauri && cargo test --no-default-features --lib
pnpm run check:ai
```

- 像素场景：起服务 → 进项目群 → 标题栏 ◧
- Goal：群顶部 Goal 条新建 → 启动 → 「推进一步」（会真实发消息给 Agent）

## Do not regress

- 不改既有 `tauri::command` 签名、Web 路由与 SQLite 既有表；新增迁移只加列/加表
- 场景开关关闭时零行为差异；Goal 条只出现在项目群
- 内核装载失败必须 fail-open（退回空上下文），不能让应用起不来

## Open follow-ups

- **未推送**：本地 34 个提交仅在本地；备份 tag `backup/pre-vision-2026-10-21` 已推
- **P4 未完成**：workflow 还剩 Ask / 发布写入；marketing 还剩生成 / 审核 / 导出；`commands.rs` / `App.tsx` 入口未瘦身
- **真实 Agent 端到端**：Goal 的自动循环未用真实 CLI 验证过（task 3.7）
- **场景视觉验收**：未在真实群数据下人工看过；tileset / 房间分区未做
- **TS 同构内核**、`waterfall` 事件、按群作用域 fork 未做
- **生产仍跑旧壳**（`:8080` 标题仍是 Linli's Workpanel），`update.json` 两端都未部署
