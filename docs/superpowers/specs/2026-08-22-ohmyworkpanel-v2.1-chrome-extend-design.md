---
date: 2026-08-22
topic: ohmyworkpanel-v2.1-chrome-extend
status: implemented
decider: user (2026-08-22 plan)
---

# 设计：ohMyWorkPanel v2.1.0 前台壳层扩展 + 七主题成戏

> 交互示意图（静态、可点）：[`docs/ui-v2.1-shell.html`](../../ui-v2.1-shell.html)  
> 配套：DSH UI SSOT [`2026-08-19-ohmyworkpanel-dsh-ui-design.md`](2026-08-19-ohmyworkpanel-dsh-ui-design.md)；widget 形态 [`2026-08-16-widget-capability-placement.md`](2026-08-16-widget-capability-placement.md)。
> 发版：与 HEAD 白屏修复（`032d4f9`，`Brand` keep-alive）合并打 **git tag `v2.1.0`**。不改写已发布的 `v2.0.0`。

## 0. 结论先行

v2.1.0 做四件事，打进**同一个 tag**：

1. **Base / Extend 贡献协议**：侧栏、输入栏、状态栏、**消息操作**可挂组件；Cordis 之后往同一张表注册，本发不接 Cordis 运行时。
2. **右栏可停靠**：默认单列页签；Extend 可申请第二列并排（成员 | Live），细拖条，几何只存本机。
3. **七主题**：原六套各自成戏 + 第 7 套 **`minimal` / 极简**。签名花在主题成戏上。
4. **对话手感靠近微信**：长按/右键菜单、引用条、语音气泡；流式中「停止」贴在气泡上。不做朋友圈、已读战、赞踩。

中栏头图标簇（🎨⚙酒/L/?）取消。主题、运行设置、群设置进入右栏第 4 个基线页签 **设置**。

## 1. 非目标

- 不改 `tauri::command`、SQLite schema、生产 unit。
- 不加 `/play /pause /advance /release`（会 spawn / 触发布仪式）。
- 不把 widget 建成新群。
- 不引入组件库 / Tailwind / 把 dsh React 搬进来。
- 本发不实现 Cordis 宿主；只留 TypeScript 贡献类型 + 假贡献单测。

## 2. 槽位与贡献协议

```ts
type ContribSlot = "right-tab" | "right-dock" | "composer-tool" | "status" | "message-action";

type UiContribution = {
  id: string;                 // 稳定 id，如 "core.settings" / "ext.panellive"
  title: string;
  slot: ContribSlot;
  origin: "base" | "extend";
  order?: number;
  motion?: "none" | "enter" | "ambient";
  /** right-dock：从页签拆成并排第二列；用户可拖回页签 */
  dockable?: boolean;
  render: () => unknown;      // ReactNode；Cordis 以后可换成 mount(el)
};
```

**Base 贡献（写死注册，不可卸载）**

| id | slot | 说明 |
|---|---|---|
| `core.members` | `right-tab` | 成员：通讯录式列表；检测 / 设管理 / 移除进右键 |
| `core.queue` | `right-tab` | 队列 |
| `core.details` | `right-tab` | 详情：版本 / 经验 / 日志（**不再含群设置**） |
| `core.settings` | `right-tab` | 设置：外观 / 运行 / 本群 |
| `core.mention` | `composer-tool` | `@` |
| `core.slash` | `composer-tool` | `/` 仅 `/board /approve /wave` |
| `core.ocr` | `composer-tool` | 🖼 |
| `core.msg.copy` | `message-action` | 复制 |
| `core.msg.quote` | `message-action` | 引用 |
| `core.msg.retry` | `message-action` | 重试（仅失败/可重试 run） |
| `core.msg.speak` | `message-action` | 朗读 |

**Extend 贡献（现有扩展映射）**

| 来源 | 默认 slot | 可 dock |
|---|---|---|
| PanelLive / AIHotel 等 `collectExtensionTabViews` | `right-tab`；清单可声明 `slot: "right-dock"` | 是 |
| 酒馆开关 | `status`：只改变全局渲染或拉底层服务，**不再占中栏头** | 否 |
| 将来 Cordis | 同一 `UiContribution` | 由清单声明 |

`status` 槽：本发允许 0–2 个指示器（例如「Live 会话中」），放中栏头右侧极窄处或设置页顶部，禁止再堆 7 个图标。

宿主：`src/contrib/registry.ts` + `RightDockHost` + `ComposerToolRow` + `MessageActionMenu`。`App.tsx` 只组装业务状态，不直接 `rightPanelTab === "members"` 三分支写死。

## 9. 对话交互（靠近微信）

工作能力还在（@Agent、run、停止/重试），外壳学微信：

- **左右气泡**：自己右、他人/Agent 左；圆角「自己缺一角」。
- **长按 / 右键菜单**：复制、引用、朗读、重试（若有）；Extend 往 `message-action` 加项。菜单绝对定位，不改变气泡高度。
- **气泡常驻**：仅流式/排队中的 **停止**（像微信语音条上的控制）。结束后气泡干净。
- **引用**：composer 上方一条可关闭灰条；发送带引用摘要（`quoteMessageId` 若本发不加 schema，先打进正文前缀）。
- **语音**：按住说话；气泡为时长条，点击播放。
- **未读**：角标保留；进群落到第一条未读。不做已读战、赞踩、朋友圈、Slack 线程分栏。

## 10. 成员列表（通讯录，不是工具条）

现 `MemberRow` 把「检测 / 设管理 / 移除 / DSH」摊在每一行，窄栏会挤、也不好看。v2.1 改成微信通讯录式：

- **行上只留身份**：头像（色块 + 在线点）、姓名、角色微标（群主 / 管理 / 系统 / 链接中）、一行次要信息（适配器 · 模型 · 就绪态）、右侧短状态（在线 / 执行中 / 不可用）。
- **管理动作进长按 / 右键**（与气泡同一套浮层，不改变行高）：Agent → 检测、设/撤销管理、切换模型、移除（系统锁定项禁用移除）；人类群主几乎无破坏项；邀请中 → 复制邀请 / 撤销邀请。
- **DSH Web** 同样进菜单，不占行。
- **列表底**一条虚线「＋ 邀请成员」。模型不做成每行一个 `<select>` 占位；切换走菜单。
- 忙碌时头像点变强调色，状态字变「执行中」；点状态仍可展开该 Agent 的排队（现有能力，不丢）。

## 3. 右栏几何

- 默认：单列 + 页签（成员 | 队列 | 详情 | 设置 | …Extend）。
- Extend `dockable`：页签右侧出现「拆出」；拆出后为两列，中间 4px 拖条，最小列宽 220px，总宽受现有右栏上限约束。
- 几何（是否 dock、第二列宽）进 `sessionStorage`（刷新保留、不进 DB、不跨用户）。
- 窄屏：第二列先收成页签（concession），再关右栏。无弹跳。

## 4. 设置页结构

右栏 **设置** 三块，纵向滚动：

1. **外观**：七张主题舞台卡（含该主题材质预览，不只色点）。
2. **运行**：现「运行设置」模态内容（发送键、Extend 开关、心跳等）。
3. **本群**：公告 + 工作区路径（从详情迁出）。

左栏底「运行设置」入口删除，避免三处。Agent 配置仍走管理员主视图（◇），不塞进设置页——它是整页工作台不是偏好。

帮助（原 `?`）：设置页底「键盘与斜杠」折叠，或 Esc 仍关浮层。

## 5. 七主题

`ThemeId` 增加 `"minimal"`。六套 ID 不变。

| id | 展示名 | 材质 | 口音动效（`prefers-reduced-motion` 时全关） |
|---|---|---|---|
| cyberpunk | Cyberpunk | 夜玻璃、青/洋红描边 | 极慢扫描线；气泡入场微色散 |
| industrial | Industrial | 钢板、铆钉、警戒斜纹 | 页签硬切，无弹性 |
| atlas | Atlas | 航图纸、细线 | 短淡入 |
| forge | Forge | 炭黑、炉口橙 | 悬停热晕；发送钮按压 |
| moss | Moss | 苔毡、软边 | 呼吸亮度 ≤2% |
| noir | Noir | 胶片硬光、细颗粒 | 硬切 |
| **minimal** | **极简** | 冷灰纸 `#f4f5f7` + 墨 `#17181a`，无纹理无辉光，强调色只用石墨描边 | **无 ambient / enter 口音** |

组件只消费 `--lp-*` / `--motion-*`。禁止组件内写死 `#hex`（现有债在本发能碰到的 chrome 上清掉）。

贡献 `motion: "ambient"` 在 `minimal` 与 `prefers-reduced-motion` 下被宿主忽略。

## 6. 测试

- 注册表：Base 四页签 id 稳定；Extend 可追加 `right-tab` 而不改 `App.tsx` 联合类型。
- Dock：拆出 / 拖回改变列数；窄屏把 dock 收成页签。
- Composer：`core.slash` 命令列表仍只有三条。
- 主题：七 id 均可 `data-theme` 切换；`minimal` 下 `--scanline-opacity: 0`。
- 回归：`themeChromeKeepAlive`（白屏）、`appHooksOrder`、`uiDemoParity`（设置页签；无中栏头图标簇；气泡无常驻复制条）。
- 对话：长按/右键打开菜单不改变列表高度；仅 streaming 气泡有停止按钮。
- 成员：行上无检测/设管理/移除按钮；这些项出现在右键菜单。

## 7. 发版

1. 占位：`docs/version-pipeline.md` 阶段 7 = v2.1.0。
2. 灰度 `:8081` → §F 壳冒烟 + 七主题各一眼。
3. `git tag v2.1.0`（含 `032d4f9` 白屏修复）。
4. 晋升生产须人类 `approve-prod-release.sh && promote-canary.sh`。

## 8. 示意图

打开 [`docs/ui-v2.1-shell.html`](../../ui-v2.1-shell.html)：可切七主题、切右栏页签、拆出 Live、右键气泡看菜单。这是设计稿不是产品运行时。
