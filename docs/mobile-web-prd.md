# PRD：ohMyWorkPanel 手机端 Web 版本（方案 A：响应式适配 + PWA）

- 作者：曲奇 🍪（PM）
- 日期：2026-08-03
- 状态：已评审，交付 cursor-agent 实现
- 验收人：linli

## 1. 背景与目标

ohMyWorkPanel 目前是 Tauri 2 桌面应用，但已具备 `build:web` 纯 Web 构建模式（api-web.ts 走 HTTP/WS + token 鉴权），Rust 服务端已在 8080 端口提供 Web 版页面。手机浏览器可访问但体验差（960px 最小宽度导致横向滚动、三栏布局无法操作）。

**目标**：在不改动后端与数据模型的前提下，让 Web 版在手机端（窄屏触屏设备）可用、好用，并支持 PWA「添加到主屏幕」获得类 App 体验。

**成功标准**：iPhone/Android 浏览器访问 `http://<host>:8080`，可完成登录、切换群、收发消息、@ 提及 Agent、查看成员/PM 面板；无横向滚动；可添加到主屏幕并独立窗口打开。

## 2. 范围

### 做（本迭代）
1. 移动端响应式布局适配（<768px 断点）
2. 触屏交互优化（点击目标尺寸、滑动、抽屉式导航）
3. PWA 支持（manifest + 图标 + service worker 基础缓存）
4. 构建与部署验证（`pnpm build:web` 通过，8080 服务正常）

### 不做（明确排除）
- 不改 Rust 后端、SQLite schema、tauri::command 签名、IPC 契约
- 不做原生 APK / Tauri Android（后续迭代，见 roadmap）
- 不做推送通知（依赖后端改造，另行立项）
- 不改桌面端布局与交互（桌面体验回归不得劣化）

## 3. 现状关键约束（已核实）

- `src/styles.css`：单文件 218 行压缩风格，`body { min-width: 960px }` 是移动端最大障碍；已有 `@media (max-width: 1080px)` 断点（隐藏成员面板为固定抽屉，`.mobile-members` 显示）
- 布局骨架：`.app-shell` 三栏 grid（236px 群侧栏 + 聊天 + 310px 成员/PM 面板）
- `dist/` 为 web 构建产物，`vite.config.web.ts` 负责 web 构建（stub Tauri）
- `package.json` 已有 `build:web` 脚本
- 桌面端有 `src-tauri/`，web 构建不触碰
- 图标资源在 `assets/`（91KB 自定义图标，可用于 PWA）
- 工作区有未提交改动（Rust + 前端），实现前先 `git status` 确认，避免混入

## 4. 技术方案

### 4.1 响应式布局（styles.css）
- 移除 `body { min-width: 960px }`，改为 `min-width: 0`，保证窄屏无横向滚动
- 新增 `@media (max-width: 768px)` 移动端断点：
  - `.app-shell` 改为单列（仅聊天面板），群侧栏与成员面板变为**抽屉**（fixed 定位 + transform 滑入滑出 + 半透明遮罩）
  - 聊天头部增加「☰ 群列表」「成员」两个移动端入口按钮（可复用现有 `.mobile-members` 模式）
  - `.message-row` 最大宽度放宽至 92%，间距收紧；`.bubble` 字号 15px（触屏可读性）
  - 输入区 textarea 高度加大、发送按钮加大（≥44px 触控目标）
  - `@` 提及菜单（`.mention-menu`）宽度适配为 `calc(100vw - 32px)`，避免溢出
  - 模态框（`.modal`）已用 `min(430px, calc(100vw - 34px))`，保持即可
- 可选：`env(safe-area-inset-*)` 适配刘海屏底部输入区
- 桌面端（>768px）行为保持现状不变，1080px 断点保留

### 4.2 移动端抽屉交互（App.tsx 或独立组件）
- 新增两个状态：`sidebarOpen` / `membersOpen`（布尔）
- 群侧栏抽屉：点击聊天头部「☰」打开，点击遮罩或选中群后关闭
- 成员面板抽屉：点击「成员」打开（移动端下成员面板不再常驻）
- 注意：`.mobile-members` 已存在，先查其现有绑定逻辑再扩展，避免重复实现
- 触屏滚动：抽屉内部 `overflow-y: auto`，`-webkit-overflow-scrolling: touch` 视需要

### 4.3 PWA
- 新增 `public/manifest.webmanifest`（或沿用 dist 输出约定）：
  - name: ohMyWorkPanel，short_name: ohMyWorkPanel，display: standalone，background/theme color 取现有配色（#202a3a / #eff2f5）
  - icons: 复用 `assets/` 现有图标，补齐 192px / 512px 尺寸（用现有源图缩放生成，若无 512 源图则放大导出）
- 新增 `public/sw.js`（或 vite-plugin-pwa 引入，二选一，**倾向轻量手写**，避免新增构建依赖）：
  - 基础缓存：静态资源（/assets/*）cache-first，网络优先回退缓存（HTML）
  - 版本号常量，`skipWaiting` + `clients.claim`
- `index.html` 增加 manifest link、theme-color、apple-touch-icon
- 注册：入口处 `if ('serviceWorker' in navigator) register('/sw.js')`（web 构建专用，Tauri 桌面构建不注册——注意 vite.config.web.ts 的 stub 机制，注册代码放 App 入口并判断环境）

### 4.4 构建与验证
- `pnpm build:web` 必须通过（tsc -b 不跑，仅 vite build，与现有脚本一致）
- `pnpm test` / `pnpm test:gate` 不得因本次改动失败（如涉及前端测试文件需同步更新）
- 8080 服务部署：确认现有发布流程（deploy-canary / test-gate / promote-canary），本任务产物为构建 + 提交，**是否热部署由 linli 确认后执行**（默认只构建验证 + git 提交）

## 5. 任务拆分（cursor-agent 执行清单）

1. `git status` 确认工作区状态，记录未提交改动清单，避免混淆
2. styles.css：移除 min-width 约束，新增 768px 断点全套移动端样式（4.1）
3. 前端：移动端抽屉状态 + 聊天头部移动端按钮（4.2），先查 `.mobile-members` 现有逻辑
4. PWA：manifest + sw.js + index.html 注入 + 图标生成（4.3）
5. `pnpm build:web` 构建验证；`pnpm test:gate` 回归
6. git 提交（单独 commit，message 如 `feat: mobile web responsive layout + PWA`）
7. 输出实现摘要：改了哪些文件、验证结果、遗留事项

## 6. 验收标准（linli 验收）

- [ ] 手机浏览器（窄屏）访问 8080：无横向滚动，三栏变抽屉可切换
- [ ] 登录、切群、发消息、@ 提及、成员面板、PM 面板在移动端可操作
- [ ] 触控目标 ≥ 44px（发送按钮、抽屉按钮）
- [ ] 桌面端（>1080px）视觉与操作与改动前一致
- [ ] 可「添加到主屏幕」，standalone 打开，图标正常
- [ ] `pnpm build:web` 通过，`pnpm test:gate` 通过
- [ ] 改动为独立 git commit，未混入此前未提交改动

## 7. 风险与备注

- **风险**：styles.css 为单文件压缩风格，改动面大，需逐个断点核对；用浏览器 DevTools 设备模拟自测
- **风险**：sw.js 缓存策略若错误会导致旧资源，version 常量 + network-first(HTML) 规避
- **备注**：PWA 图标若 assets 源图不足 512px，放大导出即可（可接受轻微模糊），不追求完美
- **备注**：本任务不部署线上，部署动作等 linli 指令
