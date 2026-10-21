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

### Requirement: 可回退的视图切换
场景 MUST 作为独立视图存在且默认不激活；退出后 MUST 回到聊天，不改变消息、草稿或调度状态。

#### Scenario: 进入与退出
- GIVEN 用户正在聊天
- WHEN 点击像素场景入口
- THEN 主区切换为场景视图
- AND 输入框隐藏
- WHEN 点击「返回聊天」
- THEN 回到聊天视图
- AND 消息与草稿保持原样

#### Scenario: 空群
- GIVEN 群内没有成员
- WHEN 进入场景
- THEN 显示空状态提示
- AND 不渲染画布

### Requirement: 点击角色打开既有面板
点击场景中的角色 MUST 打开既有的成员面板，而不是新增一条独立的详情路径。

#### Scenario: 点中角色
- GIVEN 场景中有角色
- WHEN 点击某个角色
- THEN 成员面板打开
- AND 不发送消息、不改变选中角色以外的状态
