# ohMyWorkPanel 产品与接口说明（UI/UX 重写交付书）

> 用途：把现有产品讲清楚，并把**所有对外契约**列全，交给外部设计/实现方（如 GPT）重写前台 UI/UX。
> 版本：对应 `codex/agent-playground-2026-10-22` 分支（含插件内核、Goal 自治、像素场景）。
> 原则：**只重画前台，不改契约**。所有接口、字段、鉴权、WebSocket 事件、Tauri 命令都视为不可破坏的既有契约。

---

## 1. 产品是什么

一句话：**一个本地优先（local-first）、可自托管的「多 Agent 群聊工作台」**。

- 群是协作单位：一个群 = 一个项目，有自己的工作区目录、成员（人 + Agent）、公告（项目级规则）、版本流水线、Goal。
- Agent 是群成员：每个 Agent 绑定一个本机 CLI 适配器（codex / claude-code / opencode / cursor / dsh / openclaw / mock），在群里被 `@` 时触发一次 run（一次任务执行），流式产出「思考过程 / 中间产物 / 最终回复」。
- 人是任务的发布者与审批者：人发消息、`@` Agent、审阅 run、批准 Goal 轮次；Agent 之间的协作由调度器完成（A2A、父子 run、深度限制）。
- 最终愿景是「Agent 游乐园」：Agent 之间社交、人只发布任务；前台提供像素场景，把成员投影成像素小人（工位 / 会议 / 候场分区，按 adapter 区分造型，每个群一个「房间」）。

## 2. 设计理念（重写 UI 时请保留这些「为什么」）

1. **群即工作台**
   所有业务都挂在群下：消息、run、版本、Roadmap、Goal、Marketing。UI 的心智模型应是「进入一个群 = 进入一个项目空间」，而不是「进入一堆全局功能页」。

2. **Agent 是同事，不是工具弹窗**
   Agent 有头像、角色描述、状态（待检测/就绪/不可用/链接中）、运行队列。前台应让「谁在做什么」一眼可见，而不是把所有 Agent 藏在设置里。

3. **人在环上（human-in-the-loop），不是环内**
   Goal 每轮可要求人工批准；run 可审阅（approved / rejected）；写权限收紧到群主/管理员。UI 要能清楚表达「谁在等谁批」。

4. **契约先行的插件内核**
   后端有一个 Cordis 语义的插件内核（服务注册表、依赖注入、可撤销副作用、五种事件分发、作用域 fork、清单 v2）。内置能力（extensions / adapters / workflow / marketing）都以插件服务形式提供，**调用方只走服务**。前台场景本身也是一个前端内核插件（`svc.scene`）。重写 UI 不需要理解内核，但不要绕过 `api` 层直接拼 HTTP——双端（桌面/网页）差异由 `api.ts` / `api-web.ts` 吸收。

5. **渐进增强、可回退**
   像素场景是「壳插件」：默认关闭、可随时退出，关闭时零行为差异；插件装载失败必须 fail-open（页面不白屏）。任何新 UI 也应遵循：某块能力坏掉时，主流程仍可用。

6. **本地优先 / 可自托管**
   数据在本地 SQLite（服务端），工作区是本机磁盘目录；没有云端账号体系。UI 文案不要暗示「云同步」「团队空间」。

7. **视觉语言：深色、克制、像素点缀**
   七套主题（atlas / cyberpunk / forge / industrial / minimal / moss / noir），全部通过语义 token（`--lp-*`）取色，禁止颜色字面量。像素场景是「点缀与叙事」，不是主界面。

## 3. 角色与核心循环

**角色**

| 角色 | 能做什么 |
|---|---|
| 平台管理员（root） | 建群、增删成员/设管理员、Ops、Agent 配置导入导出、模型刷新、归档群 |
| 群主（owner） | 管理本群成员、工作区、公告、版本、Goal 写操作 |
| 群管理员 Agent | 被指定为群的驱动 Agent：版本 Ask/Wave 的 kickoff、Goal 推进的实际执行者 |
| 普通成员（人/Agent） | 读群、发消息、`@` Agent；Goal 只读 |

**核心循环（最重要的三条横幅式的主循环）**

1. **协作循环**：发消息 → `@` Agent → run 进入队列 → 流式输出（thinking / artifact / final）→ 人审阅/投票 → 结果沉淀为消息。
2. **交付循环**：Roadmap → Feature/Task → 版本（Version）→ Wave 播放 → Ask 对齐 → 发布（Release，可打 Git tag）→ 版本板。
3. **自治循环**：建 Goal（目标 + 预算）→ 启动 → 推进一步（派发给管理员 Agent）→ 轮次判定（`[GOAL:MET]` / `[GOAL:PROGRESS]` / `[GOAL:FAILED]`）→ 证据账本 → 停止/完成；可开「自动推进」由后台每 10 秒扫描。

## 4. 功能地图

> 每节给：目标 / 界面元素 / 关键交互 / 数据来源 / 空态与异常，供 UI 设计者直接画页面与状态。

### 4.1 账号与入场

- **目标**：登录进入自己所在的群。
- **界面**：登录表单（用户名/密码）、注册、邀请落地页（`/api/invites/{token}` 预览 → 接受）。
- **交互**：登录成功拿 JWT，之后所有请求带 `Authorization: Bearer <token>`；WebSocket 用 `?token=`。
- **空态**：无群 → 引导建群（仅管理员）或联系管理员。
- **异常**：401 需要重新登录；403 表示有账号但无权限（例如非管理员建群）。

### 4.2 群与成员

- **目标**：管理项目空间的人与 Agent。
- **界面**：左侧群列表（未读红点、归档折叠）、群头部（名称/类型 project|chat/成员数）、右侧成员栏（成员列表、角色徽章、adapter、在线/待检测状态）。
- **交互**：建群（需要服务器上已存在的绝对路径作为工作区；聊天群无需工作区）、加成员（人/Agent，Agent 可选 adapter 与角色）、邀请链接、设管理员、移除/彻底清除成员、归档群、设置群公告（会注入所有 Agent prompt）。
- **数据**：`Group` / `Member`（见 §6.6）。
- **空态**：新群无消息 → 显示「`@` 某个成员开始协作」。
- **异常**：工作区不存在/跨盘符迁移 → 明确提示去群设置重选路径。

### 4.3 群聊协作（主界面）

- **目标**：人发任务，Agent 执行，过程可见、结果可审。
- **界面**：
  - 消息流：用户气泡 / AI 气泡、时间、发送者、`@` 提及、引用、状态。
  - 流式区块：每条 Agent 消息可展开「思考过程」「中间产物」两段（点击加载，不阻塞主消息）。
  - run 状态：排队 / 运行中 / 完成 / 失败 / 取消；心跳与阶段（phase）。
  - 右栏「队列」：当前群 run 列表与活动 run。
  - 消息操作：投票（赞/踩 + 计数）、审阅（通过/驳回）、重试、取消。
  - 输入区：`Enter` 发送、`Shift+Enter` 换行、`@` 唤起成员菜单、`/` 命令、草稿自动保存、图片/OCR 入口。
- **数据**：`Message`、`TaskRun`、`MessageFeedback`、`ChatEvent`（WS）。
- **空态**：无消息；无 Agent 时提示先加 Agent。
- **异常**：调度失败（`scheduler_error` 事件）、Agent 不可用（run 失败并带 errorMessage）、连接断开后重连（`ws_reconnected`，前端应拉一次全量）。

### 4.4 Agent 配置

- **目标**：让每个 Agent 可用（适配器、模型、凭据、工作区、保活）。
- **界面**：Agent 配置页（`mainView = "agent-config"`）、成员行右键菜单、检测按钮、模型下拉、API Key/URL 表单。
- **交互**：
  - 适配器选择（后端只接受受支持的 key：`mock|codex|claude-code|opencode|openclaw|cursor|dsh`；`chatbot-*` 为聊天机器人）。
  - 检测（`detect_agent`）→ 返回版本/路径/可用性。
  - 设置模型、API URL、API Key（只显示是否已设置，不回传明文）。
  - 设置 Agent 工作区（绝对路径，或**相对群工作区的相对路径**，如 `agents/a1`；必须落在群工作区内，越界会被拒绝）。
  - 保活（keepAlive）、预热状态（warmStatus）。
  - Agent 配置导入/导出/一键安装 CLI（管理员）。
- **空态**：未检测；CLI 未安装 → 给安装引导。
- **异常**：凭据缺失、地区限制、订阅过期等由后端错误文案透出（如 Codex 的 403）；UI 原样展示即可。

### 4.5 项目管理（Roadmap / Feature / Task / 编排）

- **目标**：把「要做什么」结构化，并能一键派给 Agent 执行。
- **界面**：版本页内的 Roadmap 视图（列表/看板）、Feature 与子任务、编排（orchestration）状态。
- **交互**：增删改 Roadmap 条目（标题/描述/状态/优先级/目标日期/排序）、增删改 Feature 与 FeatureTask、`start` 某条目 → 生成编排（可暂停/恢复/取消）。
- **数据**：`RoadmapItem`、`Feature`、`FeatureTask`、编排记录。
- **空态**：无条目 → 引导「先写一条 Roadmap」。

### 4.6 版本流水线（Version / Wave / Ask / Release）

- **目标**：把一次交付拆成版本与 Wave，按节奏推进并可发布。
- **界面**：群顶部 Wave 条（进度、当前 Wave、点击进版本页）、版本页（Git 快照、版本卡、Roadmap 编辑、Wave 时间线、Ask 状态）。
- **交互**：
  - 建版本：`kind=draft`、`status=planning`；可填 What / Who / How / 一句话 / 名称。
  - 更新 Roadmap（同上字段）。
  - 进入 Ask（需要群管理员 Agent）：群里自动发一条 `@管理员` 的对齐任务；可取消 Ask。
  - 批准 Wave（可从 Roadmap 自动生成默认 Wave）。
  - 播放 / 暂停 / 推进 Wave（播放会 kickoff 管理员 Agent）。
  - 播放 / 暂停整个版本；发布（可带 Git tag，`kind` 变 `tag`）。
- **数据**：`VersionBoard`、`ProjectVersion`、`Wave`、`GitSnapshot`。
- **空态**：无版本 → 「尚未建立 Wave，点此到版本页」。
- **异常**：非群主/管理员操作 → 403 文案；未设管理员 Agent 时 Ask/推进报错。

### 4.7 Marketing（自我营销内容）

- **目标**：基于项目证据生成对外内容（brief/渠道文案/导出）。
- **界面**：Marketing 面板（campaign 列表、详情、生成、修订、批准、导出 Markdown）。
- **交互**：创建 campaign（可指定 planner/writer Agent、来源模式 committed|include_uncommitted、base ref）→ 修订（给反馈重新生成）→ 批准 → 导出。
- **数据**：`ContentCampaign`、`CampaignExport`。
- **空态**：无 campaign。
- **异常**：未批准不能导出；缺 Agent/证据时报错。

### 4.8 Goal 自治

- **目标**：给群一个目标，让 Agent 自己一轮轮推进，预算兜底、过程可审计。
- **界面**：群顶部 Goal 条（目标、状态、轮次/预算、token、最近一轮摘要、操作按钮、「自动」勾选、「明细」账本）。
- **交互**：
  - 新建（目标文本 + 可选预算 + 是否每轮审批 + 是否自动推进）→ 启动 / 暂停 / 继续 / 批准 / 停止。
  - 推进一步：派发给管理员 Agent，阻塞等待一轮结束；期间显示「推进中…」并禁用按钮。
  - 明细：按轮展示 `#序号 判定 · token · 摘要`（达成绿/进展蓝/失败红）。
  - 状态机：`draft → active → paused / awaiting_approval → completed / aborted`（终态不可变）。
  - 停止原因：目标达成 / 轮次用尽 / 超时 / token 用尽 / 连续无进展 / 连续失败 / 人工停止。
- **数据**：`Goal`、`GoalRound`、`GoalBudget`。
- **空态**：无目标 → 输入框「给这个群一个目标，Agent 会自己推进」。
- **异常**：群未设管理员 Agent → 明确报错；预算触线 → 状态转 aborted 并显示原因。

### 4.9 像素游乐园（场景）

- **目标**：把群成员投影成像素小人，提供「游乐园」式空间感；纯前台、只读、可退出。
- **入口**：标题栏 `◧` 开关（`mainView = "scene"`），退出回聊天。
- **界面**：HUD（在场 / 工作 / 阻塞 / 完成）、Canvas 场景（顶部墙面、地板网格、房间色带）、三个功能区（工位区 / 会议区 / 候场区，虚线框 + 标签）、像素小人（头饰 + 名字旁品牌色块 + 状态动画：工作呼吸 / 阻塞闪烁 / 完成点 / 空闲轻晃）、选中高亮、候场区换行（20 人不越界）。
- **交互**：点小人 → 打开既有成员面板（不是新页面）；返回聊天；每群一个「房间」（按群 id 稳定派生种子，切换群释放场景作用域）。
- **数据**：成员 + run 只读投影（`SceneAgent`），前端纯函数布局。
- **空态**：无成员提示；插件缺失 → 显示「场景插件未装载，已回退到聊天视图」，不白屏。
- **异常**：不参与业务写入；任何场景错误都不得影响聊天。

### 4.10 扩展与 A2A

- **目标**：把外部能力（PanelLive、AIHotel 等）挂进平台，同源代理，避免 iframe 直连本机端口。
- **界面**：Extend 页签 / 扩展面板（iframe 指向平台同源 `/api/extensions/{ext_id}/{*path}` 代理）；扩展启用开关（按群）。
- **交互**：列出扩展、按群启用/停用、PanelLive 专用开关、事件上报（`/api/extensions/panellive/events`）、A2A dispatch（外部 Agent 通过 token 调用）。
- **数据**：扩展 manifest（id/name/runtime/contributes.skills）。
- **异常**：扩展未启用 → 409；代理不可达 → 502 类错误。

### 4.11 可观测与运维

- **目标**：排障与发布。
- **界面**：日志面板（级别、条数、清空）、经验库（保存/查询/删除）、指标（最新 metrics）、Ops 状态（发布状态、job、test gate、deploy canary、drain）。
- **交互**：筛选/计数/清空日志；保存经验到群；管理员触发门禁/灰度；查看 drain 状态。
- **数据**：`LogEntry`、经验记录、`OpsJobState`、`ReleaseStatus`。

### 4.12 设置与主题

- **目标**：运行时配置与外观。
- **界面**：运行时设置（心跳间隔、保活默认值等，按后端 `RuntimeSettings`）、主题选择（7 套）、`dsh` 视图入口。
- **交互**：保存设置（即时生效）、切主题（写 `data-theme` 到 `<html>`）。
- **异常**：保存失败原样提示。

## 5. 现有信息架构（供重写参考，不强制保留）

```
┌ 左控制轨：Logo / 群 / 设置 / 折叠
├ 侧栏：工作区 · 群列表（未读、归档） / 建群
├ 中栏：群头部（名称、类型、成员数、扩展、设置）
│        WAVE 条（版本进度） / GOAL 条（目标与推进）
│        主内容区：chat | versions | scene | dsh | agent-config
│        输入区（chat 视图）
└ 右栏：成员 / 队列 / 详情 / 设置（按标签切换）
```

- 视图模式：`chat`（默认）、`versions`、`scene`、`dsh`、`agent-config`。
- 群类型：`project`（有工作区、版本、Goal）与 `chat`（无工作区，多聊天机器人）。

## 6. 对外接口

### 6.1 通用约定

- **Base URL**：`http://<host>:<port>`（本地默认 `:8080`；灰度 `:8081`）。
- **鉴权**：除 `/api/auth/login`、`/api/auth/register`、`/api/health`、`/api/invites/{token}`（预览）外，全部需要 `Authorization: Bearer <JWT>`。
- **WebSocket**：`/ws?token=<JWT>`。
- **编码**：请求/响应均为 JSON，字段 camelCase（Rust 侧 serde rename）。
- **错误**：非 2xx 返回纯文本错误消息（中文）；常见码：`400` 参数/业务校验、`401` 未登录、`403` 无权限、`404` 不存在、`409` 冲突、`503` 插件内核未装载。
- **多方法**：`GET/POST` 表示同一路径两种方法。
- **管理接口**：`/api/ops/*`、`/api/agent-config/*`、`/api/agent-models/refresh`、建群、成员增删/设管理员、归档、扩展启停等，后端会校验平台管理员或群主；前端按 `403` 展示即可。

### 6.2 HTTP API 全表（109 条）

#### 认证 / 健康

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/auth/register` | 注册 |
| POST | `/api/auth/login` | 登录 → `{ token }` |
| GET | `/api/health` | 健康检查 `{ok,service}` |
| GET | `/api/auth/verify` | 校验 token |

#### 邀请

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/invites/{token}` | 邀请预览（公开） |
| POST | `/api/invites/{token}/accept` | 接受邀请入群 |

#### 群

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/groups` | 我的群列表 |
| POST | `/api/groups` | 建群（管理员） |
| GET | `/api/groups/{id}` | 群详情 |
| PUT | `/api/groups/{id}/read` | 标记已读 |
| GET/PUT | `/api/groups/{id}/announcement` | 读/写群公告 |
| PUT | `/api/groups/{id}/workspace` | 改群工作区 |
| PUT | `/api/groups/{id}/archive` | 归档/解档 |
| PUT | `/api/groups/{group_id}/admin` | 设管理员 Agent |
| GET/PUT | `/api/groups/{id}/extensions` | 列扩展 / 按群启停（PUT 带 `{ext_id}`） |
| PUT | `/api/groups/{id}/extensions/{ext_id}` | 启停某扩展 |
| PUT | `/api/groups/{id}/extensions/panellive` | PanelLive 开关 |

#### 成员

| 方法 | 路径 | 说明 |
|---|---|---|
| GET/POST | `/api/groups/{group_id}/members` | 成员列表 / 加成员 |
| DELETE | `/api/groups/{group_id}/members/{member_id}` | 移除成员 |
| DELETE | `/api/groups/{group_id}/members/{member_id}/purge` | 彻底删除 |
| PUT | `/api/members/{member_id}/workspace` | 设 Agent 工作区（绝对或相对群工作区） |
| PUT | `/api/members/{member_id}/model` | 设模型 |
| GET | `/api/users/joinable` | 可加入用户 |

#### 在线状态

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/presence` | 在线列表 |
| POST | `/api/presence/heartbeat` | 心跳 |

#### 消息 / run / 反馈

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/messages` | 发消息（`groupId, senderMemberId, content, mentionMemberIds[]`） |
| GET | `/api/groups/{group_id}/messages` | 历史消息（分页） |
| GET | `/api/groups/{group_id}/messages/{message_id}/parts/{channel}` | 取思考/中间产物分段 |
| POST | `/api/messages/{message_id}/vote` | 赞/踩 |
| GET | `/api/messages/{message_id}/feedback` | 反馈聚合 |
| GET | `/api/groups/{group_id}/runs` | run 列表 |
| GET | `/api/groups/{group_id}/runs/active` | 活动 run |
| POST | `/api/runs/{run_id}/cancel` | 取消 |
| POST | `/api/runs/{run_id}/retry` | 重试 |
| POST | `/api/runs/{run_id}/review` | 审阅（approved/rejected） |
| GET | `/api/runs/{run_id}/phases` | run 阶段 |

#### 设置 / 模型 / 适配器 / OCR

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/settings` | 运行时设置 |
| PUT | `/api/settings` | 保存设置 |
| GET | `/api/agent-models` | 模型目录 |
| POST | `/api/agent-models/refresh` | 刷新目录（管理员） |
| GET | `/api/adapters` | 适配器目录 |
| POST | `/api/ocr` | 图片 OCR（路径） |
| POST | `/api/ocr/base64` | 图片 OCR（base64） |
| GET | `/api/preset-roles` | 预设角色 |

#### Agent 配置

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/agent-config/status` | 配置状态 |
| POST | `/api/agent-config/export` | 导出配置 |
| POST | `/api/agent-config/import` | 导入配置 |
| POST | `/api/agent-config/install/{cli}` | 安装 CLI |

#### 文件系统

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/fs/list` | 列目录（路径选择器） |
| POST | `/api/fs/mkdir` | 新建目录 |

#### Roadmap / Feature / Task / 编排

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/groups/{group_id}/roadmap` | Roadmap 列表 |
| POST | `/api/roadmap-items` | 新建条目 |
| PUT | `/api/roadmap-items/{id}` | 更新条目 |
| DELETE | `/api/roadmap-items/{id}` | 删除条目 |
| POST | `/api/roadmap-items/{id}/start` | 启动编排 |
| GET | `/api/groups/{group_id}/roadmap-orchestrations` | 编排列表 |
| POST | `/api/roadmap-orchestrations/{id}/pause` | 暂停 |
| POST | `/api/roadmap-orchestrations/{id}/resume` | 恢复 |
| POST | `/api/roadmap-orchestrations/{id}/cancel` | 取消 |
| GET | `/api/groups/{group_id}/features` | Feature 列表 |
| POST | `/api/features` | 新建 Feature |
| PUT | `/api/features/{id}` | 更新 Feature |
| DELETE | `/api/features/{id}` | 删除 Feature |
| GET | `/api/features/{feature_id}/tasks` | 子任务列表 |
| POST | `/api/feature-tasks` | 新建子任务 |
| PUT | `/api/feature-tasks/{id}` | 更新子任务 |
| DELETE | `/api/feature-tasks/{id}` | 删除子任务 |
| GET | `/api/groups/{group_id}/roadmap-state` | 聚合状态 |

#### 版本 / Wave / Ask / Release

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/groups/{group_id}/version-board` | 版本板（Git 快照 + 版本 + Wave） |
| POST | `/api/project-versions` | 建版本 |
| PUT | `/api/project-versions/{version_id}/roadmap` | 更新 What/Who/How/一句话/名称 |
| POST | `/api/project-versions/{version_id}/ask` | 进入 Ask |
| POST | `/api/project-versions/{version_id}/ask/cancel` | 取消 Ask |
| POST | `/api/project-versions/{version_id}/waves/approve` | 批准 Wave |
| POST | `/api/project-versions/{version_id}/play` | 播放版本 |
| POST | `/api/project-versions/{version_id}/pause` | 暂停版本 |
| POST | `/api/project-versions/{version_id}/release` | 发布（可带 Git tag） |
| POST | `/api/waves/{wave_id}/play` | 播放 Wave |
| POST | `/api/waves/{wave_id}/pause` | 暂停 Wave |
| POST | `/api/waves/{wave_id}/advance` | 推进 Wave |

#### Marketing

| 方法 | 路径 | 说明 |
|---|---|---|
| GET/POST | `/api/groups/{group_id}/marketing/campaigns` | 列表 / 创建 |
| GET | `/api/marketing/campaigns/{campaign_id}` | 详情 |
| POST | `/api/marketing/campaigns/{campaign_id}/revise` | 要求修改 |
| POST | `/api/marketing/campaigns/{campaign_id}/approve` | 批准 |
| GET | `/api/marketing/campaigns/{campaign_id}/export` | 导出 Markdown |

#### Goal

| 方法 | 路径 | 说明 |
|---|---|---|
| GET/POST | `/api/groups/{group_id}/goals` | 目标列表 / 新建 |
| GET | `/api/goals/{goal_id}` | 目标详情 |
| POST | `/api/goals/{goal_id}/start` | 启动 |
| POST | `/api/goals/{goal_id}/pause` | 暂停 |
| POST | `/api/goals/{goal_id}/approve` | 批准本轮 |
| POST | `/api/goals/{goal_id}/abort` | 停止 |
| POST | `/api/goals/{goal_id}/advance` | 推进一步（阻塞直至一轮结束） |
| POST | `/api/goals/{goal_id}/auto-advance` | 开关自动推进（`{enabled}`） |

#### 日志 / 经验 / 指标 / Ops

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/logs` | 日志查询 |
| GET | `/api/logs/count` | 计数 |
| DELETE | `/api/logs` | 清空 |
| POST | `/api/experiences` | 保存经验 |
| GET | `/api/experiences` | 查询经验 |
| DELETE | `/api/experiences/{id}` | 删除经验 |
| GET | `/api/metrics/latest` | 最新指标 |
| GET | `/api/ops/release-status` | 发布状态 |
| GET | `/api/ops/job` | Ops job 状态 |
| POST | `/api/ops/test-gate` | 触发门禁 |
| POST | `/api/ops/deploy-canary` | 部署灰度 |
| GET/PUT | `/api/ops/drain` | 发布 drain 状态/开关 |

#### 扩展 / A2A / WebSocket

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/extensions/panellive/events` | PanelLive 事件上报 |
| ANY | `/api/extensions/{ext_id}/{*path}` | 扩展同源反代（GET/POST/…） |
| POST | `/api/a2a/dispatch` | 外部 Agent A2A 派发 |
| GET | `/ws` | WebSocket（`?token=`） |

### 6.3 WebSocket 事件（`/ws?token=`）

消息体是一个 JSON 对象，关键字段：`kind`、`groupId`、`runId`、`messageId`、`delta`、`status`、`error`、`channel`、`phase`、`seq` 等。

| `kind` | 含义 |
|---|---|
| `message_created` | 新消息（含 Agent 回复落地） |
| `message_delta` | 流式增量（`channel=thinking/artifact/final`，`replace` 表示覆盖） |
| `run_status` | run 状态变化（queued/running/done/failed/canceled） |
| `run_heartbeat` | 运行心跳（可选带耗时、RSS） |
| `scheduler_error` | 调度错误 |
| `member_removed` | 成员被移除 |
| `presence` / `presence_snapshot` | 在线状态变化/快照 |
| `ws_reconnected` | 重连提示（前端应全量刷新） |

### 6.4 桌面（Tauri IPC）命令

桌面端与 Web 端共用同一套业务代码；前端通过 `api.ts`（Tauri `invoke`）与 `api-web.ts`（HTTP）两个实现暴露同名方法。**IPC 签名不可改**。

命令清单（97 个，参数为 camelCase）：

```
bootstrap get_group_state list_messages_before get_message_channel_part
get_runtime_settings update_runtime_settings
create_group add_member list_joinable_users remove_member set_admin
send_message cancel_run retry_run set_run_review vote_message
get_message_feedback get_run_phases
get_version_board create_project_version update_version_roadmap
start_version_ask cancel_version_ask approve_version_waves
play_wave pause_wave advance_wave play_version pause_version release_version
set_member_api_url set_member_api_key
check_for_update download_update exit_and_install
detect_agent ocr_image ocr_image_base64 get_preset_roles_command
list_roadmap_items create_roadmap_item update_roadmap_item delete_roadmap_item
list_features create_feature update_feature delete_feature
list_feature_tasks create_feature_task update_feature_task delete_feature_task
get_roadmap_state
save_experience query_experiences delete_experience
list_logs count_logs clear_logs
list_server_dir create_server_dir
update_group_workspace_cmd update_member_workspace_cmd
get_group_announcement set_group_announcement_cmd
ops_release_status ops_job_status ops_run_test_gate ops_deploy_canary
set_group_archived_cmd update_member_model_cmd
create_marketing_campaign list_marketing_campaigns get_marketing_campaign
revise_marketing_campaign approve_marketing_campaign export_marketing_campaign
list_goals get_goal create_goal start_goal pause_goal approve_goal abort_goal
advance_goal set_goal_auto_advance
```

> 注意：桌面侧命令名多为 snake_case，前端 `api.ts` 已封装；Web 侧路径见 §6.2。重写 UI 请只调用 `api.*`，不要自行区分平台。

### 6.5 关键请求/响应示例

**登录**

```http
POST /api/auth/login
{"username":"root","password":"root"}

200 {"token":"<jwt>"}
```

**发消息并触发 Agent**

```http
POST /api/messages
Authorization: Bearer <jwt>
{"groupId":"<gid>","senderMemberId":"<uid>","content":"@Mock 写个大纲","mentionMemberIds":["<mock-agent-id>"]}
```

**建 Goal**

```http
POST /api/groups/{gid}/goals
{"groupId":"<gid>","objective":"完成登录页","budget":{"maxRounds":10,"maxWallTimeMs":3600000,"maxTokens":200000,"maxNoProgressRounds":3,"maxConsecutiveFailures":2},"requireApprovalEachRound":false,"autoAdvance":false}
```

**推进一步**（可能阻塞数分钟，前端需超时/取消策略）

```http
POST /api/goals/{goalId}/advance
200 <Goal>   // rounds 末尾新增一轮；status 可能 completed/aborted
```

**建版本**

```http
POST /api/project-versions
{"groupId":"<gid>","name":"v1.2","what":"…","who":"…","how":"…","oneLiner":"…","mode":"create"}
200 {"kind":"draft","status":"planning", ...}
```

### 6.6 数据模型（前端契约，`src/types.ts`）

```ts
Group { id, name, workspacePath, ownerMemberId, adminMemberId|null, createdAt,
        announcement?, announcementUpdatedAt?, groupKind?: "project"|"chat",
        archived?, isSystem?, unreadCount? }

Member { id, groupId, kind: "user"|"agent"|..., displayName, avatarColor, roleDescription,
         isActive, adapter, executablePath, runtimeStatus: "unknown"|"ready"|"unavailable"|null,
         tags, createdAt, workspacePath?, apiKeySet?, keepAlive?, warmStatus?, model?,
         apiUrl?, authUserId?, invitePending?, systemLocked? }

Message { id, groupId, senderMemberId, parentRunId|null, content, status, createdAt,
          hasThinking?, hasArtifact? }

MessageChannelPart { messageId, channel, text }

TaskRun { id, groupId, rootMessageId, agentMemberId, parentRunId|null, depth,
          status: RunStatus, outputMessageId|null, errorMessage|null,
          reviewStatus: "pending"|"approved"|"rejected"|null, reviewerMemberId|null,
          createdAt, startedAt|null, completedAt|null, phase?, phaseUpdatedAt? }

MessageFeedback { up, down, myVote?: "up"|"down"|null }

GroupState { group, members[], messages[], runs[], messagesHasMore?, messagesTotal? }

ChatEvent { kind, groupId, runId|null, messageId|null, delta|null, status|null, error|null,
            userId?, online?, onlineUserIds?, channel?, replace?, phase?,
            elapsedMs?, totalMs?, seq?, deltaCount?, rssMib? }

ProjectVersion { id, groupId, name, gitTag?, gitSha?, kind, status, what, who, how,
                 oneLiner, requesterMemberId?, createdAt, updatedAt, releasedAt? }

Wave { id, versionId, groupId, idx, title, status, phase, phaseCursor, playState,
       summary, createdAt, updatedAt }

VersionBoard { git: GitSnapshot, versions[], waves[], askingVersionId?, adminMemberId?,
               workspacePath?, workspaceSharedWith? }

RoadmapItem { id, groupId, title, description, status, priority, targetDate|null,
              sortOrder, createdAt }

Feature { id, groupId, title, description, status, priority, area, assigneeMemberId|null,
          targetRoadmapItemId|null, sortOrder, createdAt, updatedAt }

FeatureTask { id, featureId, title, done, sortOrder, createdAt }

GoalBudget { maxRounds, maxWallTimeMs, maxTokens, maxNoProgressRounds, maxConsecutiveFailures }

GoalRound { index, verdict: "progress"|"no_progress"|"failed"|"goal_met",
            tokensUsed, finishedAt, summary }

Goal { id, groupId, objective, budget, status:
       "draft"|"active"|"paused"|"awaiting_approval"|"completed"|"aborted",
       createdAt, startedAt|null, rounds[], stopReason|null,
       requireApprovalEachRound, autoAdvance }
```

其余类型（`Marketing`、`LogEntry`、`OpsJobState`、`RuntimeSettings`、`DirListing`、`Bootstrap` 等）见 `src/types.ts`。

### 6.7 插件与内核契约（内部扩展面）

- 前端场景是内核插件：`svc.scene`（`View` + `openGroupScope(groupId)`），App 通过 `resolveSceneView()` 取用；插件缺失 fail-open。
- 后端内置插件服务：`svc.extensions`、`svc.adapters`、`svc.workflow`、`svc.marketing`；调用方只走服务，内核未装载返回明确错误（Web 为 `503`）。
- 插件清单 `plugin.manifest.json`（v2）：

```json
{"id":"third-party.x","name":"X","version":"0.1.0","entry":"builtin:workflow",
 "provides":["svc.x"],"inject":["svc.workflow"],"permissions":[]}
```

- 磁盘清单扫描：环境变量 `OHMYWORKPANEL_PLUGIN_ROOTS`（`:`/`;` 分隔）；未知 entry / 损坏清单只告警跳过。
- 内核能力（供理解，不必在 UI 暴露）：服务注册表、`inject` 依赖校验、可撤销副作用、`emit/parallel/serial/bail/waterfall`、作用域 `fork()`。

### 6.8 服务端环境变量（部署用）

| 变量 | 作用 |
|---|---|
| `OHMYWORKPANEL_DATA_DIR` | SQLite 与运行数据目录 |
| `OHMYWORKPANEL_PORT` | HTTP 端口（默认 8080） |
| `OHMYWORKPANEL_WEB_DIST` | 前端 dist 目录 |
| `OHMYWORKPANEL_ROOT` | 项目根（工作区/脚本） |
| `OHMYWORKPANEL_PLUGIN_ROOTS` | 额外的插件清单目录 |
| `OHMYWORKPANEL_CODEX_PROXY_*` / `OHMYWORKPANEL_CODEX_*` | Codex 代理与凭据相关（见 systemd unit） |
| `OHMYWORKPANEL_PANELLIVE_ROOT` / `OHMYWORKPANEL_EXTENSION_ROOTS` | 扩展根目录 |

## 7. 前端技术约束（重写时必须遵守）

- 栈：React 19 + TypeScript 5.8 + Vite 7；样式为单个 `src/styles.css` + `src/themes.css`（语义 token）。
- **颜色门禁**：组件样式只允许 `var(--lp-*)`，禁止颜色字面量；`pnpm run check:colors` 必须 0 违规。
- **页面骨架**：新增用户页面必须用 `src/components/ui/PageLayout.tsx`；弹窗/表单/按钮组/徽章/通知/右键菜单优先复用 `src/components/ui/*`（Modal、FormField、ButtonGroup、Badge、Toast、ContextMenu、Divider）。
- **响应式**：需覆盖桌面/网页、窄容器、`max-width: 720px` 手机端；粗指针下触控目标 ≥44px。
- **双端 API**：新增接口要同时改 `src/api.ts`（Tauri invoke）与 `src/api-web.ts`（HTTP），类型放 `src/types.ts`；否则 Web 构建缺方法。
- **目录即架构**：业务按领域放置（`accounts/ agents/ chat/ groups/ members/ workflow/ marketing/ goals/ scene/ extensions/ observability/ shell/ components/`），不要平铺到 `src/` 根。
- **构建**：Web 出包必须 `pnpm run build:web`（桌面版 `pnpm run build` 会产出非 Web 别名包，浏览器打开会报 `invoke` 未定义）。
- **门禁**：`pnpm run test:gate`（tsc + vite build + vitest + cargo 302 + clippy + fmt + colors + 扩展纯度）；`pnpm run check:ai`；提交主题 ≤72 字符。

## 8. 重写 UI 时不可改变的东西

1. 所有 HTTP 路径、方法、请求/响应字段名（§6.2–6.6）。
2. WebSocket 事件 `kind` 与字段（§6.3）。
3. Tauri IPC 命令名与参数名（§6.4）；前端对外仍以 `api.*` 为准。
4. 鉴权方式（Bearer / `?token=`）与 403/401 语义。
5. 主题 token 名称与语义（`--lp-*`），以及 7 套主题的 `data-theme` 值。
6. 嵌入约束：扩展 UI 必须走同源 `/api/extensions/...` 代理，禁止 iframe 直连本机端口。
7. 场景的 fail-open 行为：场景插件缺失/失败时不得白屏。

## 9. 建议的 UI 交付物清单

- 页面/视图：登录/注册/邀请、群列表、群聊（含流式与 run 状态）、成员管理、Agent 配置、版本页（Roadmap/Wave/Ask/Release）、Goal 条与账本、Marketing、扩展、日志/经验、设置、像素场景。
- 每个视图需给出：默认态、加载态（骨架屏）、空态、错误态、无权限态、离线/重连态、移动端布局、7 主题下的对比度检查。
- 交互细节：`@` 提及菜单、`/` 命令、草稿保存、长消息折叠、思考/中间产物按需加载、推进 Goal 的阻塞与取消、run 取消/重试确认、删除类操作二次确认。

## 10. 本地运行与验收

```bash
pnpm install
pnpm run build:web          # 前端产物
cd src-tauri && cargo build --bin ohmyworkpanel-server
# 本地起服务（示例）
OHMYWORKPANEL_DATA_DIR=./.local-dev/data \
OHMYWORKPANEL_PORT=8080 \
OHMYWORKPANEL_WEB_DIST=./dist \
OHMYWORKPANEL_ROOT=. \
./src-tauri/target/debug/ohmyworkpanel-server
# 打开 http://127.0.0.1:8080 ，root/root 登录
```

验收：`pnpm run test:gate` 全绿；`pnpm run check:ai` 通过；浏览器实测登录、建群、发消息、Goal 推进、场景开关。
