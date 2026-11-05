# Leader 路由 delta

## ADDED Requirements

本 capability 定义 Leader Agent 委派链路的失败兜底。既有 scheduler 的失败标记/事件广播契约不变；以下 MUST 描述新增的路由动作。

### Requirement: 失败分类（R1）
系统 MUST 将委派子运行的失败归类为 Retryable / Permanent / Cancelled / Unknown，并以错误文本为依据，不得凭空猜测成功。

#### Scenario: 可恢复错误
- GIVEN 目标 Agent 的子运行因超时或连接重置失败
- WHEN 失败被处理
- THEN 分类为 Retryable，并按重试策略处理

#### Scenario: 永久错误
- GIVEN 子运行因 401/403、API Key 缺失、CLI 未安装或额度耗尽失败
- WHEN 失败被处理
- THEN 分类为 Permanent，不重试，直接升级

#### Scenario: 用户取消
- GIVEN 子运行以 cancelled/aborted 结束
- WHEN 失败被处理
- THEN 分类为 Cancelled，不重试、不升级

### Requirement: 重试路由（R2）
系统 MUST 在分类为 Retryable/Unknown 且失败次数未超过上限时，向同一目标创建新的重试运行；重试 MUST 复用原 root message、原父运行与相同深度。

#### Scenario: 首次失败重试
- GIVEN Leader 委派给目标 A 的子运行第一次失败
- WHEN 路由决策执行
- THEN 创建同一目标的新 run，父运行与深度不变，并调度执行

#### Scenario: 重试不去重输出
- GIVEN 重试运行完成
- WHEN 用户查看群聊
- THEN 允许出现两条输出（原失败消息 + 重试消息）；系统不宣称恰好一次

### Requirement: 升级路由（R3）
系统 MUST 在 Permanent 失败或重试耗尽时升级给 Leader：插入可见的「路由升级」消息，并以该消息为 root 创建 Leader 运行。

#### Scenario: 永久失败升级
- GIVEN 子运行因鉴权失败（Permanent）
- WHEN 路由决策执行
- THEN 群内出现 `[路由升级]` 消息，且 Leader 收到一条新运行用于重新指派或接手

#### Scenario: 重试耗尽升级
- GIVEN 同一目标在同一父运行下的第二次失败
- WHEN 路由决策执行
- THEN 不再重试，转为升级

### Requirement: 路由守卫（R4）
系统 MUST NOT 对以下运行做自动路由：无父运行（用户根运行）、Leader 自身运行、审阅运行、深度达到上限、Cancelled。

#### Scenario: Leader 自身失败
- GIVEN Leader 的升级运行失败
- WHEN 失败被处理
- THEN 不产生新的升级（防止升级循环）

#### Scenario: 深度上限
- GIVEN 子运行深度已达到 `max_delegation_depth`
- WHEN 失败被处理
- THEN 停止路由，仅保留 failed 标记与事件

### Requirement: 可观测（R5）
系统 MUST 为每次重试/升级写入 run_events `route`，包含 action、class、attempts；升级消息 MUST 含目标名、分类与错误摘要（≤200 字符）。

#### Scenario: 排障
- GIVEN 运维查看某失败 run 的事件
- WHEN 路由已发生
- THEN 能看到 route 事件及升级 run id

## 兼容性
- 不新增/修改数据库表或字段；
- 不改变既有 `run_status` 事件、失败消息状态与 scheduler 并发策略；
- 不新增前端 API；升级消息走既有消息通道。
