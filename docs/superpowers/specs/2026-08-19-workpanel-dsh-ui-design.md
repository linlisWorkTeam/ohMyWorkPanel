---
date: 2026-08-19
topic: workpanel-dsh-ui-design
status: approved-implementing
decider: user 2026-08-19「借鉴 DSH UI，先落文档再开 P0；本会话授权持续推进」
choice: P0 分片落地（token 两层化 → ui 基元 → 三栏 AppFrame → 硬编码清剿），每片过门禁
---

# 设计：WorkPanel × DeepSeek Harness UI 设计体系（P0 落地方案）

> 配套交互原型：`docs/ui-demo.html`（单文件高保真，可直接浏览器打开对照）。
> 取代（absorb）草稿 [`2026-08-16-dsh-ui-language-workpanel.md`](2026-08-16-dsh-ui-language-workpanel.md) 的 UI 侧方案；本文为 UI 侧 SSOT。

## 0. 结论先行

**不 rewrite、不搬 dsh 代码，按 dsh 的「设计语言」改造现有前端。** 现况已 90% token 化（`themes.css` 中 `--bg-* / --text-* / --accent-* / --border-*` 正在被消费），真正的差距是：

1. 缺「静态色板 → 语义别名」的**正式两层模型**与层级纪律（组件里仍残留大量 `#hex`）；
2. 缺 `color-scheme` 与 `meta theme-color` 跟随；
3. 缺 dsh 式**三栏几何壳**：拖拽调宽、左栏 56px 控制轨、右栏可关、窄屏让步；
4. 组件层缺**基元目录**，App.tsx 被逼成 2032 行 monolith。

P0 分 4 片，每片独立可灰度、可回滚，全程不破坏 `tauri::command` 签名与 Tauri/Web 双模式。

---

## 1. 背景与目标

dsh Web UI（`apps/web` + `packages/client/ui-*`）已是成熟的三栏面板系统：`ui-layout`（拖拽/折叠/让步）、`ui-theme`（双层 token）、`ui-sidebar`、`ui-conversation`、`ui-goal`、`ui-trajectory`、`ui-tool`、`ui-jobs`、`ui-message-feedback`、composer 家族。WorkPanel 是其业务孪生（多群多 Agent 调度 + 群聊 + 审批 + 版本/Wave）。

**目标**：借用 dsh 的**壳层交互 + 主题纪律 + 组件分层**，不改 WorkPanel 的业务模型（多群群聊 ≠ 单会话）、不丢 6 套皮肤（Cyberpunk/Atlas/Industrial/Forge/Moss/Noir）。

**非目标**：搬运 dsh React 代码（栈/依赖不同）；引入组件库或 Tailwind；改 IPC / DB schema。

---

## 2. 现状审计（数据）

| 文件 | 规模 | 说明 |
|---|---|---|
| `src/App.tsx` | 2032 行 / 93KB | monolith：`App()` 内 ~30 个 useState + 全部布局/渲染 |
| `src/styles.css` | 522 行（密集单行） | 大量硬编码 `#hex`（组件旁路主题） |
| `src/themes.css` | 935 行 | 6 主题 token + 组件覆盖 + 断点；**token 基线已存在** |
| `src/theme.tsx` | 195 行 | 6 主题元数据；`data-theme` 挂 `<html>`；默认 `cyberpunk`；`localStorage` 持久化 |
| `src/api-tauri.ts` / `api-web.ts` | — | Tauri invoke / Web fetch 双实现，**不动** |
| `src/components/` | 不存在 | 需新建基元目录 |

**现有 token（= 别名层雏形）**：`--font-display/--font-body`、`--bg-app/--bg-app-glow/--bg-app-image/--bg-sidebar/--bg-chat/--bg-surface/--bg-composer/--bg-member/--bg-bubble/--bg-bubble-own/--bg-modal/--bg-auth-card`、`--text/--text-soft/--text-faint/--text-on-sidebar/--text-on-sidebar-soft`、`--border/--border-soft`、`--accent/--accent-strong/--accent-contrast`、`--danger`、`--shadow`、`--brand-mark-bg`、`--selected`、`--hover-sidebar`、`--ring`。
缺失：`--success/--warn` 语义、`color-scheme`、静态色板层、组件间 hover/active 语义统一。

**硬编码残留（styles.css 需清剿，抽样）**：`.status.running{background:#e7f0ff}`、`.log-info{...}`、`.roadmap-*`、`.pm-*`、`.feature-card`、`.version-*`、`.hold-talk-button`、`.jump-bottom-btn` 等仍以 `#hex` 书写。

**断点现状（已是让步雏形）**：`@media (max-width:1080px)` 左栏/右栏变 Drawer；`768px` 调字号。缺「右先收→左轨化」的中间档。

---

## 3. 借鉴 dsh 的要点（映射）

| dsh 能力 | WorkPanel 对应 | 借鉴动作 |
|---|---|---|
| `ui-theme` 双层 token（static 色板 + alias 别名 + theme 选择器所有权） | 已有单层语义 token | 正式化两层：现有 `--bg-*` 等升级为**别名层**；新增**静态色板层**文档化；组件禁写色值 |
| `ui-theme`：`color-scheme` + `meta theme-color` 跟随背景 | 未设 | P0.1 补齐 |
| `ui-layout`：ResizablePane 拖拽 / rail 折叠 56px / 让步 concession | 固定三栏 + 布尔收放 | P0.3 实现几何壳（纯 CSS 变量 + App.tsx 少量 JSX） |
| `ui-layout`：几何临时、业务进存储 | 面板收放内存态 | 几何只进 `localStorage`，不进 DB |
| `ui-primitives` / `ui-slots` 组件分层 | 无基元目录 | P0.2 建 `src/components/ui/*`，AppFrame 先消费 |
| `web-styling.md`：组件 CSS 只许语义 token、主题选择器归主题所有 | 部分违反 | P0.4 清剿 + 规则写进本文/AGENTS |

---

## 4. 目标设计体系（两层模型）

```text
┌─ 静态色板 static ──┐   每个主题定义自己的色阶（可以完全不同——6 套皮肤就是 6 套色板）
│  e.g. cyberpunk:   │   --lp-static-bg0/1/2, --lp-static-pink/cyan ...
└────────┬──────────┘
         ▼ 映射（每个主题各自的「色板→语义」这一张映射表）
┌─ 语义别名 alias ──┐   全组件只消费这一层——
│  --lp-bg-app      │   bg / surface / panel / elevated / hover / active
│  --lp-text-*      │   border-l1/l2 / accent / success / warn / error
│  --lp-accent-*    │   sidebar-text / ring / focus
└────────┬──────────┘
         ▼ 兼容既有
┌─ 现有 token 兼容层 ─┐  --bg-app/--text/--accent/--border 等继续存在（别名别名），
│  （双写：--lp-* =  )│  新组件用 --lp-*，旧组件过渡期继续用旧名，最后指向同一语义
└───────────────────┘
```

**落地方案（P0.1，纯增量）**：
- 在 `themes.css` 每个主题块内补上：`color-scheme: light|dark`（atlas/moss=light，其余=dark）+ 语义 token `--lp-*`（由既有 token 组合而成，无需重写既有值 → 零视觉回归）+ `--success/--warn`。
- `theme.tsx`：主题切换时同步 `<meta name="theme-color">`。
- 新增 token 仅由**新组件**消费；既有组件逐步迁移（P0.4）。

---

## 5. 语义 token 清单（P0.1 新增）

以下 `--lp-*` 在 `:root,[data-theme=atlas]` 基块定义（组合既有 token），暗色主题若无反差需求可不重复。

| token | 定义（参考） | 用途 |
|---|---|---|
| `--lp-bg-app` | `var(--bg-app)` | 页面底色 |
| `--lp-bg-sidebar` | `var(--bg-sidebar-solid)` | 左栏 |
| `--lp-bg-panel` | `var(--bg-chat)` | 中栏会话 |
| `--lp-bg-elevated` | `var(--bg-surface)` | 卡片/浮层 |
| `--lp-bg-overlay` | `rgba(0,0,0,.5)` | 遮罩 |
| `--lp-bg-hover` | `color-mix(in srgb, var(--text) 8%, transparent)` | 悬停 |
| `--lp-bg-active` | `color-mix(in srgb, var(--accent) 16%, transparent)` | 选中 |
| `--lp-border-l1` | `var(--border-soft)` | 细边 |
| `--lp-border-l2` | `var(--border)` | 强调边 |
| `--lp-text-primary` | `var(--text)` | 主文字 |
| `--lp-text-secondary` | `var(--text-soft)` | 次文字 |
| `--lp-text-tertiary` | `var(--text-faint)` | 弱文字 |
| `--lp-text-on-sidebar` | `var(--text-on-sidebar)` | 左栏文字 |
| `--lp-accent` / `--lp-accent-strong` | `var(--accent)` / `var(--accent-strong)` | 品牌强调 |
| `--lp-success` / `--lp-warn` / `--lp-error` | 新增语义色 | 状态/队列/审批 |
| `--lp-ring` | `var(--ring)` | 焦点环 |

**使用规则**（沿用 dsh `web-styling.md`）：功能组件 CSS **禁止**出现 `#hex`/`rgb()` 字面量；一律用 `--lp-*`/既有 token；主题选择器（`[data-theme=...] .foo`）只允许出现在 `themes.css`。

---

## 6. 三栏 AppFrame 几何规范（P0.3）

```text
┌────────────┬─┬───────────────┬─┬──────────┐
│ 左栏 Sidebar││ 中栏 群聊=工作区 ││ 右栏 成员 │
│ --left-w    ││ minmax(460px,1fr) ││ --right-w│
└────────────┴─┴───────────────┴─┴──────────┘
        divL        拖拽分界     divR
```

| 参数 | 值 |
|---|---|
| 默认宽 | 左 `248px`（沿用现 248→236 对齐当前渲染）、右 `310px` |
| 拖拽约束 | 左 160–340px；右 240–420px；中栏最小 `460px` |
| 左栏折叠 | `--rail-w: 56px` 控制轨（展开/折叠切 `data-left="open|rail"`），快捷键提示 |
| 右栏关闭 | `data-right="open|closed"`；沿用 `showMembers` 语义（历史行为保留） |
| 让步顺序 | 容器宽 `≤1100px` 右栏先关 → `≤860px` 左栏轨化 → `≤640px` 保持现 Drawer 式 |
| 持久化 | 宽/折叠 → `localStorage`（键 `lp.frame.*`），几何不进 DB |
| 移动端 | 完全复用现有 `@media (max-width:1080px)` Drawer 逻辑，**不得破坏** |

实现要点：
- `.app-shell` 改 CSS 变量列：`grid-template-columns: var(--left-w) 8px minmax(460px,1fr) 8px var(--right-w)`；
- 两个 `divider` 由 App.tsx 渲染（纯新增 JSX，夹在三个既有 section 之间），pointer 拖拽改 CSS 变量；
- 不拆 App.tsx 现有 children；仅包一层结构与两个分隔条。

---

## 7. 组件基元目录（P0.2）

`src/components/ui/`，纯新增、无副作用：

```
ui/
  IconButton.tsx    # 兼容既有 .icon-button 语义（className 透传）
  Button.tsx        # variant: primary|ghost|quiet|danger
  Modal.tsx         # 复用 .modal-backdrop/.modal，props: title|onClose|children
  Spinner.tsx       # loading 态
  Tooltip.tsx       # [data-tip] 语义的 React 封装（可选，P1）
  DragHandle.tsx    # 三栏分界条（pointer 拖拽，P0.3 使用）
  Frame.tsx         # AppFrame 壳（P0.3 使用）：三栏 + 让步 + 持久化
```

原则：基元**不内置业务**；业务组件（群列表/气泡/成员行）P1 再逐步从 App.tsx 抽出。

---

## 8. 路线图（分片交付）

| 片 | 内容 | 门禁/验收 |
|---|---|---|
| **P0.1** token 两层化 | `themes.css` 补 `color-scheme` + `--lp-*` + `--success/--warn`；`theme.tsx` 同步 `theme-color` | 零视觉回归；`pnpm test` / `pnpm build` |
| **P0.2** ui 基元 | `src/components/ui/*`（含 Frame/DragHandle） | `tsc -b` 通过；双模式构建 |
| **P0.3** 三栏壳 | `.app-shell` 变量化 + App.tsx 包壳 + 拖拽/折叠/让步 + localStorage | 桌面/Web 双跑；拖拽调宽、左轨、右关、窄窗让步可手测 |
| **P0.4** 硬编码清剿 | `styles.css` 残留 `#hex` → `--lp-*`/既有 token（抽样清单见 §2） | 主题切换下各面板不再有死色 |
| **P1**（后续） | 气泡/成员行/队列卡片组件化；composer 草稿+Tab；消息操作/反馈 | 走灰度 → 批准 → promote |
| **P2**（后续） | goal/Wave 上移 goal bar；决策卡；run 轨迹视图；审批内联 | 同上 |

每片上 `dev:web` 冒烟 + `pnpm run test:gate`，经灰度 `:8081` 验证后 promote。

---

## 9. 风险与护栏

- **白屏防护**：所有本体改动先 `tsc -b` 再 `vite build`；发版按 `docs/release-checklist.md` §F 冒烟；SW 对 `/assets/` network-first（沿用教训文档）。
- **双模式**：`vite.config.ts`（Tauri）与 `vite.config.web.ts`（stub）都要过构建；不 import 平台特有 API。
- **IPC/DB**：本文全部改动仅前端样式/结构；`tauri::command`、`types.ts` union、SQLite schema 一律不动。
- **实时链路**：几何改动不碰 WS/wss 逻辑。
- **可回滚**：P0 每片是独立 commit；App.tsx JSX 变更保持在最小增量。

---

## 10. 验收标准（P0 合集）

1. `pnpm test`（Vitest）与 `pnpm run test:gate` 全绿；`pnpm build` 与 `pnpm build:web` 成功。
2. 桌面（Tauri）与 Web 双模式启动正常，白屏冒烟通过。
3. 桌面宽屏下：三栏可拖拽调宽且满足 §6 约束；左栏可折叠为 56px 控制轨；右栏可关；窄窗右栏先收、左栏再轨化。
4. 6 套主题切换后，新组件与既有界面同语义用色（无残留死色）。
5. `localStorage` 记住面板几何；刷新不丢；业务状态不受影响。

---

## 11. 不做 / 边界

- 不搬 dsh React 源码；不引入组件库/Tailwind（对齐 dsh 自身规则）。
- 不把「单会话」当群聊替代；chat 群聊模型不变。
- 不动 `theme.tsx` 的主题列表（6 套）与默认值。
- UI-P0 不触碰审批/决策卡/轨迹（那是 P2 + 治理层）；本片只做壳与地基。

---

## 12. 相关文档索引

| 文档 | 用途 |
|---|---|
| [`2026-08-16-dsh-ui-language-workpanel.md`](2026-08-16-dsh-ui-language-workpanel.md) | 旧草稿（本文吸收其 UI 侧） |
| [`docs/ui-demo.html`](../../ui-demo.html) | 交互原型（对照验收） |
| [`../version-pipeline.md`](../../version-pipeline.md) | 版本/轨道 SSOT |
| [`2026-08-16-dsh-self-bootstrap-runtime.md`](2026-08-16-dsh-self-bootstrap-runtime.md) | 轨道 G 总设计（治理层） |
