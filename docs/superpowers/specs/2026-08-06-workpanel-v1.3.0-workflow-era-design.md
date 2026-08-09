---
date: 2026-08-06
topic: workpanel-v1.3.0-workflow-era
status: approved-implementing
decider: user 2026-08-06「设计通过开工」
choice: C — full design; implement S1→S4
---

# 设计：WorkPanel V1.3.0「工作流时代」

## 0. 命名与范围

| 名称 | 含义 |
|---|---|
| **WorkPanel V1.3.0（工作流）** | 本产品里程碑：版本页 + Roadmap/Wave/Ask |
| 历史平台 `epitaph v1.3` | 仅指生产/灰度双槽位，**不是**本里程碑 |

**目标**：工作群用 Git Tag 管理产品版本；版本页与聊天平级；管理员 Agent 经 Ask → Wave 敏捷迭代编排执行；简易迭代在 Tag 时汇总。

**同版本纳入（纯平台）**：**通用扩展宿主**——消灭 panellive 硬编码，manifest 动态发现/反代/页签；PanelLive 行为零回归；不为 AIHotel 抄专用 proxy。见 [`2026-08-06-extension-host-v130-claim.md`](./2026-08-06-extension-host-v130-claim.md)。

**非目标（V1.3.0）**：聊天群 D1；多平台 IM；平台内嵌 PCM；自动无人工 promote 生产；替换 Codex/Cursor CLI 本体；AIHotel 剧本/NPC 业务（留扩展仓）。

**实现策略（已选 C）**：本文为**全量设计**；代码按 **S1→S4** 分批灰度交付，每批独立门禁/docs/commit。

---

## 1. 现状与迁移

| 现有 | 处置 |
|---|---|
| 顶栏「项目」→ `ProjectWorkflowView`（工作流看板、Roadmap 进度、Ops、公告等） | **V1.3.0 起**：工作群顶栏用「**版本**」替代「项目」；去掉看板与 Roadmap 进度 UI |
| `roadmap_items` / `features` / `feature_tasks` / `roadmap_orchestrations` | S1 只读保留数据；S2+ 新模型并行；S4 完成后旧编排 UI 下线，数据可导出只读归档 |
| 群公告 / workspace / 成员面板 | 保留；公告仍可从成员侧或设置进入（不塞进版本页主路径） |
| Live 页签 | 不变，仍与聊天平级 |

仅 `groupKind=project`（工作群）显示版本页；聊天群无版本页。

---

## 2. 信息架构（UI）

顶栏（工作群）：**聊天 | 版本 |（Live 若启用）**

### 2.1 版本页（Langraph 式、更简洁）

- 纵向时间线：每个节点 = 一个 **Version**（优先对齐 Git Tag；见 §3）。
- **默认**：最新版本卡片**展开**；历史版本**折叠**（点标题展开）。
- 展开区：版本元数据、Roadmap（What/Who/How）、Wave 列表（状态 + ▶/⏸）、简易迭代摘要（Tag 后写入）。
- 空/新项目 CTA：
  - **新建版本** → 引导从 0 填 Roadmap（§4）
  - **导入版本** → 触发管理员 Agent 读 git log/tag，生成版本页内容（§3.2）
- `finish-last-round`：HEAD 落在最新 Tag 提交上且无进行中 Version → 显示「新建版本」为主 CTA（尚未启动新一轮）。

### 2.2 与聊天协作

- Ask / Wave 执行时用户主要在**聊天**看对话；版本页是控制面（播放/暂停、看阶段）。
- 管理员头像：**Ask** / **Wave:设计** 等状态角标（§5）。

---

## 3. 版本与 Git

### 3.1 Version 实体

```text
Version {
  id, groupId
  name              // 展示名，如 v1.3.0
  gitTag            // 可空；真实 tag 名
  gitSha            // 锚定 commit
  kind              // "tag" | "virtual" | "draft"
  status            // draft | planning | asking | wave_running | awaiting_release | released | archived
  roadmap           // What / Who / How（见 §4）
  requesterMemberId // 录入 Roadmap 的人（需求提出人）
  createdAt, releasedAt?
}
```

- **真实 Tag**：`git tag -l` + 指向的 sha。
- **虚拟 Tag**：无任何 tag、仅有 commits 时，导入/新建可生成 `kind=virtual`（如 `v0.0.0-dev`），进入「启动新版本」流程；正式发布时再打真实 Tag。
- 列表排序：按 tag 时间 / committer date 降序；draft 置顶展开。

### 3.2 导入版本

1. 用户点「导入」→ 平台 `POST .../versions/import`。
2. 调度**管理员 Agent**（无管理员则拒绝并提示）。
3. Agent 读 workspace：`git log`、`git tag`；产出结构化 Version + 初稿 Roadmap（可空 What）。
4. 无 tag → 创建 virtual Version，并进入新建版本后续步骤。

### 3.3 新建版本 / 发布

- **新建**：在 `finish-last-round` 或显式「启动新版本」后创建 `status=planning`，填 Roadmap。
- **发布流程**（Roadmap/全部 Wave 完成后）：`awaiting_release` → 用户完成灰度 SIT → **手动 @管理员** 发布（Agent 打 Tag / 更新 Version=`released`）。平台**不**自动 `promote-canary`；生产晋升仍走现有审批脚本。

---

## 4. Roadmap（规划视图）

启动新版本后必填（一句话也可）：

| 字段 | 说明 |
|---|---|
| **What** | 要做成什么 |
| **Who** | 服务谁 / 角色 |
| **How** | 怎么做（可选技术约束） |
| **oneLiner** | 可选；若填则 Ask 阶段再拆成 W/W/H |

持久化在 Version 上。填完 → 仅当存在管理员 Agent 时进入 **Ask**（§5）；否则阻断并提示添加管理员。

---

## 5. Ask 模式

### 5.1 进入 / 退出

| 进入 | 退出 |
|---|---|
| Roadmap 提交后自动 | 需求提出人**同意** Wave 拆分方案 |
| `/roadmap` 联想标题或版本页「继续」 | 用户/管理员显式取消 Ask |
| | 跨群 A2A 或**其他用户**高优打断（见 §8）后可暂停 Ask |

### 5.2 行为

- 管理员 Agent：`agentMode=ask`（或等价调度标记）。
- UI：头像框标明 **Ask**。
- 主动 `@需求提出人`（`requesterMemberId`）澄清 What/Who/How。
- **忽视**同群其他用户闲聊（不创建针对这些消息的 task_run），除非：
  - 消息来自需求提出人 / 群主；或
  - 显式 `@管理员`；或
  - 跨群 A2A；或
  - 系统级打断。
- 澄清完成后：提出 Wave1…WaveN 方案 → 需求提出人同意 → 落库 Waves，`status=wave_running` 或 `ready`（待首次 ▶）。

### 5.3 Slash

- `/roadmap [标题补全]`：打开/聚焦版本 Roadmap；可「继续 Ask」。
- `/wave [id|标题补全]`：触发指定 Wave 执行（等同版本页 ▶）。

---

## 6. Wave（敏捷迭代）

### 6.1 模型

```text
Wave {
  id, versionId, index, title, status
  // pending | running | paused | blocked | done | skipped
  phase           // 见 6.2
  phaseCursor     // 阶段内细步
  playState       // playing | paused
}
```

顺序执行 Wave1 → WaveN。版本级 ▶/⏸：从当前未完成 Wave 继续；Wave 级 ▶/⏸：只控该 Wave。

### 6.2 强制阶段（管理员分配任务、按序执行）

| 阶段 | 默认用户管控 | 细步（框架固定，Agent 遵守） |
|---|---|---|
| **1. 原始需求分配** | 低 | 解析 Roadmap 切片 → 生成任务卡 → 指派成员（Agent/人类）→ 群内公示 |
| **2. 需求澄清** | **可打断** | 列疑问 →（可选）@用户提问 → 等待答复或超时策略 → 更新验收要点 |
| **3. 需求设计** | 默认自动；技术瓶颈可打断 | 方案/接口/文件边界 → 风险列表 → 设计小结入库 |
| **4. 迭代开发** | 默认自动；不可实现可打断 | 拆任务 → 串行/并行（同 Agent 仍串行）→ 提交工作区改动 |
| **5. 测试灰度验收** | 默认自动；无法验收可打断 | 门禁/自测清单 → 灰度部署建议 → 记录证据；阻塞则 `blocked` |
| **6. 总结** | 默认自动 | 变更摘要 → 回写 Wave 总结 → 解锁下一 Wave |

有 Codex 时，阶段 4 默认走 **Codex loop**（平台已有 loop/适配器能力；Wave runner 配置 `adapterHint=codex` + loop）。

### 6.3 播放 / 暂停

- ▶：恢复 `playState=playing`，调度管理员继续当前 `phase/phaseCursor`。
- ⏸：`paused`；管理员记住状态（DB cursor），不丢上下文。
- 全 Roadmap 完成 → Version `awaiting_release`（§3.3）。

---

## 7. 简易迭代

- 定义：群聊中直接 @Agent 改代码、**未**走 Wave 编排的改动。
- 记账：可选轻量 `simple_iteration_events`（messageId/runId/摘要）；若无表，发布 Tag 时由管理员 Agent 根据 `git log` since last tag 总结。
- Tag 发布说明中固定一节：**简易迭代改动**。

---

## 8. 打断与在线策略

Agent 若需向用户提问，框架综合：

1. **用户在线**（已有 presence）→ 更倾向即时 @ 打断。
2. **阶段必要性**：澄清/不可实现/无法验收 → 高；设计/开发细节 → 低（可批注后继续）。
3. **Ask 锁定**：Ask 期间优先需求提出人；他人消息默认不打断 Ask。
4. **跨群 A2A**：可打断 Ask/Wave（记录原因，`paused` 或插入系统事件）。

具体阈值参数（冷却秒数等）S4 落地时用配置项，默认偏少打断。

---

## 9. 架构（推荐方案 A）

```text
[版本页 UI] ──REST──► [version / wave API]
                           │
                           ├─ git_inspect (workspace 只读)
                           ├─ version_store (SQLite)
                           └─ workflow_runtime
                                  ├─ ask_gate (scheduler 钩子)
                                  ├─ wave_runner (阶段状态机)
                                  └─ 复用 task_runs / adapters / loop
```

- **不做 B**：不硬塞旧 `roadmap_orchestrations` 作为唯一引擎。
- 旧 PM API：S1–S3 保持可用但 UI 入口移除；S4 标注 deprecated。

---

## 10. 分批交付（S1→S4）

| 切片 | 交付 | 验收要点 |
|---|---|---|
| **S1** | 顶栏「版本」；去掉旧看板/进度；Tag 时间线只读；展开最新 | 工作群可见；聊天群无；git 失败有降级文案 |
| **S2** | 新建/导入；Roadmap What/Who/How；virtual tag；`finish-last-round` CTA | 持久化；无管理员导入失败提示 |
| **S3** | Ask 模式 + 头像态；@提出人；闲聊忽略规则；同意后生成 Waves；`/roadmap` `/wave` | 单测覆盖 ask_gate |
| **S4** | 六阶段 runner；▶/⏸；Codex loop；打断策略；简易迭代总结；awaiting_release | 灰度跑通 1 条 Wave happy path |

每片：设计增量（若有）→ 实现 → `test:gate` → 灰度 → commit；全量完成后再批生产。

---

## 11. 测试设计（门禁相关）

| 层 | 覆盖 |
|---|---|
| 单测 | git tag 解析；virtual tag；ask_gate 过滤；wave 阶段迁移；slash 补全 |
| 集成 | version CRUD；import 拒绝无管理员 |
| 不做 | 真 Codex loop 进门禁（仍 smoke 尽力） |

---

## 12. 风险

- Ask「忽略闲聊」误伤：必须白名单 @ / 提出人 / A2A。
- Git 在无仓库/脏工作区：降级文案，不崩页。
- 与旧 Roadmap 数据双轨短暂并存，避免破坏生产历史 PM 数据。
- V1.3.0 命名与历史 epitaph v1.3 并存——文档一律写「WorkPanel V1.3.0（工作流）」。

---

## 13. 开放问题（审稿时可改）

1. 版本页是否保留「工作区路径 / 群公告」入口，还是完全挪到设置/成员面板？  
2. Wave 内多人并行：除「同 Agent 串行」外，是否允许不同 Agent 并行阶段 4？  
3. 正式 Tag 命名规范是否强制 `vMAJOR.MINOR.PATCH`？

---

## 14. 自检

- [x] 无 TBD 实现步骤冒充已决  
- [x] 边界与非目标明确  
- [x] 分批 S1–S4 可独立验收  
- [x] 与现有调度/双槽位/发布审批不冲突  
