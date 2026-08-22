---
date: 2026-08-06
topic: extension-host-v130-claim
status: implementing
source: /AI/AIHotel/docs/superpowers/specs/2026-08-06-ohmyworkpanel-extension-host-v130-requirements.md
---

# 回执：通用扩展宿主纳入 ohMyWorkPanel V1.3.0

| 项 | 答复 |
|---|---|
| **是否纳入 V1.3.0** | **是**（与工作流同大版本；纯平台宿主重构，不含 AIHotel 业务） |
| **负责人** | ohMyWorkPanel **Cursor Agent**（实现）；OpenClaw 协助评审/运维配置；Codex 可接纯函数/单测切片 |
| **延期版本** | N/A（不延期） |
| **目标切片日期** | 见下表（以灰度可验双扩展为里程碑） |

## 切片排期

| 序 | 切片 | 目标日（UTC+8） |
|---|---|---|
| S0 | `OHMYWORKPANEL_EXTENSION_ROOTS` + 多扩展 `GET .../extensions` | ✅ 代码 |
| S1 | 通用 `PUT .../extensions/{extId}` + `ANY /api/extensions/{extId}/{*path}` | ✅ 代码 |
| S2 | 前端遍历 `tabs` + 通用 `ExtensionPanel` iframe（Live 桥可暂特判） | ✅ 代码 |
| S3 | 设置页多扩展开关 | ✅ 代码 |
| S4 | A2A skills 按 manifest | ✅ 代码 |
| S5 | 纯净度进 `test:gate` + docs + PanelLive 回归 | 纯净度/docs ✅；**灰度回归待 canary** |

**门禁目标**：2026-08-09 前灰度满足需求单 §6（双扩展发现、通用页签、PanelLive 零回归）。V1.3.0 **promote 生产**仍单独审批，且须 PanelLive 冒烟通过。

## 不做

- 不为 ai-hotel 抄 `proxy_aihotel` / `HotelPanel`
- 不把剧本/NPC/好感度写进平台仓
- AIHotel 引擎 `:8791` 仍由 AIHotel 仓并行交付

## 兼容

- `PUT .../extensions/panellive` 与 `/api/extensions/panellive/...` 继续可用（通用路由 `{extId}`）
- `OHMYWORKPANEL_PANELLIVE_ROOT` 回落并入扩展根列表
