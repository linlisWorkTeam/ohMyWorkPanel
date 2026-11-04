---
date: 2026-11-05
topic: frontend-redesign-implementation
branch: main (local, based on b2ce66c)
status: active
---

# Epitaph: 前台工作台重设计实施（第一轮）

## Built this session
按 `openspec/changes/0002-frontend-workbench-redesign/` 重写前台，五个提交：

- `1999aa9` 群内导航（协作/交付/目标/内容/场景）+ 目标页（列表/详情/预算/账本/审批/自动）
- `a861129` 内容页（campaign 列表/详情/brief/草稿/校验/修订/批准/导出）
- `0be6304` 可观测页（日志/经验/指标/运维）
- `e91a0ae` 交付页（路线图/版本）+ 版本列表 240px / 详情
- `5e2cb47` 设置页（外观/输入行为/运行时/桌面更新）
- `c376eeb` 协作增强：消息 run 元信息、详情面板、审阅、长消息折叠；≤720 隐藏控制轨并抽屉化
- `bdf475f` 登录/邀请重排 + 扩展页（群导航「扩展」）

## Key paths
- `src/shell/GroupNavigation.tsx`、`src/shell/Shell.tsx`、`src/shell/ChatTranscript.tsx`
- `src/goals/GoalsPage.tsx`、`src/goals/labels.ts`
- `src/workflow/DeliveryPage.tsx`、`src/workflow/VersionView.tsx`
- `src/marketing/MarketingPage.tsx`
- `src/observability/ObservabilityPage.tsx`
- `src/settings/SettingsPage.tsx`
- `src/extensions/ExtensionsPage.tsx`
- `src/members/RunDetails.tsx`
- `src/styles.css`（新增页面样式与 ≤720 规则）
- `src/shell/tokens.css`（atlas/moss/noir 的 `--dim` 调整）
- `openspec/changes/0002-frontend-workbench-redesign/tasks.md`（第 7 节实施状态）

## Locked product decisions
- 不改 HTTP/IPC/WS 契约；前台只调用 `api.*`。
- 页面样式只用 `--lp-*` token；`check:colors` 必须 0 违规。
- 版本 Release ≠ 部署；Ops 写操作仅管理员，前台无生产发布按钮。
- 场景 fail-open；扩展走同源代理。

## Known pitfalls
- 浅色主题（atlas/moss）原 `--dim` 对比度仅 3.85 / 4.62；已调深。**选中页签**原用 accent 作文字色（atlas 3.02），已改为 `--lp-text` + accent 下划线。改动后审计所有抽样 ≥5.8:1。
- `VersionView` 重写后，旧样式类（`.version-card`）仍存在但不再使用；新增 `.version-pane/.version-item/.version-detail`。
- 扩展页本地无扩展时显示空态，依赖 `OHMYWORKPANEL_EXTENSION_ROOTS`。
- 首次访问（无 token）的 401 不再提示「登录已失效」，靠 `goLogin` 里 `hadToken` 判断。
- 截图用无头 Edge + CDP（`--remote-debugging-port=9222`，Node 原生 WebSocket），token 从 `.local-dev/token.txt` 注入；脚本在 `.local-dev/*.mjs`（已 gitignore）。
- `pnpm run build` 生成桌面 dist；浏览器要用 `pnpm run build:web`。

## How to run / verify
```bash
pnpm run test:gate            # cargo 304 / tsc / vitest / clippy / colors
pnpm run check:ai
# 本地服务
OHMYWORKPANEL_DATA_DIR=./.local-dev/data OHMYWORKPANEL_PORT=8080 \
OHMYWORKPANEL_WEB_DIST=./dist OHMYWORKPANEL_ROOT=. \
./src-tauri/target/debug/ohmyworkpanel-server   # root/root
```
截图矩阵：`output/theme-<theme>-{login-1440,shell-1440,settings-1440,shell-1024,shell-390}.png`（7 主题 × 5），边界 `theme-atlas-shell-320/721.png`。

## Do not regress
- 群内导航 6 入口与 active 状态；≤720 控制轨隐藏后，侧栏底部仍保留设置/可观测/Agent 配置入口。
- `advance` 超时文案「结果待确认」，不显示已停止。
- 未批准 campaign 禁止导出；指标在桌面显示「仅 Web」。
- 首次访问不误报会话过期。

## Open follow-ups
- `App.tsx` 仍约 2100 行，M1 状态拆分未做；未达 design.md 的 <250 行目标。
- 聊天输入区（@ 菜单、/ 命令、引用、OCR）未按 UI 规格重排；P05 Agent 配置页未重排。
- 成员表单/建群弹窗/右栏队列样式沿用旧实现。
- 旧壳回退开关、200% 缩放与粗指针实测未做。
- 七主题截图未逐张人工评审，对比度只覆盖主壳抽样元素。

## 像素级对齐追加（同日后续轮次）
- `fd8a46b` 断点 1279/720、栏宽 232/208/320、群头 64/56、导航 44、正文 880、页面内距四档、控件 36/40、对话框 480/720
- `15474ea` 项目摘要条压到 56px（≤96）可折叠并记忆；「N 条新消息」
- `294d47a` Agent 环境页 PageLayout + PageHeader + PageSection
- `8f9e0ce` 成员行 64、头像 32、窄屏按窗口宽度收敛
- `9b26a8e` 对话框 480、右栏页签 40、成员表单 40、页面标题 20/28、路径按钮不换行
- 排版/焦点：辅助信息 12/18、代码块 12/18、全局 focus-visible 2px/2px
- 截图矩阵：`output/theme-<theme>-*.png`（35 张）+ `bp-*.png` + `zoom-200.png`

## 最终验收（第 21 轮汇总）

### 提交
`fd8a46b` 尺寸对齐 → `15474ea` 摘要条/未读 → `294d47a` Agent 环境页 → `8f9e0ce` 行高/头像/抽屉 → `9b26a8e` 弹窗/右栏/标题 → `640d906` 排版/焦点 → `994bca8` 成员类型化 → `1b0377f`/`4594a66`/`4b14e77`/`cf3c6cf`/`04d3b32` M1 五刀 → `52c915f` 群设置排版 → `d1501f3` 路径/场景控件 → `81b9a15` 分层。

### 实测指标
- 断点：1440/1280/1279/1024/960/959/721/720/390 全部无横向溢出
- 控件：按钮 36/40、输入 40、对话框 480（320 为底部抽屉）、页签 40、行 56/64、头像 32
- 排版：标题 20/28、区块 16/24、正文 14/22、辅助 12/18
- 对比度：七主题抽检最低 **5.81:1**（阈值 4.5）
- 200% 缩放：720 CSS px @ DPR2 无溢出
- 矩阵：`output/theme-*.png` 35 张 + `bp-*.png` + 专项截图

### M1（设计任务）进度
五刀完成：消息/runs 合并、presence/未读、事件路由、boot、WS 订阅体；共 35 个纯逻辑测试。`App.tsx` 约 2191 行，仍未达 design 的 <250 行提示。

### 剩余
- App 装配层（订阅依赖装配、草稿/视图状态）未完全下沉；
- 旧壳回退不适用（原地替换，无可回退旧壳）；
- 真实 CLI Agent E2E 受机器凭据限制，未在本轮验证。
