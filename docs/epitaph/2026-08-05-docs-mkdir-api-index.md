---
date: 2026-08-05
topic: docs-mkdir-api-index
branch: master
status: active
---

# Epitaph: 文档增量 + 路径 mkdir 说明

## Built

- 不重写全书；增量更新：
  - `AGENTS.md`：路径支持 mkdir；群公告发布流程（灰度测 → commit → promote）
  - `docs/api-web.md`：Web 路由薄索引
  - `README.md`：Web/双槽位/`test:gate`
  - epitaph v1.6 / BaseV1.0.0：路径与 Codex sidecar 表述纠偏
- 功能侧（此前已落地）：`POST /api/fs/mkdir` + `ServerPathPicker` 新建文件夹

## Do not regress

- 文档增量优先，禁止无必要全量刷新三套文档
- 生产 DB 永不被 promote 覆盖
- `/` 下禁止 mkdir；名称禁穿越

## Verify

```bash
# 灰度冒烟 mkdir
curl -sS -X POST http://127.0.0.1:8081/api/fs/mkdir ...
pnpm run test:gate
```
