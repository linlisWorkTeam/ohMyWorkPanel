---
date: 2026-08-16
topic: dsh-self-bootstrap-runtime
status: draft
---

# 结合 DeepSeek Harness 自举能力：总体规划（Self-Bootstrap via DSH Runtime）

> 本文是「把 DSH 的自举能力接入 ohMyWorkPanel」的**设计主文档**。
> 配套：群聊治理层设计 `2026-08-16-group-chat-governance-plane.md`；执行计划 `plans/2026-08-16-dsh-self-bootstrap-plan.md`。
> SSOT 占位已写入 `docs/version-pipeline.md`（新增轨道 G）。

## 背景 / 动机

ohMyWorkPanel 的自我更新是**群聊驱动的产品迭代**：群聊里讨论 → 版本/Wave 立项 → 灰度 :8081 → 审批 → promote :8080。它强在**人机协作与治理**（组织、审批、交接、审计），弱在**运行时自举原语**：会话不是机器可回放的分叉点、扩展是静态 manifest、回滚靠流程而非系统原语。

DeepSeek Harness（`dsh`）是 DeepSeek 的开源 agent harness：**一切皆插件**（Cordis）、profile/bundle 分层、append-only session log 可回放/分叉、插件注册是**可逆 effect**、fs/沙箱/subprocess/LLM 都是**可换能力 seam**、还有 subagent/ACP 跨进程委派与 headless CLI。它强在**运行时创造与可组合性**，弱在「人的治理」——没有群聊、审批门、发布流水线。

两者天然互补：**ohMyWorkPanel 提供「为什么做、谁批准、怎么保证不失控」，DSH 提供「能做出什么、能否留痕、能否回滚」。**

## 设计原则（先定边界）

1. **借鉴而非合并**：不把 DSH 内核寄生进 ohMyWorkPanel 运行时。DSH 是**外部可换执行运行时**（进程隔离），经 CLI / ACP / 同源代理 HTTP 桥接。
2. **双平面分工**：
   - **治理 / 协作平面（ohMyWorkPanel）**：群聊议事、`@` 调度、角色/管理员、版本/Wave、灰度→审批→promote。
   - **执行 / 创造平面（DSH）**：agent 循环、工具/LLM 适配器、可写 profile/插件、可回放会话、可逆注册、能力 seam。
3. **自举的动因在群聊，自举的实现进运行时**：群聊负责意图、辩论、拍板、审计叙事；DSH 负责可写回、可回放、可回滚。
4. **治理红线不可绕过**：agent 不得自我批准、不得伪造批准令牌、不得绕过灰度直改生产（延续 AGENTS.md 纪律）。
5. **锁版本 / 防漂移**：DSH 是开发者预览、破坏性变更频繁 → 固定容器/版本、当作可替换依赖，避免拖累面板自身稳定。
6. **两级自举执行者（每组一个、不可修改）**：
   - **普通群**：预制**极简模式**的 `bootstrap-dsh`（可跑 dsh 任务 + 基础干跑，**无自举写回权**）。
   - **ohMyWorkPanel 组**（种子/系统群，`is_system=1`）：预制 **`linlis-super-harness`**（超级 harness）——**只有它拥有“面板自己改自己”的完整自举能力**（提案→干跑→写回→灰度→上线）。两者均 `system_locked=1`、不可编辑/移除。

## 三层架构

```text
[意图/决策层]   群聊（议事 + 审批 + 叙事）           ← ohMyWorkPanel 负责
      │ 关联决策卡 / 提案
[结构化状态层]  版本/Wave · diff · 审批单 · 会话重放   ← ohMyWorkPanel 负责（可结构化视图）
      │ 驱动 / 观察
[执行/观测层]   DSH 运行时 · 扩展宿主 · 回滚 · 指标    ← dsh 负责（可写回/可回放/可回滚）
```

**结构性约束**：结构化存储是唯一事实源；群聊是围绕它的叙事入口。对话只放「结论 + 链接」，证据（会话重放、diff、日志）在结构化层。

## 借鉴 DSH 的四样东西（映射表）

| # | DSH 原语 | 现状（ohMyWorkPanel） | 借鉴后（ohMyWorkPanel） | 优先级 |
|---|---|---|---|---|
| ① | append-only session log 可回放/分叉 | epitaph 是「文档化石」交接 | run 级全量留痕 + 「重放一次会话再分叉」作为交接 | P1（性价比最高） |
| ② | 可逆 effect（unload 即撤销注册） | 回滚靠发版流程 | 运行时能力注册「diff → 干跑 → 应用 → 秒回滚」，与审批串接 | P2 |
| ③ | 能力 seam（fs/沙箱/subprocess/LLM 可换） | 工作区/路径/API key 写死 | 扩展宿主底层可插拔 seam，加能力不再硬编码 | P2 |
| ④ | subagent / ACP 跨进程委派 | 本地 CLI、群内串行 | 子任务可委托到另一进程/另一台机器/另一产品 | P4（远期） |

## 自举闭环（目标形态）

```text
ohMyWorkPanel 组：管理员/成员提出「给面板加能力/改规则」并 @linlis-super-harness
  ↓ 立项（版本/Wave）
linlis-super-harness（唯一完整自举引擎）以可回放会话执行：写 profile/插件/manifest → 干跑验证（test:gate / dry-run）
  ↓ 灰度 :8081（扩展宿主热载 DSH 写回的能力，session 可审计）
群聊/审批门：管理员批准（不能伪造令牌、不能绕过）
  ↓ promote :8080（运行时可回滚：plugin unload = 撤销）
完成一次被自身 agent 改写的升级 —— 全程留痕、可审计、可回滚
```

## 自举执行者：两级不可修改的 DSH 预制 Agent（bootstrap-dsh / linlis-super-harness）

**一句话定位**：平台自举引导器，**两级部署、每组一个、不可修改**——
- **普通群**：极简 `bootstrap-dsh`（轻量执行，不自举写回）。
- **ohMyWorkPanel 组**：`linlis-super-harness`（超级 harness，ohMyWorkPanel 自我更新唯一完整引擎）。

### 预制（随创建即自带，两级模板）
- **每个群**（普通项目群 / ohMyWorkPanel 组）在 seed / 创建时自动带一个自举 Agent；Web 与桌面同一套 seed 逻辑。
- **普通群极简模板**：`id=bootstrap-dsh-<group>`、`displayName=自举引导器（DSH·极简）`、`adapter=dsh`、固定 executablePath/workspace/roleDescription；能力 = 跑 dsh 任务 + 基础干跑。
- **ohMyWorkPanel 组模板**：`id=linlis-super-harness`、`displayName=linlis-super-harness`、`adapter=dsh`（super profile）；能力 = 完整自举（提案→干跑→写回 manifest/扩展→提交灰度）。
- 两者 `agent_profiles.system_locked=1` 表示平台锁定。

### 不可修改（强制点）
- **前端**：成员行**只读**——无“检测/设管理/改模型/改工作区/移除/删除”入口（保留头像/队列/状态展示）。
- **后端**：所有 mutation 命令对 `system_locked` 成员一律拒绝（`remove_member` / `set_admin` / `update_member_model_cmd` / `update_member_workspace_cmd` 等）。
- **可执行**：它仍然能跑任务（这是它的职责），只是**配置不可改、身份不可换**。

### 唯一自举通道（分两级权限）
- **完整自举写回权只属于 ohMyWorkPanel 组的 `linlis-super-harness`**：写回 manifest/扩展、提交灰度申请、驱动“面板自己改自己”。
- **普通群的极简 `bootstrap-dsh` 无自举写回权**：只做组内 dsh 执行 + 基础干跑，不碰面板本体。
- 其余普通 Agent 一律**无自举写入权限**——杜绝“任意 agent 顺手把面板改了”。

### 仍受人类治理
- **linlis-super-harness 负责“执行”完整自举**（普通群极简 bootstrap-dsh 只做组内轻量执行），但都**不能自我批准**：promote 仍需 root/人类审批（沿用不可伪造批准令牌的纪律）。
- 它把“提案 + 干跑结果 + 决策卡”交给群聊审批，审批通过后它才执行写回/上线。

### 鲁棒性
- 两级 bootstrap（极简 bootstrap-dsh / ohMyWorkPanel 组 linlis-super-harness）都依赖 dsh 运行时（headless / ACP）；需健康检查与就绪态展示。
- **dsh 缺失/退化时：自举流程 fail-closed（暂停，不半改面板）；日常群聊 fail-open（不因自举器失效而影响正常协作）。**


## 分阶段路线图

| 阶段 | 内容 | 借鉴点 | 状态 |
|---|---|---|---|
| **P0** | `dsh` headless CLI 适配器 + 「跳转 DSH Web」嵌入 | 执行桥 | ✅ 已交付 |
| **P1** | ACP 长驻适配器 + session 事件回灌 ohMyWorkPanel 消息/run | 可回放会话 ① | 待立项 |
| **P2** | 预制**两级模板**：普通群极简 `bootstrap-dsh-<group>` + ohMyWorkPanel 组 `linlis-super-harness`（均 `system_locked` 不可改 + 分级权限）+ 扩展宿主能力化（可逆注册） | 可逆 effect ② + 能力 seam ③ | 待立项 |
| **P3** | 自举全闭环：ohMyWorkPanel 组群聊 `@linlis-super-harness` 发起「面板自改」→ 版本/Wave → 灰度（热载 dsh 能力）→ 审批 promote → 可回滚 | 全闭环 | 待立项 |
| **P4** | subagent/ACP 跨机、跨产品委派 | 分布式委派 ④ | 远期 |

> P1 之前必须先跑通「群聊治理层设计」（`2026-08-16-group-chat-governance-plane.md`），否则自举闭环缺决策与审批一体面。

## 治理与护栏

1. 审批是群聊内的一等动作：决策卡关联「同意/驳回」与理由。
2. 自举类改动必须附「可回放会话 / 干跑结果」才能进群聊审批；禁止只丢一句「给我权限」。
3. 灰度期间禁止 `fuser -k` 生产端口 / 改写生产 unit / 伪造批准令牌（沿用现有双槽位纪律）。
4. DSH 版本固定：锁容器/依赖版本，升级另立小版本、走同一灰度→审批→promote。
5. 频率纪律：版本/Wave 本身即吞吐闸门；默认灰度、有限并发、冻结窗口、drain。
6. **两级 bootstrap 不可改性**：`system_locked=1` 平台锁定；前端只读、后端 mutation 拒绝；完整自举写回权只挂 ohMyWorkPanel 组 `linlis-super-harness`，普通群极简 `bootstrap-dsh` 无写回权，均不可被普通 Agent 复制。
7. **两级执行者不豁免审批**：linlis-super-harness / bootstrap-dsh 只执行、不批准；任何自举写回都必须先有群聊「决策卡 + 人类批准」。

## 风险与开放问题

- **复杂度叠加**：两套插件/配置体系并存。缓解：进程隔离 + P1 只做「回放/回灌」，不做能力热改。
- **DSH 破坏性变更**：锁版本；把「DSH 依赖」当作可替换件，不进入 ohMyWorkPanel 长期编译期契约。
- **会话回放的数据量**：run 级全量留痕需定分块与会话窗口（参考滚动摘要），避免存崩溃。
- 开放问题：
- **单点执行风险**：完整自举只走 ohMyWorkPanel 组的 linlis-super-harness，若 dsh 缺失/退化则自举暂停。缓解：健康检查 + fail-closed 只对自举、fail-open 对日常；super harness 可独立重装但身份固定。
  - P1 的 session 回灌是「落库镜像」还是「仅保留 replay 句柄」？
  - P2 的「能力热载」边界：只允许扩展宿主能力（工具/页签），还是允许改 ohMyWorkPanel 本体代码？
  - 普通群极简 bootstrap-dsh 是否允许“跨群提案”（把本群诉求上报给 ohMyWorkPanel 组）？
    - P4 的跨机委派是否需要新的凭据/信任模型？

## 验收 / 里程碑

- P0：`pnpm run test:gate` 绿；成员栏可建 `dsh` Agent；「跳转 DSH Web」可开内嵌 :3080。
- P1：一次 `@dsh` 任务结束后，群聊内可展开「会话重放」；epitaph 交接可一键分叉。
- P3：通过群聊发起一次「面板自带扩展」升级，全程「群聊决策卡 + 灰度 + 审批 + 可回滚」走通。
