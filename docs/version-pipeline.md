---
date: 2026-08-05
topic: version-pipeline
status: active
---

# LinlisWorkPanel 版本流水线（发展方向锁定）

> **本文是产品/平台演进的单一事实源（SSOT）。**  
> 新功能先在本文件占位（轨道 + 阶段），再开 Feature / 写 epitaph；禁止「东改西改」无版本归属的散弹改动。  
> 细节仍以 `docs/epitaph/*` 与 `docs/superpowers/{specs,plans}/*` 为准；本文件只看**流水线与方向**。  
> 当前生产产物：豆包语音 UX 等已 promote（`prod` RELEASE ≈ `2026-08-05T16:49:03Z`）。Git 标签待补 **`v1.2.0`**；前序 **`v1.1.0`**。下一产品里程碑：**WorkPanel V1.3.0（工作流）**（设计审稿中）。

## 怎么用（Agent / 人类）

1. 动手前：确认改动属于哪条**轨道**、落在哪个**阶段**（已交付 / 进行中 / 下一站）。
2. 不在流水线上的需求：先讨论是否立项，写入「下一站」或「暂缓」，再实现。
3. 发版：灰度 `:8081` → docs → commit →（管理员批）promote `:8080`。嵌套调用 ≤3。
4. 版本号说明：历史 epitaph 用过 `v1.2`…`v1.7` 与 `BaseV1.0.0`；`package.json` 仍为 `0.1.0`（未跟 semver 对齐）。**产品里程碑以本文「流水线阶段」为准**，不以 npm version 为准。

---

## 流水线总览（左→右）

```text
[地基 v0.x] → [协作内核 v1.2–v1.7] → [Base V1.0.0 生产基线]
                                              │
                    ┌─────────────────────────┼─────────────────────────┐
                    ▼                         ▼                         ▼
            [轨道 A 工作群]            [轨道 B 发布与稳态]          [轨道 C 扩展 Live]
                    │                         │                         │
                    ▼                         ▼                         ▼
            [轨道 D 聊天群] ← 方向已议，Phase1 待拍板
                    │
                    ▼
            [轨道 E 质量与测试债务]
```

---

## 已交付流水线（按时间 / 里程碑）

### 阶段 0 — 地基（v0.1–v0.4）

| 里程碑 | 能力摘要 | 交接 |
|---|---|---|
| v0.1 MVP | 群 CRUD、@ 触发、调度排队/取消/重试、mock / Codex / Claude | `docs/roadmap.md` 早期条目 |
| v0.2 | OpenCode / Cursor 适配器、AGENTS、smoke | 同上 |
| v0.3–v0.4 | OCR、Windows 路径、Tauri+Web 双模等 | epitaph archive v0.4 |

### 阶段 1 — 协作内核（v1.0–v1.7，Base 前）

| 里程碑 | 能力摘要 | 交接 |
|---|---|---|
| v1.0–v1.1 | PM：Roadmap / Feature / Task API + 前端面板 | `epitaph/2026-07-19-v1.0-pm-frontend.md` |
| v1.2 | Experience / Logs 面板；Web 服务可启动 | `epitaph/2026-08-01-v1.2-experience-logs-startup.md` |
| v1.3 | **生产/灰度双槽位**；freeze / deploy-canary / promote；数据隔离 | `epitaph/2026-08-01-v1.3-prod-canary.md` |
| v1.4 | 流式 parts；**同 Agent 串行**；Cursor session resume | `epitaph/2026-08-01-v1.4-streaming-session.md` |
| v1.5 | 测试策略 + **canary 前门禁** `test:gate` | `epitaph/2026-08-03-v1.5-test-gate.md` |
| v1.6 | 服务端路径/mkdir；群公告；项目工作流视图；Ops API | `epitaph/2026-08-03-v1.6-workflow-pm.md` |
| v1.7 | run phases；chatbot；workspace 沙箱；keep-alive；内存门禁约束 | `epitaph/2026-08-03-v1.7-chatbot-phases.md` |

### 阶段 2 — Base V1.0.0（生产基线，2026-08-04）

| 项 | 状态 |
|---|---|
| 灰度冒烟 + **promote 生产**（bin+dist，不碰 prod DB） | ✅ |
| 验收说明 | `docs/superpowers/specs/2026-08-04-workpanel-base-v1.0.0-acceptance.md` |
| 验证记录 | `epitaph/2026-08-04-v1.0.0-base-release-verify.md` |

**含义**：Base 之后，默认「生产可用协作面板」；其后改动按轨道增量，经灰度再晋升。

### 阶段 3 — Base 之后 → **v1.1.0**（已打 tag / 已 promote 生产，2026-08-05）

> 下列随 canary 验证后已晋升生产（`prod` RELEASE `promotedAt` ≈ `2026-08-05T08:28:53Z`）。后续增量仍须灰度 → 批准 → promote；勿中断 stop→start。

| 批次 | 能力摘要 | 交接 / 计划 |
|---|---|---|
| 文档与路径 | API 索引、mkdir 说明纠偏 | `epitaph/2026-08-05-docs-mkdir-api-index.md` |
| 成员 | 加入已有登录用户 | `epitaph/2026-08-05-link-existing-user-member.md` |
| PanelLive | Extension Host；同源代理；A2A live.*（禁 PCM）；Live 页签；Live 短回复注入（<50 字） | `epitaph/2026-08-05-panellive-short-reply.md` 等；roadmap v0.5 |
| 发版硬化 P1–P4 | 种子群 `is_system`；releasing 60s；心跳；metrics 20s | `epitaph/2026-08-05-release-hardening.md` |
| UX | 重连横幅前 30s 静默 | `releasingState` + checklist |
| 调度可见性 | 成员栏 **执行中 · 排队 N** + 展开取消 | `epitaph/2026-08-05-member-queue-visibility.md` |
| 聊天 D0 | 默认响应者；chatbot 窗口 12 + 时间戳 + 滚动摘要 | `epitaph/2026-08-05-chat-default-responder-context.md` |
| 未读/在线 | 左侧未读角标+排序+进群清零；用户 WS 在线绿点 | `docs/superpowers/specs/2026-08-05-presence-unread-design.md` |
| 邀请/删除 | 邀请链接入群（24h）；移除 vs 永久删除（purge/roster_hidden） | `epitaph/2026-08-05-invite-hard-delete.md` |
| 发布治理 | `approve-prod-release.sh`；promote trap；可选 prod watchdog；tag **v1.1.0** | scripts + GitHub |

### 阶段 4 — **v1.2.0**（生产基线，2026-08-06）

> 管理员指令「基线该版本」：将当时 canary（含豆包语音 UX）promote 生产。`prod` RELEASE 以 promote 时刻为准。

| 批次 | 能力摘要 | 交接 / 契约 |
|---|---|---|
| Host 豆包语音 | 主聊天「按住说话」+ 气泡 ▶；`purpose=playback`；松手即发 | `epitaph/2026-08-05-doubao-voice-ux-host.md`；Extend contract |
| 代理修复 | PanelLive 同源代理**转发 query**（`?format=json`） | `extensions::with_proxy_query` |
| Live 一致 | 会话态经代理；聊天同步进 Live UI | `epitaph/2026-08-05-live-session-chat-parity.md` |
| 工作区边界 | Live Extend ↔ Host 仓/群拍板（文档） | `specs/2026-08-05-workspace-boundary-live-host.md` |
| 发布 | promote 已完成；**git tag `v1.2.0` 待补**；不碰 prod DB | 本阶段 |

### 阶段 5 — **WorkPanel V1.3.0（工作流）**（实现中 → 灰度，2026-08-06）

> 与历史 epitaph「v1.3 双槽位」不同名不同义。全量设计：[`specs/2026-08-06-workpanel-v1.3.0-workflow-era-design.md`](superpowers/specs/2026-08-06-workpanel-v1.3.0-workflow-era-design.md)。

| 切片 | 内容 | 状态 |
|---|---|---|
| S1 | 版本页签 + Tag 时间线；去掉顶栏「项目」 | ✅ 代码；版本页标明 Git 工作区 + 多群共享告警 |
| S2 | 新建/导入版本；Roadmap What/Who/How；虚拟 Tag | ✅ 代码 |
| S3 | Ask 模式 + 头像 Ask 徽标；默认 Waves 确认；ask_gate | ✅ 代码（slash 补全后续） |
| S4 | Wave ▶/⏸/推进阶段；Roadmap 播放；awaiting_release 标记发布 | ✅ 代码（无独立 Codex loop 引擎，走管理员 kickoff） |
| **EH** | **通用扩展宿主**（消灭 panellive 硬编码；AIHotel 页签） | **认领**；见 [`specs/2026-08-06-extension-host-v130-claim.md`](superpowers/specs/2026-08-06-extension-host-v130-claim.md)；S0/S1 后端已开工 |
| **Wiki** | 跨 Agent 记忆：Wiki `retrieve` + 调度注入【全局知识·Wiki】 | W0/W1 代码；设计 [`WorkPanelWiki/.../2026-08-08-cross-agent-memory-compliance-design.md`](/AI/WorkPanelWiki/docs/superpowers/specs/2026-08-08-cross-agent-memory-compliance-design.md) |

---

## 锁定的产品方向（勿漂移）

| 轨道 | 定位 | 做 | 不做（本阶段） |
|---|---|---|---|
| **A 工作群** | 绑定 workspace 的 Agent 协作（主产品） | @ 调度、串行、A2A、公告、**版本/Wave 工作流**、经验/记忆 | 把工作群做成纯社交 IM |
| **B 发布与稳态** | 双槽位自迭代不断供 | 门禁、灰度公告、探活/心跳/指标、promote 审批 | Agent 擅自 promote；打断 stop→start |
| **C 扩展宿主** | manifest 动态 Extend（含 PanelLive / AIHotel） | `LINLIS_EXTENSION_ROOTS`、通用反代/页签/A2A | 平台写扩展业务；为每扩展抄 proxy_* |
| **D 聊天群** | `groupKind=chat`，无业务 workspace；轻对话 | Phase 1：体验地基（见下） | 未拍板前不做多平台中枢大工程 |
| **E 质量** | 门禁绿 + 策略文档同步 | 纯函数/调度单测；补缺口见 testing-strategy | 用真 CLI smoke 替代门禁 |
| **F 工作流 V1.3.0** | Git Tag 版本 + Ask + Wave | 见阶段 5 设计文 | 一次做完 S1–S4；自动无审批 promote |

### 轨道 D — 聊天群（已对齐的方向，待拍板开工）

来源：群聊讨论 + OpenClaw 分析 + Cursor 附议（2026-08-05）。

| 阶段 | 内容 | 状态 |
|---|---|---|
| D0 | 默认响应者可设；chatbot 窗口默认 12 + **滚动摘要**（超窗折叠后累加；非向量 RAG） | ✅ **v1.1.0 / 生产** |
| D1 | 聊天群类型体验：与工作群 UI 区分；表情/富媒体；主题按**用户**隔离 | **待拍板 Feature** |
| D2 | ChatBot 保持快响应；知识能力用可 @ Agent，不塞进 chatbot 本体 | 方向锁定；与 D0 一致 |
| D3 | 多平台映射（建议 QQ MVP → 防回环设计） | **暂缓**；仅调研，不进当前迭代 |

---

## 下一站（建议顺序，避免并行发散）

1. **V1.3.0 扩展宿主 EH S2–S5**：通用 iframe 页签 + 设置多开关 + 纯净度门禁（目标 **2026-08-09**）。
2. **V1.3.0 工作流灰度验收** →（批）promote。
3. **补打 git tag `v1.2.0`**。
4. **聊天群 D1（若拍板）** / 质量 E / D3：不与宿主收尾抢并发。
5. **稳态债（B）**：每次增量仍灰度 → 批准 promote；勿中断 stop→start。

---

## 变更纪律

| 规则 | 说明 |
|---|---|
| 先占位再改码 | 新能力写入本文件「下一站」或某轨道阶段 |
| 一文一事 | 大改动配 epitaph；本文只更新一行流水线状态 |
| 旧 `docs/roadmap.md` | 保留早期 v0.x 勾选；**新方向以本文为准**，roadmap 顶部指向本文 |
| 禁止无主改动 | 无轨道归属的「顺手重构/改交互」需显式拒绝或补立项 |

---

## 相关索引

| 文档 | 用途 |
|---|---|
| [`docs/epitaph/README.md`](epitaph/README.md) | 会话交接 |
| [`docs/testing-strategy.md`](testing-strategy.md) | 测试金字塔与缺口 |
| [`docs/release-checklist.md`](release-checklist.md) | 发版检查 |
| [`docs/panellive-platform-requirements.md`](panellive-platform-requirements.md) | Live 契约 |
| [`docs/roadmap.md`](roadmap.md) | 历史勾选 + 指向本文 |
| [`docs/superpowers/specs/2026-08-06-workpanel-v1.3.0-workflow-era-design.md`](superpowers/specs/2026-08-06-workpanel-v1.3.0-workflow-era-design.md) | V1.3.0 工作流全量设计 |
