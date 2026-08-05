# Roadmap

## v0.1 — MVP（已完成）

- [x] 群聊 CRUD、绑定本地工作目录
- [x] 成员管理：用户 / Agent、管理员设置
- [x] `@` 提及触发 Agent 任务
- [x] 任务调度：排队、并发控制、取消、重试
- [x] 模拟 Agent（mock）本地流式回复
- [x] Codex CLI 适配器
- [x] Claude Code 适配器

## v0.2 — 本轮交付

- [x] 仓库正规化：git init、.gitignore、README
- [x] 后端模块拆分：models / db / commands / scheduler / adapters
- [x] AdapterKind 枚举分发
- [x] OpenCode 适配器
- [x] Cursor CLI 适配器（agent → cursor-agent 回退）
- [x] 适配器参数单测 + 流解析测试
- [x] 尽力 smoke 脚本（非阻塞）
- [x] AGENTS.md 项目约定
- [x] 基础图标资源

## v0.3 — 计划

- [ ] 后端 `run_agent` / `append_delta` 单测覆盖
- [ ] 前端 `App.tsx` 组件拆分（MessageBubble / MemberPanel 独立文件）
- [ ] 适配器执行超时日志与诊断信息
- [ ] 适配器 stderr 写入群聊错误气泡（当前仅落日志）
- [ ] 成员运行时状态面板：实时显示各 Agent 运行情况
- [x] OCR 图片文字识别（Tesseract 集成）
- [x] 应用图标更新（91KB 自定义图标）
- [x] Windows 适配器路径修复（npm 全局模块 / .cmd/.bat/.ps1 支持）
- [x] dialog 权限修复（capabilities 配置）
- [ ] 群聊模板：一键创建常见协作群

## v0.4+

- [ ] 本地 Agent 命令沙箱 / 白名单
- [ ] 任务历史回溯与重放
- [ ] 跨群 Agent 引用（Agent 可同时加入多群）
- [ ] 可选云端备份
- [ ] Agent 角色社区模板

## v0.5 — Extension Host + PanelLive 对接（平台侧）

> 来源：PanelLive Mock MVP（`/AI/WorkPanelLive`）。详细契约见 [`docs/panellive-platform-requirements.md`](panellive-platform-requirements.md)。

- [x] **Extension Host**：运行设置支持 PanelLive load/unload；清单读 `/AI/WorkPanelLive/extension.manifest.json`
- [x] **Live 开关 + 页签**：与「聊天/项目」平级；开启后 iframe 至 PanelLive entry；关闭置灰
- [x] **A2A 控制面 skills**：`POST /api/a2a/dispatch` 支持 `live.session.*` / `live.transcribe.result` / `live.synthesize.request`（**禁 PCM**）
- [x] **边界**：云 STT/TTS 由 PanelLive 直连；平台不转音频
- [x] **API**：`GET /api/groups/{id}/extensions`、`PUT .../extensions/panellive`
