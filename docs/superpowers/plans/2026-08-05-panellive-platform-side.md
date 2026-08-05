# WorkPanel 平台侧方案 — PanelLive Extension Host + A2A 控制面（v0.5）

> 状态：**已认领（OpenClaw/PM）** · 实现：Cursor Agent / Codex · 日期：2026-08-05
> 契约来源：`docs/panellive-platform-requirements.md`、`docs/roadmap.md → v0.5`、`docs/superpowers/specs/2026-08-05-panellive-mock-mvp-design.md`
> 发布规则：灰度 → docs → commit → 生产（群公告）

## 1. 现状盘点（2026-08-05 实测）

### ✅ 已有（`src-tauri/src/extensions.rs`，未跟踪、未接线）
- ExtensionManifest 解析（对齐 PanelLive `extension.manifest.json`：tabs / a2aSkills / runtime）
- `group_extensions` 表（group_id × extension_id × enabled）
- `set_extension_enabled` / `is_extension_enabled` / `list_group_extensions`
- 健康检查 `check_panellive_health`（原始 TCP HTTP GET，2GB 机器不引 reqwest）
- `set_panellive_enabled`：enable 前置 health 校验，失败拒绝 load；disable = unload
- 单测模块（cfg test）

### ❌ 缺口
| # | 缺口 | 说明 |
|---|---|---|
| G1 | web.rs 路由未接线 | 无 `GET/PUT /api/groups/{id}/extensions*` |
| G2 | 前端零实现 | api.ts 无扩展函数；App.tsx 无 Live 页签/开关 |
| G3 | A2A 控制面未做 | live.* skills 无事件入口、无转发、无载荷校验 |
| G4 | 文档未更新 | roadmap v0.5 未勾选；api-web.md 缺新路由 |

## 2. 任务拆分（实现清单）

### T1 后端 API 接线（web.rs + extensions.rs 已有能力）
- `GET /api/groups/{id}/extensions` → `list_group_extensions`（群成员可读）
- `PUT /api/groups/{id}/extensions/panellive` body `{ "enabled": bool }` → `set_panellive_enabled`（仅 admin）
- 错误语义：404 群不存在 · 403 非 admin · 409 PanelLive 未就绪（带 health 详情）
- `db.rs` 已有 `group_extensions` 表接入（ensure 幂等）

### T2 前端（api.ts / api-web.ts / api-tauri.ts / App.tsx）
- 新增 `getGroupExtensions(groupId)` / `setExtensionEnabled(groupId, id, enabled)`
- 页签注册：读 `extensions[].tabs`，`tab://live` 与「聊天/项目」平级渲染；`disabledWhenUnloaded` 且未启用 → 置灰
- 运行设置新增 **Live 开关**：开 → PUT enable（失败展示 health 详情）；关 → PUT disable + 页签置灰
- ⚠️ **Live 页签入口必须走平台代理，禁止 iframe 直连 `http://127.0.0.1:8790`**（web 模式下浏览器端 127.0.0.1 指向用户本机；且 HTTPS 下直连 http 会触发混合内容拦截——即群记忆里 ws:// 白屏同类坑）

### T3 平台代理 + A2A 控制面（web.rs）
- **UI 代理**：`GET /api/extensions/panellive/{*path}` → 反代 `127.0.0.1:8790/{path}`（同源，规避混合内容；Tauri/web 双模式可用）；iframe src = `/api/extensions/panellive/live.html`
- **事件入口**：`POST /api/extensions/panellive/events`（PanelLive → 平台）
  - 鉴权：`X-Panellive-Token` 头，与 `LINLIS_PANELLIVE_TOKEN` 环境变量比对（MVP 固定 token）
  - 载荷：JSON task envelope `{ taskId, skill, sessionId, payload, ts }`
  - skills 白名单：`live.transcribe.result` / `live.session.*`（其余拒绝）
  - **禁 PCM 校验**：payload 仅允许 `{ text, isFinal, lang? }` 等文本字段，白名单字段 + 大小上限（如 8KB）；出现非白名单字段/audio 类字段 → 400
  - 处理：`live.transcribe.result` → 广播 WS 事件（chat-event 通道新增 live 变体）→ 前端 Live 页展示；**不进群消息流**（MVP 不污染聊天记录，见对齐点 A1）
- **平台 → PanelLive**（server 侧直连 127.0.0.1:8790，复用 extensions.rs 的 TCP 客户端思路，补 POST 支持）：
  - `live.session.start` → `POST /v1/session/start`
  - `live.session.cancel` → `POST /v1/session/cancel`
  - `live.session.stop` → MVP 以 cancel 语义兜底（PanelLive 无 /v1/session/stop，见对齐点 A2）
  - `live.synthesize.request` → `POST /v1/tts/mock`，载荷仅 `{ text }`（禁 PCM）

### T4 文档（随 commit 一起）
- `docs/roadmap.md` v0.5 勾选完成项
- `docs/api-web.md` 补 3 个新路由 + 事件入口契约
- epitaph：`docs/epitaph/2026-08-05-panellive-platform-side.md`
- `AGENTS.md` 补一句：扩展页签入口须走平台代理（防 127.0.0.1 直连坑）

### T5 发布流程（遵守群公告）
1. `deploy-canary.sh` → 灰度 :8081
2. 灰度冒烟：`npm run smoke`（PanelLive 侧）+ 平台侧（开关 → 页签出现 → iframe 打开 /live.html → mock 说话出文本 + 音频）
3. `test:gate` 绿 → commit（含 T4 文档）
4. `promote-canary.sh`（timeout 防卡，历史经验）→ 生产 :8080 冒烟（§F 前端壳检查）

## 3. 验收标准（可测）

1. `GET /api/groups/{id}/extensions` 返回 manifest 派生状态（enabled/healthy/healthDetail/baseUrl/tabs/a2aSkills）
2. `PUT .../panellive {enabled:true}`：PanelLive 未启动 → 409 + 详情；已启动 → 200 且 healthy=true
3. 灰度开关 Live → 页签与「聊天/项目」平级出现；关闭 → 置灰；再次开启 → 恢复
4. iframe 经 `/api/extensions/panellive/live.html` 可交互（Mock STT 文本 + Mock TTS 音频），Console 无 mixed-content 报错
5. 伪造事件（无 token / 带 audio 字段）→ 401 / 400；合法 `live.transcribe.result` → WS 事件可达前端
6. `test:gate` 全绿；文档同步；生产冒烟通过

## 4. 风险与对策

| 风险 | 对策 |
|---|---|
| iframe 直连 127.0.0.1 指向用户本机 / HTTPS 混合内容 | T3 平台代理（同源）为硬性要求 |
| PanelLive 缺 `/v1/session/stop` | MVP cancel 兜底；对齐点 A2 推动 PanelLive 补端点 |
| `transcribe.result` 推送机制 PanelLive 侧未实现 | 平台先定契约（events 入口 + token），PanelLive 下迭代接入 |
| events 入口无鉴权 → 伪造转写注入 | token 校验 + skill 白名单 + payload 字段白名单 |
| 2GB 内存：PanelLive(node)+WorkPanel 并存 | enable 前置 health 校验防呆；监控 RSS，超限则提示 |
| `systemctl restart` 卡住（历史） | 晋升用 timeout；沿用 release-checklist 经验 |

## 5. 对齐点（需 root / PanelLive 方确认）

- **A1**：`live.transcribe.result` 是否进群消息流？建议：MVP 仅 WS 事件 + Live 页展示，不进群消息（避免污染聊天记录）；后续可让指定管理员 Agent 订阅后转述。
- **A2**：PanelLive 是否补 `POST /v1/session/stop`（与 start 对称）？MVP 平台侧先用 cancel 兜底。
- **A3**：Live 页签可见性：建议与聊天页签一致（群成员可见，开关仅 admin 可操作）。

---
*本文件为 PM 方案，实现与 commit 由 Cursor Agent 按 T1–T5 执行。*
