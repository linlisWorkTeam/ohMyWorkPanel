# Delta for Scene

## ADDED Requirements

### Requirement: 只读投影
场景 MUST 只从既有的 member / run 数据投影出角色，不拥有业务状态、不新增写入路径，也不要求后端改动。

#### Scenario: 无 run 的成员
- GIVEN 群里有成员但没有对应的 run
- WHEN 生成场景
- THEN 该成员出现在候场区
- AND 状态为 `idle`

#### Scenario: 有活跃 run 的成员
- GIVEN 成员有一条 `running` 的 run
- WHEN 生成场景
- THEN 该成员坐在工位
- AND 状态为 `working`

#### Scenario: 同一成员多条 run
- GIVEN 同一成员先 `running` 后 `done`
- WHEN 按时间顺序传入
- THEN 以最后一条为准
- AND 状态为 `done`

### Requirement: 确定性布局
同一输入 MUST 产生同一组坐标，保证测试与动画补间可复现。

#### Scenario: 重复投影
- GIVEN 相同的成员与 run
- WHEN 连续投影两次
- THEN 两次结果完全一致

#### Scenario: 工位换行
- GIVEN 成员数超过一行工位
- WHEN 投影
- THEN 第 N 列之后回到第一列
- AND 行号递增一个工位间距

### Requirement: 未知状态降级
未知的 run 状态 MUST 按 `idle` 处理，避免新增状态把角色画成异常态。

#### Scenario: 新增状态
- GIVEN run 状态为 `paused`（未知）
- WHEN 投影
- THEN 角色状态为 `idle`
