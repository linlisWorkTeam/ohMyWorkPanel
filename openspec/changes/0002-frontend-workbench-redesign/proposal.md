# 前台工作台重设计

## Why
产品同时承载群聊、任务执行、版本交付、Goal 自治与插件场景。现有入口将领域状态、流订阅和管理表单集中在 App 中，主任务与设置混杂，附件不足以直接确定所有异常态与双端差异。重设计需要可还原的尺寸、组件状态、可测场景与领域迁移边界。

## What Changes
- 将群作为稳定工作空间，建立「协作 / 交付 / 目标 / 内容 / 场景 / 扩展」导航，保留聊天群能力差异。
- 统一任务状态与审批入口；精确区分运行状态、审批状态、连接状态、Agent 可用性。
- 提供页面级布局、交互与异常规范，七主题、桌面/Web/手机一体设计。
- 推荐保留 React + TypeScript + Vite 的领域模块化单体，拆薄 App，不重写后端，不新增全局框架。
- 新增前台体验 delta specs；保留既有领域 spec，不宣称本设计已经实现。

## Scope
账号与邀请、群与成员、聊天与 run、Agent 配置、Roadmap/Feature/Task/编排、Version/Wave/Ask/Release、Goal、Marketing、场景、扩展/Live、日志/经验/指标/Ops、设置与桌面更新入口。详见 [页面设计](ui-ux.md)、[功能与契约矩阵](feature-matrix.md)。

## Non-goals
本 PR 仅文档；不修改 src、Rust、数据库、依赖、API、IPC、WS、主题 palette 或生产部署。不承诺云同步、多人文档协同、插件市场、远程浏览器文件选择、服务端消息幂等能力。保留现有 Live Host 边界。

## Baseline and impact
附件《ohMyWorkPanel 产品与接口说明》为设计输入；代码基线为 `codex/agent-playground-2026-10-22@b2ce66c33052a3be08b2739ade325df796d08ac1`。
该分支包含 OpenSpec/Goal/场景，main 尚未包含全部内容，因此后续 PR 应先向此分支提交；若 #20 先合并，复核后改 base 到 main。
本 change 状态：设计待评审。实施任务均未完成；不合并 delta 到当前行为 SSOT，不归档为已交付。

