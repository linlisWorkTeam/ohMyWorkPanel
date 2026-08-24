# Spec：主题一致性治理（临时任务，交付给 Codex）

> 状态：待执行 · 发起人：main 会话 · 执行人：Codex
> 这是一次性临时任务：**只改代码与测试，不许发布**（详见「工作方式与红线」）。
> 全部所需背景/数据/验收在本文件内，无需向发起人追问即可开工；有歧义列入「待确认清单」统一回报。

## 0. 工作方式与红线（先读，最高优先级）

1. **只改工作树，不发布**：禁止 `git commit` / `git push` / 打 tag / 构建安装包（`pnpm tauri build`、`build-desktop.ps1`） / 升版本号 / 部署灰度或生产 / 发更新清单。发起人会统一合批与发布。
2. 开工前 `git status`：仓库可能有他人未提交改动（`fix/user-acceptance` 分支工作树），**不得 git checkout/stash/reset 他人改动**。
3. 允许改动的文件：`src/**`（css/tsx/ts）、`scripts/check-color-purity.mjs`（新建）、`package.json`（仅加 `"check:colors"` 脚本）、`src/themeTokens.test.ts`（新建）。**禁止**动 `src-tauri/**`、`docs/release-*.md`、版本号任何文件。
4. 业务逻辑与布局**不许改**，只做"样式来源统一/换 token"。
5. 遇到需要拍板的（观感取舍、token 语义歧义、文件范围疑问）：**不当场自裁**，列入交付报告「待确认清单」，由发起人收口。
6. 交付形式（一次性汇报，简短）：改动文件清单 + 四组任务各自完成情况 + 两张证据截图（六套主题任选：设置弹窗、右键菜单）+ 门禁命令输出 + 待确认清单。**无需**长叙述。

## 1. 背景（诊断数据，已由发起人完成）

- 主题体系：`src/themes.css` 六套主题（cyberpunk 默认…），语义 token `--lp-*` 214 个定义；另有旧变量 `--elev/--ink/--line/--text/--border/--accent/--danger` 与 `--lp-*` **双体系并存**，主题块覆盖不齐 → 换主题时部分组件不跟随。
- 硬编码颜色：`src/**/*.css` 中不引用 var 的 hex/rgba **642 行**（styles.css 的按钮/表单/弹窗/边框为主）；`src/**/*.tsx|ts` 字面量 37 处（theme.tsx 22 处为主题定义=合法；**PmPanel.tsx 8 处为真硬编码**）。
- 新壳 token：`src/shell/tokens.css`；新壳组件在 `src/shell/`；旧组件在 `src/components/`（furniture.tsx 等）。
- 现有原子件雏形：`src/components/ContextActionMenu.tsx`、`src/components/uiShared.ts`、`src/components/ui/{Divider,useAppFrame,index}.ts`；`Modal` 目前在 App.tsx 内联（`<section className="modal">`，样式在 styles.css 单行长规则）。

## 2. 目标

新页面/小窗/弹层在六套主题下自动一致，且**硬编码颜色无法再进入代码**（自动卡点）。

## 3. 任务 A：Token 唯一真源（SSOT）

1. `themes.css` 顶部确立唯一语义 token 清单（约 24 个，命名与既有 `--lp-*` 对齐）：覆盖 bg / 浮层 / 文本 / 强调 / 边框 / 危险 / 成功 / 警告 / 悬停 / 禁用 / 阴影 / 圆角 等。
2. **六套主题块全部只覆盖同一份清单**（每个 token 都必须定义；允许引用 `var(--acc)` 等既有值）。
3. 旧变量（`--elev/--ink/--line/--text/--border/--accent/--danger`）改为别名映射（如 `--elev: var(--lp-bg-elev)`），保留兼容，不删。
4. 新增 `src/themeTokens.test.ts`（vitest）：解析 themes.css，断言六套主题块定义集合完全一致（漏定义=红灯）。
5. 各主题**观感不变**（只统一来源）。

## 4. 任务 B：小窗原子组件层

在 `src/components/ui/` 补齐（全部只用 token，禁止字面量）：
- `Modal`：从 App.tsx 内联版抽出正式组件（backdrop/弹体/标题/关闭按钮；props: title/onClose/children）并替换 App.tsx 使用处
- `FormField`（label+hint+required 态）、`ButtonGroup`（主/次/危险三态，兼容现有 primary-wide/pm-btn sm 语义）、`Badge`、`Toast`、`ContextMenu`（现有 ctx-menu 样式迁 token）

## 5. 任务 C：颜色纯度扫描器 + 门禁接入

1. 新建 `scripts/check-color-purity.mjs`（node，零依赖）：扫 `src/**/*.{css,tsx,ts}`（跳过 `node_modules`/`*.test.*`/`stubs/`）；出现 `#hex`/`rgb(`/`rgba(`/`hsl(` 且不在主题定义区即报错；白名单：`theme.tsx`、`themes.css` 主题定义块、`shell/tokens.css`；违规退出码非 0。
2. 存量迁移：按批次替换硬编码为 token（优先 styles.css 的 modal 系/表单控件/按钮 → furniture.tsx 内联 → shell 细节），每处理完一个文件该文件即过扫描器。
3. 接入：`package.json` 加 `"check:colors": "node scripts/check-color-purity.mjs"`；**不要**动 build-desktop.ps1（发起人合批时统一接入门禁）。

## 6. 任务 D：视觉收尾（观察判断归你）

六套主题下弹窗/右键菜单/列表行/禁用态/错误提示的观感微调（只改 token 值或组件样式，不改布局）；提交前每套主题各截一张关键页作为证据。

## 7. 验收命令（全部必须通过后交付）

```bash
pnpm exec vitest run --pool=forks --maxWorkers=1   # 含 themeTokens.test.ts
node scripts/check-color-purity.mjs                # 0 违规
pnpm build                                          # tsc + vite
pnpm exec vitest run src/appHooksOrder.test.ts      # App.tsx hooks 顺序防线不得破坏
# 下列为只读确认，不得修改任何 src-tauri 文件：
cd src-tauri && cargo test --no-default-features --lib && cargo check --lib
```

## 8. 交付报告模板

```text
改动文件：<清单>
A token SSOT：<完成/部分，差距一句话>
B 原子组件：<Modal/FormField/ButtonGroup/Badge/Toast/ContextMenu 各自状态>
C 扫描器：<结果 0 违规；存量已迁移行数>
D 视觉：<截图 2 张路径>
门禁：<四行命令结果>
待确认清单：<无 / 条目>
```