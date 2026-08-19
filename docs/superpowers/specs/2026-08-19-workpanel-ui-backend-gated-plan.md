---
date: 2026-08-19
topic: workpanel-ui-backend-gated-plan
status: implemented-2026-08-20
---

> **实施状态（2026-08-20）**：四个方案全部落地并灰度验证——
> 1. ✅ run 审批内联（`set_run_review` + `/api/runs/{id}/review`，cargo 单测）
> 2. ✅ 消息反馈 👍/👎（`message_feedback` 表 + `/api/messages/{id}/vote`，单测）
> 3. ✅ run 轨迹（`run_phase_log` 表 + `/api/runs/{id}/phases`，单测）
> 4. ✅ 斜杠命令→决策卡（`/board /approve /wave`，`workflow::try_slash_command` + 单测；项目群 + 用户成员，纯 conn 不产生 run；composer `/` 提示）
> 均为增量命令/表，未破坏既有 schema/签名；cargo test 122 passed、vitest 72 passed、双构建通过。

# 设计：剩余 P2 的后端最小扩展方案（run 审批 / 决策卡 / 消息反馈 / run 轨迹）

> 配套 `2026-08-19-workpanel-dsh-ui-design.md`（P2 段）。
> 前端骨架已就位：「待审批」信息卡（右栏·队列）、消息悬停操作条（赞/踩预留 mini-btn）、goal bar（Wave 常驻条）。
> 本文件只解决**缺后端命令/数据**的四项，全部为**增量命令（非破坏）**，走完整发布门禁。

## 原则

- **不破坏**既有 `tauri::command` 签名 / SQLite 既有表结构（只加新命令、新表/新列）。
- 每项前端不动 IPC 抽象层：`src/api.ts` 加 async 方法 + `src/api-web.ts` 对应 endpoint。
- 门禁：`cargo test --no-default-features --lib` → `pnpm run test:gate` → 灰度 `:8081` 冒烟（§F）→ commit → 批准 → promote。

---

## 1. run 审批内联（approval-composer，P2 最接近）

**现状**：`task_runs` 已有 `review_status(pending|approved|rejected)` + `reviewer_member_id`；调度器在 `scheduler.rs` 让批复 Agent 自动处理 `pending`（`UPDATE task_runs SET review_status=?,status=?`）。**缺**用户侧裁决命令。

| 层 | 改动 |
|---|---|
| Rust | 新增 `#[tauri::command] async fn set_run_review(run_id, decision: "approved"|"rejected")`；approved → `review_status='approved', status=按上下文`；rejected → `review_status='rejected', status='changes_requested'` 以便重试。加 `db::update_run_review` + 单测 |
| Web | `POST /api/runs/{id}/review`（复用 `web.rs` 现有鉴权） |
| 前端 | 右栏队列「待审批」卡的 `批准 / 拒绝` 按钮从占位变真功能（调 `api.setRunReview`），完成后 `refresh()` |

**风险**：低（增量列已存在；调度器既有 `pending` 分支兼容，人类裁决与 Agent 裁决语义需在 scheduler 侧确认互斥——建议：`reviewer_member_id` 为空或为 admin 用户时允许人类裁决）。

## 2. 决策卡 / 斜杠命令（`/ask /propose /approve /wave /release`）

**现状**：后端**无**斜杠命令解析（已核实：只有 `@` 提及触发）。Workflow API（`approveVersionWaves / playWave / advanceWave / playVersion`）已存在。

| 层 | 改动 |
|---|---|
| Rust | 消息 intake（`scheduler.rs` 处理用户消息处）加斜杠命令路由：`/wave <title>`→建 Wave、`/approve`→批准当前 asking 版本、`/ask <what>`→把 `project_versions` 置 `asking`、`/release`→标记待发布。返回系统消息回显 |
| Web | 无（走消息通道） |
| 前端 | composer 加斜杠命令提示 popover（`/` 触发，列出真实命令），goal bar 点击不变 |

**风险**：中（改调度 intake，需覆盖单测；命令集先固定 `wave/approve/ask/release` 四条）。

## 3. 消息反馈 👍/👎（ui-message-feedback）

**现状**：右键联 nav「阅读全文 / 回放」等无；悬停操作条有复制（已实现）、赞/踩占位未接真功能。

| 层 | 改动 |
|---|---|
| DB | 新表 `message_feedback(message_id, member_id, vote CHECK(vote IN ('up','down')), created_at, PRIMARY KEY(message_id, member_id))` |
| Rust | `vote_message(message_id, vote)` upsert；`get_message_feedback(message_id)` 聚合计数 |
| 前端 | 悬停条 `👍 有用 / 👎` 调 API，本地乐观更新（demo 一致） |

**风险**：低（纯增量表 + 两个命令）。

## 4. run 轨迹视图（会话回放）

**现状**：`message-parts`（thinking/artifact/tool/command）已是逐包轨迹的 UI 载体；`task_runs.phase/phase_updated_at` 记录当前阶段（无历史表）。

| 层 | 改动 |
|---|---|
| DB | 可选新表 `run_phase_log(run_id, phase, at, note)`（调度器在每次 `append_delta`/phase 变更时追加一行） |
| Rust | `GET /api/runs/{id}/phases` 返回按时间排序的阶段日志 |
| 前端 | 队列卡加「轨迹」展开：列出阶段时间线；气泡内思考/产物保持现状 |

**风险**：中（调度热点路径加一行插入，注意锁粒度：phase 写入走 `MutexGuard` 同步函数，勿在 async 闭包持有）。

---

## 实施建议顺序（与 UI 依赖对齐）

1. **run 审批**（P2 最接近、依赖最少）→ 前端待审批卡变真功能
2. **消息反馈**（低风险纯增量）→ 悬停条补真反馈
3. **斜杠命令**（中风险，改调度 intake）→ 决策卡/composer 命令菜单
4. **run 轨迹**（中风险，热点路径）→ 队列卡轨迹时间线

每步独立灰度回归；**不做**的：改既有表结构、破坏 `tauri::command` 签名、无门禁直推灰度。

## 验收（四项全部后）

- 待审批卡可 批准/拒绝 并持久化；消息可赞/踩并累计；`/wave /approve /ask /release` 在聊天中生效并回显；队列卡可展开 run 阶段时间线。
- `pnpm run test:gate` 全绿 + `cargo test --lib` 新增用例全过；灰度 §F 冒烟通过。
