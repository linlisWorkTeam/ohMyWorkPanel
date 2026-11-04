---
date: 2026-11-04
topic: frontend-workbench-design
branch: docs/frontend-workbench-openspec-2026-11-04
status: design-ready-submit-blocked
---

# 前台工作台设计交接

## Built this session
仅Markdown设计：OpenSpec 0002 proposal/design、UI/UX参数和逐页交互、23组功能映射、16条验收场景与实施tasks。无业务代码与运行行为变化，测试设计N/A。

## Key files
- [设计入口](../../openspec/changes/0002-frontend-workbench-redesign/README.md)
- [版本流水线](../version-pipeline.md)

## Locked product decisions
群是协作空间；保留React/TS/Vite与领域模块化单体；App薄入口；api.*双传输边界；七主题语义token与PageLayout；场景只读且fail-open。

## Known pitfalls
附件run状态和桌面能力列表不完全等同代码。准确基线b2ce66c33052a3be08b2739ade325df796d08ac1，目标base为codex/agent-playground-2026-10-22，避免混入#20已有实现。
网络拒绝直接git clone，环境未发现cargo；完整门禁未通过，不能宣称已提交/已创建PR。所有设计变更已准备为可应用文档包，后续必须先满足提交规则或由用户明确授权纯文档例外。

## How to run / verify
设计包可检查UTF-8、相对链接、delta Requirement/Scenario结构和仅Markdown范围。正式提交前运行pnpm run test:gate，提交后pnpm run submit:ai。当前未执行成功，未做浏览器/七主题渲染或真实CLI验证。

## Do not regress
HTTP/IPC/WS/SQLite、Live Host边界、桌面免Web登录、默认响应者、发送键偏好与草稿隔离均保留。不把UI版本发布当成生产部署。

## Open follow-ups
先解决提交门禁环境限制，再创建PR；评审0.3后才能开始M0–M5实现。文档中的权限/seq待确认项须核验，不得编造契约。

