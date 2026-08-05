---
date: 2026-08-05
topic: release-hardening-p1-p4
status: active
---

# Epitaph: 发版优化 P1–P4（灰度）

## 交付

| 项 | 内容 |
|---|---|
| P1 | `groups.is_system`；种子群标记；`assert_group_deletable`；种子群 agent 可绝对 workspace |
| P2 | WS `releasing`≤60s + `/api/health` 探活；`ws_reconnected` 全量 resync + `/runs/active`；事件 `seq` |
| P3 | `run_heartbeat` WS；设置心跳 Auto/聚焦/后台；前端展示当前频率（无 100ms 轮询） |
| P4 | perf 采样 20s；`GET /api/metrics/latest`；设置页打开时 5s 拉指标 |

## 约束

- 调用嵌套 ≤3；生产 promote 须管理员批准
- 灰度公告：`scripts/canary-announce-a2a.sh`

## 风险

- 跨工作空间仅种子群；普通群仍锁群目录
- releasing 与指数退避重连叠加，需人工断网/重启冒烟
