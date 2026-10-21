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
- [x] 1.7 样板迁移：扩展宿主成为第一个内置插件（Web 反代与 A2A 走内核服务，桌面命令保留等价自由函数）
- [ ] 1.7a 桌面命令侧切换到内核服务（保持 IPC 签名不变）
- [ ] 1.8 验证：`cargo test --lib` + `pnpm test` + `pnpm run test:gate`

## P2 像素前台（可与 P1 并行）

- [x] 2.1 scene 壳模式开关 + 回退到现有壳（默认关闭）
- [x] 2.2 只读投影适配层：member / run → 场景角色（7 个单测，纯函数、无后端改动）
- [x] 2.3 Canvas 2D 渲染：地板、网格、桌面、角色、状态色（DPR 与缩放适配）
- [x] 2.4 角色状态机：idle / working / blocked / done 与动画（呼吸 / 闪烁 / 完成点）
- [x] 2.5 交互：点角色 → 打开既有成员面板
- [x] 2.6 渲染预算：30fps 上限 + 地板离屏缓存 + 候场区换行（20 成员不越界）+ 选中高亮
- [ ] 2.7 验证：关闭开关时零行为差异；开启时可完整操作（需真实群数据人工过一遍）

## P3 Goal 模式 MVP

- [x] 3.1a `goal` 领域模型与状态机（Draft/Active/Paused/AwaitingApproval/Completed/Aborted，12 个单测）
- [x] 3.1b `goal` 持久化：`goals` + `goal_rounds`（schema v8，5 个 store 单测）
- [x] 3.1c Goal 应用服务：创建校验、状态迁移、轮次事务写入（6 个单测）
- [x] 3.1d Goal IPC + 前端入口：Tauri 7 命令 / Web 6 路由 / 群聊顶部 Goal 条（新建、启动、暂停、批准、停止）
- [x] 3.1e 写权限收紧：群主 / 平台管理员（3 个单测；Web 入口已拦截）
- [x] 3.2a 驱动器内核：注入式 runner/judge、单次步数上限、prompt 组装（7 个单测）
- [x] 3.2b 驱动运行时：单 goal 驱动锁 + tick 事务落库 + 失败回滚（5 个单测）
- [x] 3.2c 派发式 runner：dispatcher trait + 轮询/超时/失败/取消 + 摘要截断（7 个单测）
- [x] 3.2d 调度器适配：写消息 + 建 run + 注入 kick + 状态映射 + 启发式 judge（7 个单测，含端到端 tick）
- [x] 3.2e 驱动器入口：`advance_goal` 命令 + Web 路由 + Goal 条「推进一步」按钮（阻塞任务 + 角色选择单测）
- [x] 3.3 预算与熔断：轮次 / 时长 / token / 无进展 / 连续失败（触线即停）
- [x] 3.4 人类闸门：每轮确认（`AwaitingApproval` → `approve`）
- [x] 3.5 证据账本：`goal_rounds.summary`（schema v9）+ Goal 条展示最近一轮
- [x] 3.6a 自动推进扫描：目标级开关（schema v10）+ 冷却 + 失败隔离 + 创建表单勾选（4 个单测）
- [x] 3.6b 后台循环接线：Tauri / Web 启动时 spawn 扫描（10s），重启后自动接管 active+auto 目标
- [ ] 3.7 验证：一个真实 goal 端到端跑完并停止（需真实 Agent）

## P4 全量插拔

- [x] 4.1a 适配器目录插件：`svc.adapters` + Web 入口优先服务、回退自由函数（3 个单测）
- [x] 4.1b 适配器解析走内核服务：scheduler 执行路径优先 `svc.adapters.resolve`，失败回退自由函数
- [x] 4.1c 候选可执行文件走服务：`candidates()` + agent 自检调用点迁移；`build_args` 暂留适配器内部（仅 run_streaming 使用）
- [x] 4.2a workflow 契约 + 版本板读数插件：`specs/workflow/spec.md` + `svc.workflow.board`（3 个单测）
- [ ] 4.2b workflow 写操作插件化（版本 / Roadmap / Ask / Wave / 发布）
- [x] 4.3a marketing 契约 + 查询插件：`specs/marketing/spec.md` + `svc.marketing.list/get`（3 个单测）
- [ ] 4.3b marketing 生成 / 审核 / 导出插件化
- [ ] 4.4 内置功能不再直接出现在 `commands.rs` / `App.tsx`
- [ ] 4.5 每个迁移都补 OpenSpec delta 与回归测试
