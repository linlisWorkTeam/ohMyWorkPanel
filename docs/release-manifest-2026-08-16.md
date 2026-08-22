---
date: 2026-08-16
topic: release-manifest-dsh-self-bootstrap
status: checklist
---

# 发布变更清单（Release Manifest）：DSH 自举接入 P0 + P2

> 发布流程见 `docs/release-runbook-2026-08-16-dsh-self-bootstrap.md`。
> 本清单 = 本次待合入/发布的**全部文件改动**，供有 shell 的执行者 `git diff` 核对。

## A. 代码（P0 + P2）

| 文件 | 改动 |
|---|---|
| `src-tauri/src/adapters/dsh.rs` | **新增**：DeepSeek Harness headless 适配器（`--profile headless`） |
| `src-tauri/src/adapters/mod.rs` | `AdapterKind::Dsh`（parse/as_str/candidates/build_args/单测） |
| `src-tauri/src/adapters/models.rs` | `"dsh" => &[]`（模型归 dsh profile） |
| `src-tauri/src/models.rs` | `Member.system_locked: bool` |
| `src-tauri/src/db.rs` | 迁移 `agent_profiles.system_locked`；`MEMBER_SELECT` + `member_from_row`；`ensure_ohmyworkpanel_super_harness`、`ensure_minimal_bootstrap_dsh`、`assert_member_mutable` |
| `src-tauri/src/commands.rs` | 守卫（remove/set_admin/改模型/改工作区）+ 建群 seed 极简 bootstrap |
| `src-tauri/src/web.rs` | 守卫（remove/purge/admin/model/workspace）+ 建群 seed 极简 bootstrap |
| `src/types.ts` | adapter 联合加 `"dsh"`；`Member.systemLocked` |
| `src/agentModels.ts` | `dsh: []` |
| `src/App.tsx` | `DSH_WEB_URL`、`DSHView`、适配器选项、`mainView==="dsh"` 分支、成员「跳转 DSH Web」、`systemLocked` 只读 + 「系统」徽标 |
| `src/styles.css` | `dsh-view` 样式 |

## B. 文档（设计 + 发布）

| 文件 | 说明 |
|---|---|
| `docs/superpowers/specs/2026-08-16-dsh-self-bootstrap-runtime.md` | 总设计（两级自举 Agent / 闭环 / 路线 P0–P4 / 护栏） |
| `docs/superpowers/specs/2026-08-16-group-chat-governance-plane.md` | 群聊治理层（决策卡/审批/slash） |
| `docs/superpowers/specs/2026-08-16-dsh-ui-language-ohmyworkpanel.md` | DSH UI 三栏语言（工作区=群聊、右栏=Agent） |
| `docs/superpowers/plans/2026-08-16-dsh-self-bootstrap-plan.md` | 执行计划（P0 ✅ / P2 ⚠️ 待 build·发布） |
| `docs/release-runbook-2026-08-16-dsh-self-bootstrap.md` | 发布 Runbook（本地→GitHub→灰度→生产） |
| `docs/release-manifest-2026-08-16.md` | 本变更清单 |
| `docs/epitaph/2026-08-16-dsh-self-bootstrap-design.md` | 交接墓志铭（+ 索引登记） |
| `docs/version-pipeline.md` | SSOT 轨道 G 占位（两级不可改自举执行者） |
| `README.md` | 适配器表 `dsh` 行 + 文档索引 |

## C. 发布前必须核对（重点）

1. `cargo build` 通过；若无，检查：
   - `Member { ... }` 字面量是否遗漏 `system_locked`（唯一标准构造点是 `db::member_from_row`，已补）；
   - `put_member_workspace_web` 里出现**两个连续 `let conn`**（Rust 同名遮蔽合法、可编译，建议顺手合并为一个）。
2. `cargo test --no-default-features --lib` + `pnpm run test:gate` + `pnpm run build:web` 全绿。
3. 启动后自迁移 `system_locked` 列；ohMyWorkPanel 种子组出现只读「系统」`linlis-super-harness`；新建项目群出现极简 `bootstrap-dsh-<group>`。
4. 对锁定 Agent 调 remove/admin/改模型/改工作区（Tauri 与 Web 两条路径）→ 后端拒绝“平台锁定的自举 Agent 不可修改或移除”。
5. `dsh web` :3080 时，「跳转 DSH Web」内嵌可用。

## C2. Git 提交 / PR 描述（可直接复制）

```bash
cd /AI/ohMyWorkPanel
git add -A
git commit -m "feat: dsh 自举执行者 P0+P2（适配器/DSH Web 嵌入/两级 bootstrap/system_locked 只读）"

# PR / 发布说明正文（copy-paste）
feat: DSH 自举接入 P0 + P2

- P0: dsh headless 适配器 + 群聊「跳转 DSH Web」内嵌 :3080
- P2: 两级不可改自举 Agent（普通群 bootstrap-dsh-<group> / ohMyWorkPanel 组 linlis-super-harness）
  - agent_profiles.system_locked 幂等迁移 + Member.systemLocked
  - 种子/建群自动落位；桌面(commands)与 Web(web.rs) 成员 mutation 全守卫
  - 前端成员行只读 + 「系统」徽标
- 文档：3 份 design spec + plan + epitaph + runbook + manifest

验证：cargo test --no-default-features --lib && pnpm run test:gate && pnpm run build:web
发布：docs/release-runbook-2026-08-16-dsh-self-bootstrap.md（ECS 灰度:8081 → 生产:8080）
```

## D. 印证“全做完”的达成标准

- [ ] 本地 test:gate / cargo test / build:web 全绿
- [ ] 已合入 GitHub master
- [ ] ECS 灰度 :8081 验证通过（成员只读 + dsh Agent 任务 + DSH Web 内嵌）
- [ ] 灰度转生产 :8080（approve + promote + 冒烟）
- [ ] 对外发布口径文档齐备（3 份 design spec + runbook + manifest）

> 当前会话：bash 被禁用 + 无外网，上述 D 无法代为执行；代码与文档均已落盘待发布。
