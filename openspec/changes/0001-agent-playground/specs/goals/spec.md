# Delta for Goals

## ADDED Requirements

### Requirement: 目标生命周期
目标 MUST 从 `draft` 开始，经 `active` 运行；可以暂停、等待人类审批，最终进入 `completed` 或 `aborted`。终态 MUST 不可再变更。

#### Scenario: 启动
- GIVEN 一个 `draft` 目标
- WHEN 启动
- THEN 状态变为 `active`
- AND 记录启动时间

#### Scenario: 终态不可变
- GIVEN 目标已 `completed`
- WHEN 再次记录轮次或中止
- THEN 返回 `Terminal` 错误
- AND 状态保持不变

### Requirement: 预算必停
任何一次记录轮次之后，只要轮次、墙钟时长或 token 触达上限，目标 MUST 立即停止并记录停止原因。

#### Scenario: 轮次耗尽
- GIVEN `max_rounds = 2`
- WHEN 记录第 2 轮
- THEN 返回 `Stopped(RoundsExhausted)`
- AND 状态变为 `aborted`

#### Scenario: 时长耗尽
- GIVEN 启动时间早于 `max_wall_time_ms`
- WHEN 记录一轮
- THEN 返回 `Stopped(WallTimeExhausted)`

#### Scenario: token 耗尽
- GIVEN 累计 token 达到 `max_tokens`
- WHEN 记录一轮
- THEN 返回 `Stopped(TokensExhausted)`

#### Scenario: 不限制
- GIVEN 某项预算为 `0`
- WHEN 运行
- THEN 该项不触发停止

### Requirement: 进展与失败护栏
目标 MUST 在连续「无进展」或连续「失败」达到阈值时停止。任意一次有进展的轮次 MUST 重置无进展计数。

#### Scenario: 连续无进展
- GIVEN `max_no_progress_rounds = 2`
- WHEN 连续两轮 `NoProgress`
- THEN 第二轮返回 `Stopped(NoProgress)`

#### Scenario: 有进展重置计数
- GIVEN 已有一轮 `NoProgress`
- WHEN 下一轮为 `Progress`，再下一轮为 `NoProgress`
- THEN 不停止

#### Scenario: 连续失败
- GIVEN `max_consecutive_failures = 2`
- WHEN 连续两轮 `Failed`
- THEN 返回 `Stopped(ConsecutiveFailures)`

### Requirement: 达成即完成
判定为 `GoalMet` 的轮次 MUST 立即把目标置为 `completed`，不再评估后续护栏。

#### Scenario: 目标达成
- GIVEN 处于 `active`
- WHEN 记录 `GoalMet`
- THEN 返回 `Stopped(GoalMet)`
- AND 状态为 `completed`

### Requirement: 人类闸门
开启每轮审批后，每轮结束 MUST 进入 `awaiting_approval`，只有人类批准才能继续。

#### Scenario: 等待审批
- GIVEN 目标开启每轮审批
- WHEN 记录一轮
- THEN 返回 `AwaitApproval`
- AND 状态为 `awaiting_approval`

#### Scenario: 批准
- GIVEN 目标在 `awaiting_approval`
- WHEN 批准
- THEN 状态回到 `active`

#### Scenario: 未激活时记录
- GIVEN 目标已暂停或等待审批
- WHEN 记录轮次
- THEN 返回 `NotActive`

### Requirement: 持久化
目标与轮次 MUST 落库，进程重启后 MUST 能恢复状态、预算快照与全部轮次；终态与停止原因 MUST 保留。

#### Scenario: 恢复
- GIVEN 目标已运行 3 轮并落库
- WHEN 重新读取
- THEN 状态、预算、3 条轮次与停止原因一致

#### Scenario: 终态保留
- GIVEN 目标已因预算中止
- WHEN 重新读取
- THEN 状态为 `aborted`
- AND 停止原因为对应护栏

#### Scenario: 按群列表
- GIVEN 同一群有多个目标
- WHEN 列出
- THEN 按创建时间倒序返回
