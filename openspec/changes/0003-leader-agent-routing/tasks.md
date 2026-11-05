# Tasks

## 1. 规格
- [x] 1.1 proposal / design / delta spec（R1–R5）
- [ ] 1.2 维护者评审分类启发式与默认重试上限（待评审）

## 2. 实现
- [x] 2.1 `routing::classify`（Retryable/Permanent/Cancelled/Unknown）
- [x] 2.2 `routing::decide`（重试/升级/停止 + 守卫）
- [x] 2.3 `routing::route_failed_run`（DB 落地：重试 run、升级消息 + Leader run、route event）
- [x] 2.4 `scheduler::finish_failed` 挂钩并触发 `schedule_group`
- [x] 2.5 模块注册与 rustfmt/clippy 干净

## 3. 验证
- [x] 3.1 纯逻辑单测：分类样本、决策全分支（classify/decide 共 7 例）
- [x] 3.2 DB 集成测试：首次失败重试、重试耗尽升级、Leader 失败不升级、根运行不路由（5 例）
- [x] 3.3 `cargo test --no-default-features --lib` 全绿（316 passed，含 12 个 routing 测试）
- [ ] 3.4 真实群冒烟（未完成：mock 不回显 @，E2E 无法触发委派；需真实 CLI 或可回显的测试适配器）
