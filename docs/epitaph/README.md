# Epitaph Index

Active handoff notes for future agents.

## Active

| Date | Topic | Status |
|------|-------|--------|
| 2026-08-05 | [文档增量 + 路径 mkdir / API 索引](./2026-08-05-docs-mkdir-api-index.md) | active |
| 2026-08-04 | [v1.0.0 Base 灰度/生产验证](./2026-08-04-v1.0.0-base-release-verify.md) | active |
| 2026-08-03 | [v1.7 phases / chatbot / sandbox / keep-alive](./2026-08-03-v1.7-chatbot-phases.md) | active |
| 2026-08-03 | [v1.6 工作流视图 / 服务端路径 / 群公告 / Ops](./2026-08-03-v1.6-workflow-pm.md) | active |
| 2026-08-03 | [v1.5 自动化测试策略 + canary 门禁](./2026-08-03-v1.5-test-gate.md) | active |
| 2026-08-01 | [v1.4 流式分区 + 同 Agent 串行 + Cursor Session](./2026-08-01-v1.4-streaming-session.md) | active |
| 2026-08-01 | [v1.3 生产/灰度双槽位 + 自迭代](./2026-08-01-v1.3-prod-canary.md) | active |
| 2026-08-01 | [v1.2 Experience/Logs + Web 服务可启动](./2026-08-01-v1.2-experience-logs-startup.md) | active |
| 2026-07-19 | [v1.0–v1.1 PM Frontend UI + API Layer](./2026-07-19-v1.0-pm-frontend.md) | active |

## Archive

| Date | Topic | Status |
|------|-------|--------|
| 2026-07-19 | [v0.4 Tauri + Web Dual-Mode](./2026-07-19-v0.4-dual-mode.md) | archived |
| 2026-07-19 | v0.1-v0.3 项目初始化/OCR/技能文档 | archived |

See individual files for details. Newest first.

**接手优先读 v1.7 + v1.5 + v1.3**：phases/chatbot/沙箱/保活见 v1.7；测试门禁见 v1.5；生产 `:8080` 冻结；改代码只部署灰度 `:8081`（含门禁），测通后 `promote-canary.sh`。2GB 主机勿对整门禁 `ulimit -v`，勿再引入 `reqwest`。
