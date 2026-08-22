---
date: 2026-08-16
topic: dsh-ui-language-ohmyworkpanel
status: superseded
superseded-by: 2026-08-19-ohmyworkpanel-dsh-ui-design
---

> **已被取代**：完整 UI 方案见 [`2026-08-19-ohmyworkpanel-dsh-ui-design.md`](2026-08-19-ohmyworkpanel-dsh-ui-design.md)。本文保留为历史草稿（本文 11 条映射表已并入新文档）。

# 借鉴 DeepSeek Harness UI 设计语言：三栏 AppFrame（工作区=群聊，右栏=Agent）

> 目标：把 dsh 的 UI 设计与交互语言搬到 ohMyWorkPanel——**中栏是群聊（工作区），右栏是 Agent（成员/子 Agent），左栏是导航**。配套总设计 `2026-08-16-dsh-self-bootstrap-runtime.md` 与群聊治理层 `2026-08-16-group-chat-governance-plane.md`。

## 为什么借鉴（dsh UI 的成熟点）

dsh Web UI（`apps/web` + `packages/client/ui-*`）已经是一套完整、易扩展的三栏面板，实测/已实现的交互有：`ui-layout`（三栏拖拽/折叠/让步）、`ui-sidebar`（会话/subagent 活动）、`ui-conversation`、`ui-details`、goal bar、plan review、`ui-trajectory`（虚拟化轨迹）、todo 行、`ui-tool`/`ui-skill` 行、composer 草稿与 Tab 几何、消息操作/反馈/回合尾动作、队列/后台任务、审批（approval-composer）、权限上下文、模型设置、插件配置、PWA、主题 token。这些和 ohMyWorkPanel 要做的“群聊即工作区 + 右栏 Agent + 自举治理”高度吻合。

## 目标布局：三栏 AppFrame

```text
┌────────────────┬─────────────────────┬──────────────────┐
│ 左栏 Sidebar   │ 中栏 Conversation     │ 右栏 Details        │
│ 群列表          │ 群聊（工作区）         │ Agent（成员）       │
│ · 群导航        │ · 消息流             │ · 成员/子 Agent列表  │
│ · 控制轨(56px)  │ · goal/plan/决策卡    │ · 执行中·排队        │
│ · 模型/设置/技能 │ · trajectory 轨迹     │ · 模型/工作区        │
│ 可折叠          │ · composer（草稿）    │ · 审批/决策          │
│                │                     │ · 会话详情           │
└────────────────┴─────────────────────┴──────────────────┘
 拖拽调宽｜折叠（左栏留 56px 控制轨，右栏可关到 0）
 窗口变窄时 concession 让步：右栏先收，再自动关
```

**关键变换（按用户要求）**
- **工作区＝群聊**：中栏不再是“单会话”，而是所选群的完整群聊（消息流 + @ 触发 + Wave/版本 + 决策卡）。
- **右栏＝Agent**：dsh 的 details 右栏在 ohMyWorkPanel 里就是成员面板（Agent），显示成员/子 Agent、执行中·排队、模型、工作区、审批中心。
- **左栏＝导航**：群里列表 + 控制轨（折叠态）+ 模型/设置/技能/扩展入口。

## 借鉴映射表（dsh 组件 ↔ ohMyWorkPanel 现状 ↔ 改造）

| # | dsh 组件/能力 | ohMyWorkPanel 现状 | 借鉴改造 |
|---|---|---|---|
| 1 | 三栏 AppFrame（拖拽/折叠/让步） | 现有左群聊+中消息+右成员三栏，但固定宽度 | 引入拖拽分界、左栏折叠留 56px 控制轨、右栏可关、窗口窄时 concession |
| 2 | theme presenter（color-scheme + 别名 token + meta theme-color） | 已有 `theme.tsx` + `themes.css` | 统一成 token 化明暗主题，`color-scheme`/`meta theme-color` 对齐 dsh |
| 3 | composer（草稿滚动、Tab 几何、question composer） | 已有发送框/OCR/语音，`sendKey` 切换 | 补草稿持久、Tab 在动作间移动、粘贴即 OCR、发送态更清晰 |
| 4 | goal bar / plan review | 已有版本/Wave | 把当前群 Wave/目标上移到“goal bar”；`/propose` 出决策卡即 plan review |
| 5 | trajectory（虚拟化轨迹）/ todo 行 | 有思考/中间产物折叠 | 加 run 级「轨迹」视图（会话回放 P1 的 UI 载体）+ 待办行呈现 |
| 6 | ui-tool / ui-skill 行 | `@` 触发 + 扩展页签 | 把工具/技能/knowledge（Wiki/经验）从输入框额外暴露成清晰入口 |
| 7 | 消息操作/反馈/回合尾动作 | 有停止/重试/朗读 | 补点赞/踩反馈、回合尾动作（继续/改要求/交给某 Agent） |
| 8 | 队列 / 后台任务 | 有成员排队展开 | 用 dsh 的队列/后台任务卡片呈现多 Agent 排队与取消 |
| 9 | 审批 approval-composer / 权限上下文 | 有计划做决策卡 + `/approve` | 审批以 dsh 风格内联在消息/右栏，绑定决策卡（治理层落地时做） |
| 10 | details：会话详情/lifecycle/settings | 已有群设置/运行设置 | 右栏承载会话详情 + 交接摘要（context_seams 已有） |
| 11 | 无障碍/键盘/PWA | 基本 | 对齐焦点顺序、`robust` 键盘、离屏时可 PWA 化 |

## 设计原则（延续 dsh）

1. **三栏想清楚再看内容**：中栏永远是“这件事”的主场，右栏是“这个 Agent/会话”的详情，左栏是“去哪/切换”。
2. **几何临时、状态进存储**：面板宽/折叠不进 `localStorage`（dsh 如此），业务状态进结构化层/DB。
3. **主题单一事实源**：渲染背景是颜色权威，token 化、可暗色。
4. **本地优先 + 离线友好**：PWA/Service Worker（`public/sw.js` 已有基础），右键栏本地即时响应。
5. **接纳新交互不改核心 API**：UI 只读后端现有接口 + 决策卡新接口；不破坏 `tauri::command` 签名。

## 分阶段（UI 侧，独立于轨道 G 的执行计划）

| 阶段 | 内容 | 状态 |
|---|---|---|
| UI-P0 | 三栏 AppFrame：拖拽分界、左栏折叠 56px 控制轨、右栏成员可关、窄屏 concession | 待立项 |
| UI-P1 | composer 升级（草稿/Tab/粘贴 OCR）+ 消息操作/反馈 + 队列卡片化 | 待立项 |
| UI-P2 | goal/Wave 上移 + plan(决策卡) + run 轨迹视图 + 审批内联（配合治理层 MVP） | 待立项 |

## 不做 / 边界

- 不把 dsh 前端代码直接搬进 ohMyWorkPanel（两套 React 栈、依赖不同；只借鉴**设计语言与交互**）。
- 不把“单会话”当作群聊替代：ohMyWorkPanel 保持多群、多 Agent 的群聊模型，不退回单人会话。
- UI 改动必须过 `pnpm run test:gate` 与前端壳冒烟；不破坏桌面(Tauri)与 Web 双模式。

## 验收

- 窄窗下右栏自动让步/关闭，左栏折叠成控制轨；拖拽后可调三栏宽度。
- “工作区=群聊、右栏=Agent”直观成立：中栏是群聊消息+Wave/决策卡，右栏成员含执行中·排队/模型/审批。
- 明暗主题彻底 token 化，`meta theme-color` 跟随背景。
- 灰度回归后 promote；桌面与 Web 行为一致。
