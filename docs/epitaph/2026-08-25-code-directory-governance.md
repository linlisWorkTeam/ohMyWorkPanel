---
date: 2026-08-25
topic: code-directory-governance
status: active
---

# 代码目录治理交接

## 本轮目标

让目录直接表达产品领域，不再依赖维护者先阅读巨型入口文件建立心智模型。本轮只移动代码、修复 import/module 路径并固化规范，不修改 Tauri IPC、Web API、SQLite schema 或用户行为。

## 已完成

- 前端业务文件从 `src/` 平铺层迁入 accounts/chat/groups/members/agents/workflow/extensions/observability。
- Rust auth/presence、Agent adapters/config/catalog、运行日志/指标/Ops/保活/drain 分别迁入 accounts/agents/operations。
- Rust crate 根保留旧模块路径的兼容 re-export，减少一次性迁移风险。
- 新增 `docs/reference/code-organization.md` 并接入贡献指南和文档索引。
- 删除 Git 中遗留的时间戳 `.bak` 代理脚本，生效实现只保留单一来源。
- 将 `scripts/test-gate.sh` 与其调用的 `scripts/check-extension-purity.sh` 规范为 LF，修复 Linux Bash 因 CRLF 无法识别 `set -o pipefail` 的问题。
- 新增 AI 编码与提交规范、PR 模板及 `scripts/ai-harness.sh`，把目录落位、仓库卫生、文档链接、commit subject 和提交证据变成可执行约束。

## 后续边界

- `App.tsx`、`web.rs`、`db.rs`、`commands.rs`、`scheduler.rs` 仍然过大，但必须分别立行为不变切片拆分，不能与目录移动、接口修改混在同一提交。
- 新功能不得把业务文件重新放回 `src/` 或 `src-tauri/src/` 平铺层。
- 删除兼容 re-export 前必须先完成所有内部路径迁移，并确认是否存在 crate 外调用。
