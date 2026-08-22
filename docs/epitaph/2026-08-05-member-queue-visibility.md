---
date: 2026-08-05
topic: member-queue-visibility
branch: master
status: active
---

# Epitaph: 成员面板同 Agent 排队数可见

## Built this session

- **纯前端**：`queueCounts` / `agentBusyLabel` / `runsForAgentActive` + Vitest；零后端 API
- **成员栏**：`执行中` / `执行中 · 排队 N` / `排队 N`；空闲恢复 runtime 文案
- **展开取消**：点击忙态文案展开该 Agent 的 queued+running，复用 `cancelRun`
- **验证**：`test:gate`；灰度 `:8081` + A2A 公告（见发布记录）

## Key files

| 文件 | 说明 |
|---|---|
| `src/queueCounts.ts` | 聚合与文案 |
| `src/queueCounts.test.ts` | 单测 |
| `src/App.tsx` | `MemberRow` 接入 |
| `src/styles.css` | 展开列表样式 |
| `docs/superpowers/plans/2026-08-05-member-queue-visibility.md` | 方案 |

## Locked product decisions

| 项 | 选择 |
|---|---|
| 数据 | 现有 `current.runs` + `run_status` WS；无新 API |
| M3 | 本期做展开+取消 |
| 生产 | 另批 promote |

## Do not regress

- 同 Agent 串行（`plan_queued_starts`）
- 空闲时不得残留「执行中/排队」文案
- 多 Agent 计数不得串

## Open follow-ups

- 连续 @ 两次灰度手工验收
- 管理员批后 promote
