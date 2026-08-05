# 成员面板：同 Agent 排队数可见

- 日期：2026-08-05
- 提出：root（Feature「成员面板：同 Agent 排队数可见」+ 3 条 checklist）
- 状态：已完成（M1–M4；canary :8081 已部署+A2A；commit `5598fe0`；生产另批）

## 背景

同一 Agent 被多人/连续 @ 时是串行排队（`plan_queued_starts`，同 Agent 并发 1），
但右侧成员状态栏只显示布尔态「生成回复中」，看不出后面排了几单，易误判为卡住。

## 勘察结论（关键：无新后端 API）

| 数据/能力 | 现状 | 是否够用 |
|---|---|---|
| 群内全量 runs（含 queued/running） | `GET /api/groups/{id}/runs` → `get_runs` 无状态过滤 | ✅ |
| 前端 runs 列表 | `current.runs`（App.tsx:901/979 已在用，含 queued） | ✅ |
| 状态变更推送 | `run_status` WS 事件（scheduler.rs:69/191/820/956 + web.rs cancel），App.tsx:423 已消费 | ✅ |
| 取消排队任务 | `POST /api/runs/{id}/cancel` 已支持 queued/running | ✅（可选 M3 用） |

结论：**纯前端聚合**即可，队列数量随既有 run_status 事件自动刷新，零后端改动。

## 方案

### M1 纯函数 + 单测（TDD）
`src/queueCounts.ts`：
```ts
export function queueCounts(runs: TaskRun[], agentMemberId: string): { running: number; queued: number }
```
- running = status==='running' 且 agentMemberId 匹配
- queued = status==='queued' 且 agentMemberId 匹配
- 单测：混合状态、他人 run 不计入、空列表

### M2 成员状态栏接入（App.tsx L1449 区域）
替换现有 `responding` 布尔逻辑为 counts 文案：
- running>0 → `执行中 · 排队 {queued}`（queued=0 时仅 `执行中`）
- running==0 && queued>0 → `排队 {queued}`
- 均无 → 保持现有空闲文案（已就绪/不可用/待检测）
- 保留现有 adapter/保活/检测 前缀后缀

### M3 可选：点击展开排队列表（checklist 4）
- 点击状态文案 → 弹出该 agent 的 queued+running 任务气泡列表（同源 `current.runs`）
- 每项「取消」按钮 → 复用 `POST /api/runs/{id}/cancel`（已支持 queued）
- 若本期不做，checklist 4 标记后续迭代

### M4 docs + 发布
- epitaph 记录 + plans 勾选；无路由变更（api-web.md 不动）
- 灰度 :8081 → canary-announce-a2a.sh 公告 → commit
- 生产晋升另批（管理员 approve）

## 验收标准
1. 连续 @ 同一 Agent 两次：状态栏显示「执行中 · 排队 1」，第二条完成后变回空闲文案
2. 状态变化（排队→执行→完成）时文案实时刷新，无需手动刷新页面
3. 多个 Agent 互不串计数；访客视角无越权（runs 权限现状不变）
4. `test:gate` 绿

## 风险
- 前端 runs 列表若被截断（分页）会漏计——当前 `get_runs` 全量返回，无分页，风险低
- M3 展开列表若做，注意取消按钮与调度器状态竞争（cancel 幂等，已处理）
