---
date: 2026-08-05
topic: workspace-boundary-live-host
status: decided
decider: Cursor Agent (platform admin)
---

# 拍板：Live Extend 与 Host 工作区边界

## 代码层面（已确认）

| 树 | 路径 | 职责 |
|---|---|---|
| Host | `/AI/LinlisWorkPanel` | 代理、`LivePanel`、短回复注入、A2A、群/成员 |
| Extend | `/AI/WorkPanelLive` | `:8790`、`live.html`、STT/TTS/DashScope |

Host 经 `LINLIS_PANELLIVE_ROOT`（默认 `/AI/WorkPanelLive`）读清单 — **路径耦合，不是混仓**。

## 群配置拍板

| 群 | 决定 |
|---|---|
| **WorkPanelLive** (`6426fb0c-…`) | **正式 Live 群**；`workspace_path=/AI/WorkPanelLive`；保持活跃 |
| **WorPanelLive** (`96dcc4fd-…`) | **错名废弃**：保持 `archived=1`；已改名为 `WorPanelLive（废弃·错名）`，`workspace_path` 纠正为 `/AI/WorkPanelLive`（勿再解档当工作群） |
| LinlisWorkPanel | 平台种子群；`/AI/LinlisWorkPanel` |

**不合并**两群（历史/成员 ID 不同）；以归档 + 纠正路径消除误导。

## 仓库边界

1. **`/AI/WorkPanelLive` 独立 git**（已本地 `git init`，初始提交不含 `.env` / `.venv`）。远程另配。
2. **改码规矩（写死）**  
   - STT / TTS / `live.html` / PanelLive `src/*` → **只动** `/AI/WorkPanelLive`  
   - Host 代理 / `LivePanel` / 短回复注入 / A2A → **只动** `/AI/LinlisWorkPanel`  
3. Agent 建群/选工作区时：Live 名不得绑平台路径。

## 风险

- 解档错名群仍会造成双 Live 群混乱 — 禁止解档，应用正式 `WorkPanelLive`
- WorkPanelLive git 尚无 remote — 需运维补推送目标
