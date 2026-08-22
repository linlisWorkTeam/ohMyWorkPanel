---
date: 2026-08-22
topic: workpanel-v2.1.1-shell-visual-parity
status: accepted
decider: user (2026-08-22)
version: 2.1.1
---

# 设计：v2.1.1 按 `ui-v2.1-shell.html` 重画主壳 + 全站 token 对齐

> 视觉 SSOT：[`docs/ui-v2.1-shell.html`](../../ui-v2.1-shell.html)（可点示意图，不是运行时）。  
> 交互契约仍以 [`2026-08-22-workpanel-v2.1-chrome-extend-design.md`](2026-08-22-workpanel-v2.1-chrome-extend-design.md) 为准（四页签、dock、斜杠三条、贡献协议）。本文件只解决 **「产品不像那张稿」**。  
> 版本：**v2.1.1**（patch）。前台壳允许灵活重构，不为此单开 2.2。

## 0. 拍板（已确认）

| 项 | 选择 |
|---|---|
| 视觉稿 | **A.** `docs/ui-v2.1-shell.html` |
| 主壳做法 | **2.** 按稿 DOM 重画壳，App 只留数据/命令 |
| 画面范围 | **C.** 所有 Web 画面一次进灰度 |
| 稿外页面 | **1.** 同一视觉语言，**保留现有版式与字段** |
| 实现结构 | **A.** 新壳组件 + `--lp-*` / `--s-*` **别名**到稿 token |
| 版本号 | **2.1.1**，不是 2.2.0 |

## 1. 非目标

- 不改 `tauri::command`、SQLite schema、生产 systemd unit。
- 不实现 Cordis 运行时；不加 `/play /pause /advance /release`。
- 不为登录/版本/Agent 配置/Live/邀请另画第二张示意图，也不重排它们的信息结构。
- 不把 widget 建成新群；不擅自 `promote-canary`。
- 不把 `ui-demo.html` 继续当像素 SSOT（门禁从 `--s-*` 城市底图断言迁走）。

## 2. 架构

```text
API / WS → App（唯一状态机：鉴权、群、消息、成员、dock）
             ├─ login / checking     → AuthScreen     （旧版式 + 新 token）
             ├─ 无群                 → EmptyHome      （同上）
             ├─ versions / agent-config / dsh
             │                       → 现有全页       （同上）
             └─ 主聊天               → Shell
                  ├─ rail / 群列表
                  ├─ ChatTranscript + Composer
                  └─ RightDockHost → Roster | 队列 | 详情 | 设置 | Extend dock
```

- Token SSOT 从稿抽出：`--bg --surf --elev --ink --dim --acc --acc2 --line --user`，字体 IBM Plex Sans / Mono，每主题 `--radius-mode`、扫描线/颗粒/呼吸。七 id 不变（含 `minimal`，扫描线 0）。
- 现有 `--lp-*`、`--s-*` **别名到上述变量**，稿外页面不改 JSX 也能换皮肤。
- `src/shell/` 新建：`tokens.css`、`Shell.tsx`、`ChatTranscript.tsx`、`Composer.tsx`、`Roster.tsx`。`App.tsx` 不再拼三栏 DOM。
- `RightDockHost` 保留为右栏宿主。

## 3. 主壳（按稿重画 DOM）

对齐稿的可见结构：

- 栅格：56px 轨 | 左群 | 中栏 | 右栏；Extend 可 dock 第二列。
- 中栏头只留群身份（名称 + `project`/`chat`）；不要图标簇。窄屏打开右栏走轨或单一 ☰，不恢复 🎨⚙。
- WAVE 虚线条保留（稿有）。
- 气泡：他人左、自己右；圆角「自己缺一角」；时间在泡下；引用是泡内左竖条；语音是时长条；流式只露「停止」；失败泡描边，重试只在长按/右键。
- 思考/产物画在气泡**内部**虚线块；**删除**整条 `agent-reply-fold`。
- 成员：通讯录行（头像点、名、角色微标、次要行、右侧短状态）；**无行内按钮、无 `⋯`**；检测/设管理/移除/模型/DSH 仅长按或右键。
- 设置外观：七张小舞台卡（色条 + 名），不要 ui-demo 那种大预览剧场卡。
- Composer：引用灰条、`@` `/` `🖼`、按住说话、发送。斜杠仍只有 `/board /approve /wave`。

`parseMessageContent`、发送、引用前缀（无 `quoteMessageId` 列）不变。

## 4. 稿外页面（换皮不换版式）

`AuthScreen`、`EmptyHome`、`VersionView`、`AgentConfigView`、`InviteLanding`、`ExperiencePanel`、`LogsPanel`、`ExtensionPanel` / `LivePanel`：组件树、字段、路由/页签行为不变。按钮、输入、卡片、边框、toast、WS 横幅改吃稿 token（可加一层共享 class，不重做布局）。

错误仍走 App 的 `error` 条，视觉改为 `--elev` + `--line`，不新做错误总线。

## 5. 测试

- `uiDemoParity` **改锁本稿**：七主题 `--bg` 与 `ui-v2.1-shell.html` 一致；`minimal` 扫描 0；Shell 含轨+左+中+右；`.row.me` 缺一角圆角；源码无 `agent-reply-fold`；成员行无 `⋯`；无中栏 `title="外观主题"` / `🎨`。
- 保留：`themeChromeKeepAlive`、`appHooksOrder`、`FALLBACK_CLI_ADAPTERS` 值导入、contrib registry、dock、quote/unread。
- 合入前 `pnpm run test:gate`。

## 6. 发版

内部顺序（一次进灰度，不拆用户可见小版本）：token 别名 → Shell/Chat/Roster/Composer → 稿外换皮 → 删除与稿冲突的旧 chrome CSS。

1. `deploy-canary.sh`（`:8081`）+ `release-checklist.md` §F。
2. 人工：稿与灰度并排；七主题各一眼；四页签与 dock；气泡菜单不撑高；登录/版本/配置/Live/邀请确认「熟版式、新皮肤」。
3. `package.json` / Cargo 对齐 **2.1.1**；灰度通过后打 tag `v2.1.1`。
4. 晋升生产必须人类 `approve-prod-release.sh && promote-canary.sh`。

## 7. 与 v2.1.0 的关系

v2.1.0 交付的是**交互契约**（贡献槽、设置进右栏、微信手势、七主题 id）。v2.1.1 交付的是**视觉兑现**（DOM/token 跟静态稿）。不回退 v2.1.0 的行为锁定。
