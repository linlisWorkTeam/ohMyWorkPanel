---
date: 2026-08-20
topic: dsh-ui-design-p0-p2
branch: master
status: active
---

# Epitaph: DSH 设计语言落地 ohMyWorkPanel 前端 —— P0/P1/P2 + 后端四项（已全部完成并推 origin）

> 目标「全部按计划做完」已完成（goal 已 complete，round 10/256）。
> 接手本文件者先读 Spec：`docs/superpowers/specs/2026-08-19-ohmyworkpanel-dsh-ui-design.md`（UI 侧 SSOT）与 `...-ui-backend-gated-plan.md`（状态已标 implemented-2026-08-20）。
> 交互原型：`docs/ui-demo.html`（单文件高保真，浏览器直接开）。

## 这一轮做了什么（11 个 commit，`27f84bd` → `4a54d69`，已推 origin；工作树干净）

**P0 设计地基**
- `--lp-*` 双层 token：6 套皮肤（Cyberpunk/Atlas/Industrial/Forge/Moss/Noir）=「色板→语义别名」，组件 CSS 零色值；每主题补 `color-scheme` + `--success/--warn/--error`
- 三栏 AppFrame 壳：CSS 变量列 + 两个 `<Divider>`（拖拽 160–340 / 240–420）+ 左栏 56px 控制轨 + 右栏可关（数据右 `showMembers`）+ 窄屏让步（`ResizeObserver`，仅穿越阈值时自动收敛）；几何只存 `localStorage`
- `src/components/ui/{Divider,useAppFrame,index}.tsx`
- `theme.tsx` 换主题同步 `meta theme-color`

**P1 家具**
- 气泡悬停条：复制(真)/👍/👎(真，round8)/停止/重试（并入，删旧 run-actions）；composer 草稿按群存 localStorage、切群恢复、发送清空；`@` 工具行；思考/产物卡片化；成员行+队列列表卡片化；空态欢迎页 `EmptyHome`（无群不再永远转圈）；**组件化抽取**：`src/components/furniture.tsx`（10 个家具）+ `uiShared.ts`（PHASE_LABEL/time/dayLabel/readError），App.tsx 2217→1740 行

**P2**
- goal bar（项目群 chat 头下 Wave 常驻条/进度，点击进版本页）；右栏「队列」页签（执行中/排队/待审批卡 + 轨迹展开）；桌面右栏开关（`members-toggle`，修了"关后无法重开"老缺口）；⌘/Ctrl+1 左轨、⌘/Ctrl+2 成员、Esc 关浮层；PWA manifest/manifest 主题对齐；`index.html` 默认 `data-theme="cyberpunk"` + `#070012`（防首屏闪白）

**后端四项（均为增量命令/表，未破坏既有 schema/签名）**
| 项 | 命令 / 路由 / 表 | 位置 |
|---|---|---|
| run 审批内联 | `set_run_review` + `/api/runs/{id}/review` | `commands.rs`/`web.rs`/`db.rs::set_run_review` |
| 消息反馈 👍/👎 | `message_feedback` 表 + `vote_message`/`get_message_feedback` + `/api/messages/{id}/vote|feedback` | db/commands/web |
| run 轨迹 | `run_phase_log` 表 + `get_run_phases` + `/api/runs/{id}/phases`；在 `db::set_run_phase`（唯一汇聚点）内统一记录 | db/commands/web |
| 斜杠命令→决策卡 | `workflow::try_slash_command`：`/board /approve /wave`（项目群+用户成员，纯 conn 不 spawn run，未知命令回落普通消息，群内回显）| workflow.rs + send_message（commands/web 双入口）+ composer `/` 菜单 |

## 验证
- `cargo test --no-default-features --lib`：**122 passed**（新增 4 单测：set_run_review / message_feedback / run_phase_log / slash_command）
- Vitest：**72/72**；`pnpm build`（tsc）与 `pnpm build:web` exit 0
- 本地灰度 `:8082` 已换到最新前后端（见下），DB 完好

## 本地灰度环境（这台 Windows 机）
- `:8082` = 本机 `src-tauri\target\release\ohmyworkpanel-server.exe`（Windows release），数据 `D:\AI\ohMyWorkPanel\.local-panel\data`
- 重启配方（3 个环境变量 + cwd）：
  `$env:OHMYWORKPANEL_PORT=8082; $env:OHMYWORKPANEL_DATA_DIR=...\.local-panel\data; $env:OHMYWORKPANEL_WEB_DIST=...\dist;` cwd=`src-tauri`，`Start-Process` 该 exe，日志 `.local-panel\logs\gray-server.log`
- 旧 exe 备份：`.local-panel\bin\server-{rX}/`
- 注意：这是**本机 Windows 灰度**；仓库发布脚本 `deploy-canary.sh/promote-canary.sh` 面向 Linux/systemd（`/AI/...`+`/opt/ohmyworkpanel`），**本机无法执行**；真正的灰度 :8081/生产 :8080 在服务器上，走用户发布仪式。

## 视觉评审指引（给有视觉能力的模型/用户）
1. 打开 `http://127.0.0.1:8082/`，**Ctrl+Shift+R 硬刷新**（避开旧 SW/缓存）。
2. 逐项看：三栏拖拽分界、左栏 `◀◀`→56px 控制轨、头部 `▤成员` 开关右栏、`Ctrl+⌘1/2`、Esc；悬停气泡（复制/赞美/停止/重试/朗读）；换 6 套主题观察状态/日志/徽标/成员/队列语义色跟随；项目群 goal bar + 队列卡轨迹展开；输入 `/` 看命令菜单、发 `/board` 看群内回显；PWA manifest（暗色）。

## 坑（必读，别重踩）
1. **PowerShell `Get-Content/Set-Content` 会把 UTF-8 中文写成 mojibake**（本会话已毁过一次 furniture.tsx，`git checkout` 恢复 + edit 工具重做）。改中文文件一律用 read/edit/write 工具，或 `[System.IO.File]::ReadAllText(path, UTF8)` / `WriteAllText(..., new UTF8Encoding(false))`。
2. **Rust：往 `mod tests {` 前面插顶层函数，会把悬在 `mod tests` 上的 `#[cfg(test)]` 吸到新函数头** → 该函数只在测试构建存在、release 找不到（round 7 与 round 10 各踩一次）。插完检查 `#[cfg(test)]` 归属是否仍在 `mod tests` 前。
3. `set_run_phase` 是 run 阶段唯一汇聚点（已接 run_phase_log）；**勿在其它地方另写阶段日志**。
4. 本地后端换班：**先停进程再 `cargo build --release`**（运行中的 exe 被锁，覆盖会 os error 5）；换完用配方重启并 §F 冒烟。
5. 斜杠命令边界：只做纯 conn 的 `/board /approve /wave`；**`/play /pause /advance /release` 会 spawn Agent run / 触发布晋升仪式，勿擅自加**（决策卡/推进部分有意留治理层）。
6. `APP` git：**从不 stash**；每轮验证后直接 commit+push（网络不稳，push 失败要重试几次）。

## 之后
- **壳层已按 `ui-demo.html` 落地**（见 [`2026-08-20-ui-demo-chrome-parity.md`](./2026-08-20-ui-demo-chrome-parity.md)）：独立 56px 轨、chip 头、composer 卡片、三页签、常驻 WAVE、去掉城市底图。气泡内部 / 成员行仍待对齐。
- 上生产仍按老流程（服务器 `git pull` → `deploy-canary.sh` → §F 冒烟 → `approve-prod-release.sh && promote-canary.sh`，需 root 批准）。
