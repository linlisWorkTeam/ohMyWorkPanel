---
date: 2026-08-20
topic: v1.3-ui-demo-chrome-parity
branch: master
status: active
---

# Epitaph: 按 `docs/ui-demo.html` 对齐壳层 + 修登录/思考过程崩溃

> 上一份 P0–P2 交接：[`2026-08-20-dsh-ui-design-p0-p2.md`](./2026-08-20-dsh-ui-design-p0-p2.md)（后端四项仍以那份为准）。
> 交互原型 SSOT：[`docs/ui-demo.html`](../ui-demo.html)。Spec 仍写「不 rewrite」——**产品侧已改口**：壳层必须跟 demo 走，不再只换 token。

## Built this session

**为何看起来不像 demo**
- P0–P2 把 `--lp-*`、Divider、goal bar **螺栓在旧 ohMyWorkPanel chrome 上**：城市底图、聊天/版本/设置胶囊、composer 外置发送、右栏六七个页签。Gate 只跑 tsc/拖拽/主题，从未像素对照 HTML。

**壳层对齐（本次）**
- 6 套主题末尾覆盖 `--s-*` 色板，`--lp-*` / `--bg-*` 双写；去掉 cyberpunk/industrial 城市 SVG；Atlas/Moss 侧栏改为与 demo 相同的浅底深字。
- 中栏头：标题 + `project`/`chat` chip；☰ 成员、🎨 主题、⛭ 版本、◇ Agent 配置、? 帮助。去掉胶囊 `view-toggle`，字幕不再甩 `\\?\` 工作区路径。
- Composer 改成 demo 卡片：`@` / OCR / 斜杠在内，发送在 hint 行。
- 左栏折叠为**独立 56px 图标轨**（◉◎◇⚙▶），不再把群列表压成首字头像。
- 项目群 WAVE 条**常驻**（无版本时「尚未建立 Wave · 点此到版本页」）。
- 右栏收敛为 **成员 / 队列 / 详情**；群设置、版本、经验、日志进详情。

**崩溃修复（同工作树，勿丢）**
- 登录 React #310：`Ctrl+1/2` / Esc 的 `useEffect` 必须在 `session === "login"` 早退之前（`src/appHooksOrder.test.ts`）。
- 点「思考过程」白屏：P1 把 `LazyChannelPart` 放到 `furniture.tsx` 的 `../api`，web Vite 原先只别名 `./api` → 打到 Tauri `invoke`。`vite.config.web.ts`：`find: /^(?:\.\.\/)+api$|^\.\/api$/`。**禁止** `/(?:^|\/)api$/`（Windows 会留下 `.` 前缀把路径拼坏）。
- 嵌套 `<details>`：内层思考过程受外层 fold 抢走 `open`。`src/detailsToggle.ts` 的 `applySelfDetailsToggle`；内层 `stopPropagation`。

## Key files

- `src/themes.css` — 文件末尾 `ui-demo.html 壳层对齐` 大段（必须在末尾才能压过硬编码）
- `src/App.tsx` — rail / header / composer / 三页签 / 常驻 goal bar / hook 顺序
- `src/theme.tsx` — `HeaderThemePop`
- `src/components/furniture.tsx` + `src/detailsToggle.ts`
- `vite.config.web.ts` — web `api` 别名覆盖 `../api`
- 测：`src/uiDemoParity.test.ts`、`src/appHooksOrder.test.ts`、`src/webApiAlias.test.ts`、`src/detailsToggle.test.ts`

## Locked product decisions

- 壳层以 `docs/ui-demo.html` 为准，不是旧 ohMyWorkPanel 胶囊头。
- Cyberpunk 主色跟 demo：`#00f0ff`（不再粉青城市图）。
- 斜杠仍只有 `/board /approve /wave`，不加 `/play /pause /advance /release`。
- 本机 Windows `:8082` ≠ Linux 灰度 `:8081` / 生产 `:8080`。Agent 不得伪造 `approve-prod-release` 或动生产 unit。

## Known pitfalls

1. PowerShell `Set-Content` 会把 UTF-8 中文写成 mojibake。改中文文件用编辑器工具。
2. web api 别名不要用 `/(?:^|\/)api$/`。
3. 新 hook 不得放在 `session === "login"` / `"checking"` / invite 早退之后（React #310）。
4. 硬刷新：`:8082` 有 SW；改 dist 后 **Ctrl+Shift+R**。验证时看 HTML 引用的 `index-*.js` 是否刚 build 的那份。
5. 气泡内部（懒加载思考/产物）和成员行「检测/移除」**仍未**做成 demo 的 parts/todo 行——下一刀。

## How to run / verify

- 前端：`npm test`（本机 PATH 无 `pnpm` 时）→ **79 passed / 23 files**（含上述 4 个新测）。
- `npm run build:web` → `dist/`；`:8082` 的 `OHMYWORKPANEL_WEB_DIST` 指向该 dist。
- 视觉：`http://127.0.0.1:8082/` Ctrl+Shift+R。CDP 曾测：`--lp-accent=#00f0ff`、`body::before` opacity 0、折叠轨宽 56、右栏三页签、思考过程点击后 `#root` 仍在。
- 登录 seed：`root` / `root`。数据：`D:\AI\ohMyWorkPanel\.local-panel\data`。

## Do not regress

- 登录页与主界面 hook 数量必须一致（`appHooksOrder.test.ts`）。
- `vite.config.web.ts` 必须同时匹配 `./api` 与 `../api`。
- 嵌套 details 不得无条件 `setOpen(event.currentTarget.open)`。
- 不要把城市 SVG 底图加回去；不要恢复胶囊 `view-toggle` 当主头。
- 气泡 `.m-actions`（复制/停止/重试）必须常驻，禁止 hover 才 `display`（会撑开列表抖动）。
- 不要把 👍👎 加回气泡；`message_feedback` API 可留，待有下游再接。

## Open follow-ups

- 气泡对齐 demo：parts / todo / tool-chip，而不是折叠 + 懒加载摘要。
- 成员行视觉（状态点、tag），弱化「检测/设管理/移除」工具条感。
- 登录页尚未按 demo 重做。
- 上 Linux 灰度仍走 `deploy-canary.sh`；晋生产需 root `approve-prod-release.sh && promote-canary.sh`。
