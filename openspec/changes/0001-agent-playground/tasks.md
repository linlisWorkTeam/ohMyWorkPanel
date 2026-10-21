# Tasks: Agent 游乐园程序

> 每完成一条勾选一条；勾选前必须写明验证方式的结果。

## P0 契约与骨架（本变更）

- [x] 0.1 建立 `openspec/` 骨架：README、project、specs、changes
- [x] 0.2 写 `specs/extensions/spec.md` 作为样板契约（含非目标）
- [x] 0.3 写本 change 的 proposal / design / tasks
- [x] 0.4 建 capability 清单并标注迁移状态（`specs/README.md`）
- [x] 0.5 在 `docs/version-pipeline.md` 占位「阶段 9 — Agent 游乐园」
- [x] 0.6 `pnpm run check:ai` 通过（目录/链接/卫生检查）
- [x] 0.7 提交 P0（单独 commit，不夹带代码）

## P1 插件内核 v0

- [x] 1.1 Rust：`kernel::Context` + `Service` 注册表（稳定 key、类型安全取回）
- [x] 1.2 Rust：`inject` 声明依赖（缺失时一次性列出全部缺口）
- [x] 1.3 Rust：`effect` / disposer 回收（逆序卸载、重复释放幂等，含单测）
- [ ] 1.3a Rust：按群作用域 fork 子上下文（服务可见性隔离）
- [x] 1.4 Rust：类型化事件分发（emit / parallel / serial / bail，含短路与注销语义）
- [x] 1.4b Rust：插件装载器（依赖拓扑排序、声明兑现校验、失败整体回滚，7 个单测）
- [ ] 1.4a Rust：`waterfall`（环绕中间件）语义
- [ ] 1.5 TS：前端同构内核，契约测试与 Rust 侧共享用例
- [x] 1.6 manifest v2：`plugin.manifest.json`（id/version/entry/provides/inject/permissions）+ 目录发现 + 注册表映射（7 个单测）
- [ ] 1.7 样板迁移：把现有扩展宿主改成 kernel 上的插件
- [ ] 1.8 验证：`cargo test --lib` + `pnpm test` + `pnpm run test:gate`

## P2 像素前台（可与 P1 并行）

- [x] 2.1 scene 壳模式开关 + 回退到现有壳（默认关闭）
- [x] 2.2 只读投影适配层：member / run → 场景角色（7 个单测，纯函数、无后端改动）
- [x] 2.3 Canvas 2D 渲染：地板、网格、桌面、角色、状态色（DPR 与缩放适配）
- [x] 2.4 角色状态机：idle / working / blocked / done 与动画（呼吸 / 闪烁 / 完成点）
- [x] 2.5 交互：点角色 → 打开既有成员面板
- [ ] 2.6 性能：20 成员场景稳定 60fps，离屏缓存与裁剪
- [ ] 2.7 验证：关闭开关时零行为差异；开启时可完整操作（需真实群数据人工过一遍）

## P3 Goal 模式 MVP

- [ ] 3.1 `goal` 领域对象与 SQLite schema（迁移版本递增）
- [ ] 3.2 循环驱动器插件：plan → act → observe → judge
- [ ] 3.3 预算与熔断：轮次 / 时长 / token / 无进展
- [ ] 3.4 人类闸门：每轮确认或里程碑确认
- [ ] 3.5 证据账本：每轮结果落库并在群里可回看
- [ ] 3.6 验证：一个真实 goal 端到端跑完并停止

## P4 全量插拔

- [ ] 4.1 迁移 CLI 适配器为插件
- [ ] 4.2 迁移工作流（版本 / Wave）为插件
- [ ] 4.3 迁移 marketing 为插件
- [ ] 4.4 内置功能不再直接出现在 `commands.rs` / `App.tsx`
- [ ] 4.5 每个迁移都补 OpenSpec delta 与回归测试
