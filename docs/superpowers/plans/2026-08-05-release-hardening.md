---
date: 2026-08-05
topic: release-hardening
status: draft
owner: OpenClaw (PM) → Cursor Agent (impl)
---

# 发版优化方案（P1–P4）

> root 指派 @OpenClaw 连带修复的发版优化四项。本文为平台侧方案；实现走
> 灰度 → docs → commit → 生产 流程，与 PanelLive 晋升（4043503+b5ea3df）互不阻塞。

## 现状盘点（已实测）

| 需求 | 现状 | 缺口 |
|---|---|---|
| P1 种子群不可删 + 跨工作空间 | `ensure_default_seed` 已建 `seed-group-workpanel`（root/Codex/OpenClaw/Cursor）；**无任何 delete group 路由**（删除天然不存在） | 无显式 `is_system` 标记/guard；agent workspace 全锁在群工作区下（`resolve_agent_workspace_under_group`），**无跨工作空间能力** |
| P2 发布断连 60s 超时重传 | 前端已有指数退避重连（max 30s）+ `ws_reconnected` 事件；App.tsx:386 仅**任务终态** resync | 无 60s 发布等待窗口；无心跳探活发布状态；重连后进行中任务不续传（丢 delta） |
| P3 Agent 响应心跳 + 聚焦感知 | 服务端 keepalive 仅 agent 档案保活（warm_status）；WS 有 20s 客户端心跳（NAT） | **无 run 进行中的进度心跳**；无聚焦感知；无动态频率；运行设置无心跳项 |
| P4 CPU/内存可视 | `metrics.rs` 已有 RSS/CPU 采样（/proc/self）→ logs(source=perf)，**60s 一次** | 无查询 API；前端不可见；要求 20s 后台存储 + 设置页 5s 刷新 |

## 方案

### P1 — WorkPanel 种子群加固

1. `groups` 表加 `is_system INTEGER NOT NULL DEFAULT 0`（迁移：ensure/migrate 时 `ALTER TABLE` 幂等）；`seed-group-workpanel` 置 1。
2. 删除守卫：任何删除群路径拒绝 `is_system=1`（现无删除功能，guard 留给未来）+ 单测。
3. 跨工作空间：`resolve_agent_workspace_under_group` 增加特例——**仅种子群 + agent 显式配置绝对 workspace_path** 时允许绝对路径（跳出群目录），其余行为不变。
4. 信任边界说明：放宽仅限 WorkPanel 自维护群；文档写入 epitaph。

### P2 — 发布断连 60s 超时重传

1. 前端 WS 状态机：`connected → releasing(≤60s) → connected | timeout`。
   - onclose 进入 releasing：顶部横幅「发布中/重连中 Ns」；
   - 期间每 5s `fetch /api/health` 探活（轻量、无鉴权或复用会话）；
   - 探活恢复但 WS 未回 → 继续等待；60s 超时 → 提示手动刷新。
2. 重连成功（ws_reconnected）触发**全量 resync**：群列表 + 当前群 + 进行中 runs 完整状态 + 最近消息增量（`since` 游标）。
3. 后端补 `GET /api/tasks/active`（或复用现有 run 列表）+ WS 事件带单调 `seq`，前端按 seq 去重补拉。
4. 进行中 run 的 delta 快照保留（后端 run 内存态已有），resync 时一次性补齐。

### P3 — Agent 响应心跳 + 聚焦感知动态频率

1. 后端：run 进行中每 **N 秒** 发 `run_heartbeat` WS 事件 `{run_id, status, elapsed_ms, delta_count, rss_mib}`；N 默认 5s，可在设置覆盖（聚焦 1s / 后台 10s）。
2. 前端聚焦感知：`document.visibilityState` + 当前视图是否为该 run 所在群 → 聚焦/非聚焦。
3. Auto 模式（默认）：
   - 聚焦 → 心跳 1s（实时性靠 WS 推送，**不做 100ms HTTP 轮询**；100ms 级由后端 delta 推送 + 前端节流呈现）；
   - 非聚焦 → 5s；
   - 内存压力（`navigator.deviceMemory` ≤4 或后端 rss 高）→ 频率自动降档。
4. 运行设置新增「心跳」分组：聚焦频率 / 非聚焦频率 / Auto 开关，**实时打印当前生效频率**（如 `心跳：聚焦 1s · 后台 5s（Auto）`）。

### P4 — CPU/内存可视化

1. `metrics.rs`：采样间隔 60s → **20s**（后台存储不变）。
2. 新增 `GET /api/metrics/latest` → `{rss_mib, cpu_pct, ts}`（读 logs source=perf 最近一条，或内存缓存最新采样）。
3. 前端：运行设置打开时显示 CPU/RSS 卡片，**5s 轮询**；设置关闭即停轮询（后台仍 20s 存储）。
4. MVP 仅主进程（/proc/self）；agent 子进程统计下一迭代。

## 任务拆分（Cursor Agent）

| 任务 | 内容 | 验收 |
|---|---|---|
| M1 | P1：is_system 迁移 + seed 标记 + 删除 guard + workspace 特例 | 单测绿；种子群不可删；种子群 agent 可用绝对 workspace |
| M2 | P2：后端 resync/seq + 前端 releasing 状态机（60s + 探活 + 全量 resync） | 杀 WS 进程模拟发布：60s 内自动恢复，进行中 run 不丢 delta |
| M3 | P3：run_heartbeat + 聚焦感知 + 设置项（Auto/手动 + 当前频率展示） | 聚焦 1s/后台 5s 生效；设置页显示当前频率 |
| M4 | P4：perf 20s + /api/metrics/latest + 设置页 5s 卡片 | 设置页打开可见动态刷新；关闭后停轮询 |
| M5 | docs（roadmap、api-web.md、epitaph）+ 灰度冒烟 → commit → 生产 | 群规则全流程 |

## 对齐点（待 root 确认）

- **Q1 跨工作空间范围**：建议仅「种子群 + 显式绝对 workspace 的 agent」放宽，默认不放大 —— 同意？
- **Q2 100ms 级实时**：不做 100ms HTTP 轮询，用 WS 推送 + 前端节流呈现 —— 同意？
- **Q3 排序**：P1–P4 与 PanelLive 晋升并行；PanelLive 晋升优先（已复核通过），本批完成后单独走晋升 —— 同意？

## 风险

1. 跨工作空间放宽是信任边界变更：仅限种子群，防止普通群 agent 越权读盘。
2. releasing 状态机与现有重连逻辑叠加，需回归测试（ws_reconnected 已存在路径）。
3. perf 20s 写入量：logs 表增长 ~3x，量级仍可忽略（每行几百字节）。

## 实现约束（root / 测试方补充）

- **调用嵌套深度 ≤ 3**：Agent↔平台↔Extend（含 A2A / Live / 委派）链路不得形成超过 3 层的同步嵌套调用，防止 deadloop；实现时需带 depth/hop 计数并在超限时拒绝或降级。
- **发版节奏**：先由测试方完成验收；**管理员发版**（approve + promote）。实现 Agent 在收到开工指令前不自动 promote。

## 状态

- **等待测试完成 + 管理员发版指令**；Cursor Agent 暂缓 M1–M5 大规模实现，直至明确开工。
