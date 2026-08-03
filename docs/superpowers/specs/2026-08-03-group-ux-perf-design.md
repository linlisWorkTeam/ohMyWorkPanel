# Design: 群 UX 增强 + 性能指标（2026-08-03）

## Locked decisions

| 项 | 选择 |
|---|---|
| Codex app-server | **搁置**（1.8G 主机不扩容） |
| 归档交互 | **1A**：群项「−」归档；底部「已归档」可展开/取消归档 |
| Agent 模型 | **2B**：所有 Agent（CLI + chatbot）均可选模型，跑任务时传入 |
| 群类型 | `project`（默认，现行为） / `chat`（纯聊天，可多 chatbot，无项目视图） |

## Features

### 1. 发送快捷键
- 本地偏好 `sendKeyMode`: `enter` | `ctrlEnter`（localStorage）
- 发送区旁切换；默认 `enter`；换行：Enter 模式用 Shift+Enter，Ctrl+Enter 模式用 Enter

### 2. 每 Agent 模型
- `agent_profiles.model TEXT`（可空 = CLI/提供方默认）
- 目录按 adapter 固定可选列表；API `PUT /api/members/{id}/model`
- 调度：chatbot JSON `model`；CLI 传 `--model` / `-m`（空则不传）

### 3. 归档群
- `groups.archived INTEGER DEFAULT 0`
- `PUT /api/groups/{id}/archive` body `{ archived: bool }`
- 侧栏：活跃群 + 折叠「已归档」区

### 4. 聊天群 / 项目群
- `groups.group_kind`：`chat` | `project`（缺省 `project`）
- 建群弹窗先选类型；聊天群不要求工作区（存空串）；项目群保持现逻辑
- 聊天群：允许多 chatbot；隐藏「项目」视图与编排入口
- 项目群：chatbot 仍限 1 个（现规则）

### 5. 性能指标与日志
进程（WorkPanel server）目标（不含子 Agent CLI）：

| 指标 | 健康 | 告警 | 危险 |
|---|---|---|---|
| RSS | ≤ 80 MiB | > 120 MiB | > 200 MiB |
| CPU（采样窗口） | ≤ 5% 空闲均值 | > 25% 持续 | > 50% 持续 |

- 每 60s 写 `logs`：`source=perf`，message 含 `rss_mb` / `cpu_pct` / `warn|ok`
- 超阈值用 `warn` level，便于后续分析

## Out of scope
- Codex app-server 真流式
- 主机扩容
- 归档物理删除群
