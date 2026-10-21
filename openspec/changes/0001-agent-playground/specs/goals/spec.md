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

### Requirement: 驱动循环
驱动器 MUST 按「执行一轮 → 判定 → 记录」推进，并在目标停止、等待审批或触达单次步数上限时结束。

#### Scenario: 推进到达成
- GIVEN 目标是 `active`
- WHEN judge 在第 3 轮判定 `GoalMet`
- THEN 驱动执行 3 轮后结束
- AND 终止原因为 `GoalMet`

#### Scenario: 单次步数上限
- GIVEN 预算不限且 judge 始终返回 `Progress`
- WHEN 上限为 5
- THEN 驱动在 5 轮后停止
- AND 返回 `StepBudgetReached`

#### Scenario: 审批闸门
- GIVEN 目标开启每轮审批
- WHEN 驱动执行一轮
- THEN 结束原因为 `AwaitingApproval`
- AND 目标进入 `awaiting_approval`

#### Scenario: 执行失败
- GIVEN runner 返回错误
- WHEN 驱动一轮
- THEN 返回 runner 错误
- AND 不写入任何轮次
- AND 目标仍为 `active`

#### Scenario: 确定性 prompt
- GIVEN 同一目标连续组装 prompt
- WHEN 轮次未变
- THEN 输出完全一致
- AND 包含目标、轮次序号与上一轮判定

### Requirement: 派发式执行
驱动器 MUST 通过 dispatcher 发起一轮并轮询结果；等待超时、run 失败或取消 MUST 视为本轮失败，由驱动器决定是否记录。

观察输出 MUST 截断到 2000 字以内再进入证据账本。

#### Scenario: 轮询后完成
- GIVEN dispatcher 先返回 pending，再返回 completed
- WHEN 驱动一轮
- THEN 得到输出摘要与 token 用量

#### Scenario: 等待超时
- GIVEN dispatcher 一直返回 pending
- WHEN 超过轮询上限
- THEN 返回超时错误
- AND 错误信息包含派发句柄

#### Scenario: run 失败或取消
- GIVEN dispatcher 返回 failed 或 canceled
- WHEN 驱动一轮
- THEN 返回错误
- AND 不产生观察结果

#### Scenario: 长输出截断
- GIVEN 输出超过 2000 字
- WHEN 生成观察
- THEN 摘要截断为 2000 字加省略号

### Requirement: 证据账本
每一轮 MUST 保存观察摘要（`summary`），并能在重新读取时恢复；旧数据 MUST 以空串兼容。

#### Scenario: 保存摘要
- GIVEN 驱动器执行一轮并产出观察摘要
- WHEN 轮次落库
- THEN 摘要与判定、token 一起保存
- AND 重新读取可见相同摘要

#### Scenario: 旧数据兼容
- GIVEN schema v8 的轮次记录没有 summary 列
- WHEN 升级到 v9
- THEN 新增列默认空串
- AND 旧轮次仍可读取

#### Scenario: 界面上回看
- GIVEN 目标至少有一轮
- WHEN 渲染 Goal 条
- THEN 显示最近一轮摘要（过长省略，悬停看全文）

### Requirement: 写权限
只有平台管理员或该群群主 MUST 能创建、启动、暂停、批准或中止目标；普通成员 MUST 只能读取。

#### Scenario: 平台管理员
- GIVEN 平台管理员
- WHEN 创建或启动目标
- THEN 允许

#### Scenario: 群主
- GIVEN 用户在群里的成员绑定为该群 owner
- WHEN 操作本群目标
- THEN 允许

#### Scenario: 普通成员
- GIVEN 普通成员
- WHEN 尝试启动目标
- THEN 返回「只有群主或平台管理员可以操作目标」
- AND 目标状态不变

#### Scenario: 跨群群主
- GIVEN 用户是另一个群的群主
- WHEN 操作本群目标
- THEN 拒绝

### Requirement: 单驱动与事务边界
同一个目标同一时刻 MUST 只允许一个驱动器；驱动一轮与轮次落库 MUST 在同一事务内完成，失败 MUST 回滚。

#### Scenario: 并发驱动
- GIVEN 目标已有驱动器持有锁
- WHEN 第二个驱动器尝试启动
- THEN 返回「正在被驱动」
- AND 不执行任何轮次

#### Scenario: 失败回滚
- GIVEN runner 执行失败
- WHEN 驱动一轮
- THEN 轮次不落库
- AND 目标状态保持 `active`

#### Scenario: 释放后重试
- GIVEN 一次驱动已完成
- WHEN 再次申请驱动权
- THEN 申请成功

### Requirement: 创建校验
创建目标 MUST 校验群存在、目标非空且不超过 2000 字；不满足时 MUST 拒绝并给出原因。

#### Scenario: 目标为空
- GIVEN 目标描述为空白
- WHEN 创建
- THEN 返回错误
- AND 不写入任何记录

#### Scenario: 群不存在
- GIVEN 一个不存在的 groupId
- WHEN 创建
- THEN 返回「群不存在」

#### Scenario: 状态迁移落库
- GIVEN 目标已启动
- WHEN 记录一轮并触发停止
- THEN 轮次与状态在同一事务内写入
- AND 重新读取可见相同结果

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
