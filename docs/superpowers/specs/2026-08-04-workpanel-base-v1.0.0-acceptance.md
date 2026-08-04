# Workpanel BaseV1.0.0 — 现状梳理与验收标准

日期：2026-08-04  
路线图项：`Workpanel BaseV1.0.0发布`（`41142775-7693-49df-a059-85bf9d5186da`）  
本文件对应 checklist：`梳理现状与验收标准（Workpanel BaseV1.0.0发布）`

## 1. 产品定位（Base 范围）

LinlisWorkPanel BaseV1.0.0 指「可日常使用的多 Agent 协作工作台」基线，而非全部未来能力（如 A2A 协议另列为后续 Feature）。

基线能力包括：

| 域 | 能力 |
|---|---|
| 运行形态 | Web 双槽位：生产 `:8080` / 灰度 `:8081`，数据目录分离 |
| 协作 | 群聊、@Agent、任务运行态、会话/历史分页 |
| Agent | Cursor / Codex / Claude / OpenCode / OpenClaw / Chatbot 适配 |
| 项目 | 路线图 + Feature + checklist，编排串行派发 |
| 安全/ACL | JWT 登录、scoped 用户、群公告注入 |
| 体验 | 主题（含赛博/工业）、移动端/PWA、思考/中间产物懒加载 |

明确 **不在 BaseV1.0.0 必验收**：Agent 间 A2A 协议（现有 Feature「Agent间通过A2A协议交互」可并行推进，不阻塞 Base 发布门闩）。

## 2. 现状快照（2026-08-04）

### 2.1 代码与槽位

- Git：`master`，HEAD 约 `ca8c0ef`（含懒加载 parts、赛博/工业主题、Codex key 解析等）。
- 生产 / 灰度 **二进制 SHA 一致**：`f14bf7088e4fbeddc80faf299dfe8276ad30bd77caee1065f9e009122162c331`
  - prod `promotedAt`: `2026-08-04T04:06:22Z`
  - canary `deployedAt`: `2026-08-04T03:53:42Z`
- 数据：`/AI/LinlisWorkPanel/data`（prod）与 `data-canary`（灰度）分离 — 不得互相覆盖。
- 服务：`linlis-work-panel`、`linlis-work-panel-canary`、`linlis-codex-proxy` 均为 active（代理提供 Codex→`:18888` Responses shim）。

### 2.2 已落地的关键交付（相对 Base）

- 路线图编排：checklist 串行、`@` 派发、启停/取消。
- 聊天性能：thinking/artifact 列表剥离，点击后读库。
- Codex：DeepSeek 模型经本地 shim；systemd 下可从 `~/.codex/auth.json` 取 key。
- 主题：运行设置内选择；Cyberpunk / Industrial 背景图与动效。
- 群用户：ACL、气泡身份、`@用户` 不唤醒管理员 Agent。

### 2.3 未完成 / 风险（不阻塞「梳理」本身）

| 项 | 说明 |
|---|---|
| 工作区未提交 | `linlis-codex-proxy.service`、`deploy-canary.sh` / proxy 脚本改动尚在 working tree |
| A2A Feature | 仍为 in_progress，**不作为 BaseV1.0.0 门闩** |
| Checklist 后续步 | 「实现核心改动并跑通测试门禁」「灰度验证并记录结果」待执行 |
| 代理依赖 | Codex 依赖 `:18888`；进程挂掉会表现为 stream disconnected（已用 systemd Restart=always） |
| promote 历史 | 曾出现 `systemctl restart` 卡住；现 prod/canary SHA 已对齐，后续用 `timeout` + 冒烟 |

## 3. BaseV1.0.0 验收标准（Definition of Done）

下列全部勾选后，可将路线图项 `Workpanel BaseV1.0.0发布` 标为 **completed**。

### A. 质量门禁

- [x] `pnpm run test:gate` 在发布提交上 exit 0（Vitest + `cargo test --lib --no-default-features`）— 2026-08-04 Cursor Agent 跑通
- [x] 发布相关改动已 git commit（不含 `data/`、密钥、`.linlis/` 运行态）— Codex proxy systemd + 验收文档入库

### B. 灰度（`:8081` / `data-canary`）

- [ ] `deploy-canary.sh` 成功；`http://127.0.0.1:8081/` → 200
- [ ] root/root 登录；群 **LinlisWorkPanel** 可见
- [ ] 任选一 Agent（建议 Cursor）完成一轮 @ 提及并收到最终回复
- [ ] Codex（若启用）：`linlis-codex-proxy` active，一次成功 exec（无 `OPENAI_API_KEY` / `:18888` 断连）
- [ ] 路线图：能打开项目视图，看到本路线图项与 checklist

### C. 生产晋升（`:8080` / `data`）

- [ ] `promote-canary.sh`（或等价：仅复制 bin+dist，**永不**覆盖 `data`）成功
- [ ] `http://127.0.0.1:8080/` → 200；root 登录；**LinlisWorkPanel** 群与历史仍在
- [ ] ops `release-status`：prod/canary `binarySha256` 一致（或文档说明有意差异）
- [ ] 生产冒烟：发一条消息或刷新聊天无 5xx；主题可在「运行设置」切换

### D. 文档与交接

- [ ] 本验收文档已入库（本文件）
- [ ] 发布当日有简短 epitaph 或发布记录（槽位 SHA、是否 touch 生产 DB=否）
- [ ] 群内确认：BaseV1.0.0 门闩清单已勾完；A2A 等后续项不挡关闭本路线图项

## 4. 建议执行顺序（后续 checklist）

1. **实现核心改动并跑通测试门禁** — 收口未提交的 proxy/systemd 与任何 Base 缺口，跑 `test:gate` 并 commit  
2. **灰度验证并记录结果** — 按 §3.B / §3.C 勾选，把结果贴到经验库或 epitaph  
3. 将路线图项状态改为 `completed`

## 5. 本任务完成标准

「梳理现状与验收标准」本身在以下条件满足时视为完成：

1. 本文档已写入仓库路径 `docs/superpowers/specs/2026-08-04-workpanel-base-v1.0.0-acceptance.md`
2. 范围边界（含 A2A 非门闩）与验收清单 §3 可供下一 Agent 直接执行
3. 对应 Feature Task 在生产库标记 `done=true`
