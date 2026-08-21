---
date: 2026-08-05
topic: version-pipeline
status: active
---

# LinlisWorkPanel 版本流水线（发展方向锁定）

> **本文是产品/平台演进的单一事实源（SSOT）。**  
> 新功能先在本文件占位（轨道 + 阶段），再开 Feature / 写 epitaph；禁止「东改西改」无版本归属的散弹改动。  
> 细节仍以 `docs/epitaph/*` 与 `docs/superpowers/{specs,plans}/*` 为准；本文件只看**流水线与方向**。  
> 当前 Git 标签：**`v2.0.0`**（Cursor 环境包 + DSH 壳层时代）。前序 **`v1.3.0`**（工作流 `8e3869d`）、**`v1.2.0`**（`0750306`）、**`v1.1.0`**。`package.json` / Cargo 已对齐 **2.0.0**。

## 怎么用（Agent / 人类）

1. 动手前：确认改动属于哪条**轨道**、落在哪个**阶段**（已交付 / 进行中 / 下一站）。
2. 不在流水线上的需求：先讨论是否立项，写入「下一站」或「暂缓」，再实现。
3. 发版：灰度 `:8081` → docs → commit →（管理员批）promote `:8080`。嵌套调用 ≤3。
4. 版本号说明：历史 epitaph 用过平台小步 `v1.2`…`v1.7` 与 `BaseV1.0.0`。Git tag 与 `package.json` 对齐产品里程碑（`v1.1.0` / `v1.2.0` / `v1.3.0` / **`v2.0.0`**）。**勿把历史 epitaph「v1.3 双槽位」当成 tag `v1.3.0`。**

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
| 发布 | promote 已完成；**git tag `v1.2.0` = `0750306`**；不碰 prod DB | 本阶段 |

### 阶段 5 — **WorkPanel V1.3.0（工作流）**（已打 tag `v1.3.0` = `8e3869d`）

> 与历史 epitaph「v1.3 双槽位」不同名不同义。全量设计：[`specs/2026-08-06-workpanel-v1.3.0-workflow-era-design.md`](superpowers/specs/2026-08-06-workpanel-v1.3.0-workflow-era-design.md)。

| 切片 | 内容 | 状态 |
|---|---|---|
| S1 | 版本页签 + Tag 时间线；去掉顶栏「项目」 | ✅ 代码；版本页标明 Git 工作区 + 多群共享告警 |
| S2 | 新建/导入版本；Roadmap What/Who/How；虚拟 Tag | ✅ 代码 |
| S3 | Ask 模式 + 头像 Ask 徽标；默认 Waves 确认；ask_gate | ✅ 代码（slash 补全后续） |
| S4 | Wave ▶/⏸/推进阶段；Roadmap 播放；awaiting_release 标记发布 | ✅ 代码（无独立 Codex loop 引擎，走管理员 kickoff） |
| **EH** | **通用扩展宿主**（消灭 panellive 硬编码；AIHotel 页签） | **认领**；见 [`specs/2026-08-06-extension-host-v130-claim.md`](superpowers/specs/2026-08-06-extension-host-v130-claim.md)；S0/S1 后端已开工 |
| **Wiki** | 跨 Agent 记忆：Wiki `retrieve` + 调度注入【全局知识·Wiki】 | W0/W1 代码；设计 [`WorkPanelWiki/.../2026-08-08-cross-agent-memory-compliance-design.md`](/AI/WorkPanelWiki/docs/superpowers/specs/2026-08-08-cross-agent-memory-compliance-design.md) |

### 阶段 5.1 — 1.3.0+ 增量补丁（HEAD 上，未另打小版本）

| 切片 | 内容 | 状态 |
|---|---|---|
| P1 | 平滑发版 Drain + 重启重入队；Cursor 4.6 模型目录；群公告/工作区设置入口恢复 | ✅（前序补丁） |
| **I1** | **Agent 配置一键导入**：顶部「Agent 配置」页（仅管理员）——服务器导出配置包 → 本地一键导入（写 `~/.codex`/`~/.claude`/`~/.cursor`/通用 `files`，同步 agent_profiles，持久化+启动自动重放）；缺失 CLI 自动安装（best-effort）；环境自检；release 槽位自带 codex shim 脚本（开箱即用，新机无需重新 vibecoding） | ✅ 代码（spec：`specs/2026-08-18-agent-config-one-click-import.md`；本机端到端验证；待 ECS 灰度） |

### 阶段 6 — **WorkPanel V2.0.0**（Cursor 环境包 + DSH 壳层）

> 相对 v1.3.0 的 breaking UX（三栏壳跟 `ui-demo.html`）+ 开箱配置包。**不**把 cursor-agent 二进制或登录态打进 Git。

| 切片 | 内容 | 状态 |
|---|---|---|
| 壳层 | DSH 设计语言 P0–P2 + ui-demo 对齐；气泡操作常驻、去掉无下游 👍👎 | ✅ 代码 |
| 开箱 | Agent 配置导入/导出/自检/CLI 安装（阶段 5.1 I1） | ✅ 代码 |
| Cursor 包 | [`docs/releases/v2.0.0/`](releases/v2.0.0/) 脱敏 bundle + `scripts/pack-cursor-agent.sh` | ✅ 本阶段 |
| 发布 | git tag `v2.0.0`；灰度后需人批准才能 promote | 本阶段 |

---

## 锁定的产品方向（勿漂移）

| 轨道 | 定位 | 做 | 不做（本阶段） |
|---|---|---|---|
| **A 工作群** | 绑定 workspace 的 Agent 协作（主产品） | @ 调度、串行、A2A、公告、**版本/Wave 工作流**、经验/记忆、**交接运行时注入**、**CLI 适配器 Manifest（内部多 CLI）** | 把工作群做成纯社交 IM；把 CLI 插件塞进 IM connector / Extend 页签 |
| **B 发布与稳态** | 双槽位自迭代不断供 | 门禁、灰度公告、探活/心跳/指标、promote 审批 | Agent 擅自 promote；打断 stop→start |
| **C 扩展宿主** | manifest 动态 Extend（含 PanelLive / AIHotel） | `LINLIS_EXTENSION_ROOTS`、通用反代/页签/A2A | 平台写扩展业务；为每扩展抄 proxy_* |
| **D 聊天群** | `groupKind=chat`，无业务 workspace；轻对话 | Phase 1：体验地基（见下） | 未拍板前不做多平台中枢大工程 |
| **E 质量** | 门禁绿 + 策略文档同步 | 纯函数/调度单测；补缺口见 testing-strategy | 用真 CLI smoke 替代门禁 |
| **F 工作流 V1.3.0** | Git Tag 版本 + Ask + Wave | 见阶段 5 设计文 | 一次做完 S1–S4；自动无审批 promote |
| **G 自举运行时** | 借 DSH 自举能力：会话可回放/分叉、能力可热载/可回滚、subagent 跨进程委派；**自举只由预制不可改的两级自举 Agent 执行（WorkPanel 组 `linlis-super-harness` 完整 / 普通群极简 `bootstrap-dsh`）** | 群聊治理层（决策卡/审批）先行；两级 `system_locked` 不可修改；完整自举写回权只挂 `linlis-super-harness`；DSH 进程隔离 + 锁版本；自举动作必须经群聊人类批准 | 把 dsh 内核寄生进 WorkPanel；agent 自我批准/绕过灰度；普通 Agent 或极简 bootstrap-dsh 拥有面板自举写回权 |

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

1. **交接运行时桥（TOP1 / 轨道 A）**：Context Seams — epitaph 摘要注入 + Logs 记账（本切片）；全文 epitaph / 新事件表 / Wave 闭环不做。
2. **V1.3.0 扩展宿主 EH**：S0–S5 代码已齐；剩余灰度回归，不与本切片抢并发。
3. **V1.3.0 工作流已在生产**；后续增量仍灰度 →（批）promote。Wave 闭环为 TOP2，本切片之后再立项。
4. **Git tag**：`v2.0.0` 已打（2026-08-21）；下一小版本待立项后再打。
5. **聊天群 D1（若拍板）** / 质量 E Phase 2 / D3：不与宿主收尾抢并发。
6. **稳态债（B）**：每次增量仍灰度 → 批准 promote；勿中断 stop→start。
7. **轨道 G 自举运行时（新占位）**：设计 Spec `2026-08-16-dsh-self-bootstrap-runtime.md` + 群聊治理层 `2026-08-16-group-chat-governance-plane.md`；P0（dsh headless 适配器 + DSH Web 嵌入）✅；P1 ACP 会话回放待立项（先立群聊治理层）。**小组件定位**：见 `specs/2026-08-16-widget-capability-placement.md`（widget=页签/能力，不单独建群；仅治理型项目群例外）。
8. **Agent 配置一键导入（I1，轨道 A / 发布打包）**：代码已落地（阶段 5.1）；下一步 ECS canary `:8081` 部署 + 群公告 →（批）promote 生产；顺带验证「服务器导出 → 新机一键导入」端到端与前端壳冒烟（`release-checklist.md §F`）。
9. **CLI 适配器 Manifest（轨道 A）**：SSOT accepted — `specs/2026-08-21-cli-adapter-manifest.md`。**P0 已灰度 `:8081`**（查表 + `GET /api/adapters`）；P0.1+ 未开工。

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
| [`docs/superpowers/specs/2026-08-16-dsh-self-bootstrap-runtime.md`](superpowers/specs/2026-08-16-dsh-self-bootstrap-runtime.md) | 轨道 G：DSH 自举接入总设计（会两级不可改自举 Agent） |
| [`docs/superpowers/specs/2026-08-21-cli-adapter-manifest.md`](superpowers/specs/2026-08-21-cli-adapter-manifest.md) | 轨道 A：CLI 适配器 Manifest（**accepted** SSOT） |
| [`docs/superpowers/specs/2026-08-16-dsh-ui-language-workpanel.md`](superpowers/specs/2026-08-16-dsh-ui-language-workpanel.md) | 借鉴 DSH UI 设计语言（三栏：工作区=群聊，右栏=Agent） |
| [`docs/superpowers/specs/2026-08-16-widget-capability-placement.md`](superpowers/specs/2026-08-16-widget-capability-placement.md) | 小组件形态判定与收敛路线（widget=页签/能力，不单独建群） |
