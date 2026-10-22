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

### Requirement: 渲染预算
场景 MUST 限制重绘频率（默认 30fps），静态层（地板/墙面）MUST 缓存复用；候场区 MUST 按行换行，保证 20 个成员不越出画布。

#### Scenario: 静态层缓存
- GIVEN 场景已初始化
- WHEN 连续多帧渲染
- THEN 地板只绘制一次并复用

#### Scenario: 20 个成员
- GIVEN 20 个成员
- WHEN 生成布局
- THEN 每个成员位置唯一
- AND 所有坐标落在画布范围内

### Requirement: 点击角色打开既有面板
点击场景中的角色 MUST 打开既有的成员面板，而不是新增一条独立的详情路径。

#### Scenario: 点中角色
- GIVEN 场景中有角色
- WHEN 点击某个角色
- THEN 成员面板打开
- AND 不发送消息、不改变选中角色以外的状态

### Requirement: 分群房间
场景 MUST 由群 id 稳定派生房间种子（0..7），并在地板色带上体现；同一群 MUST 永远得到同一房间，不同群 SHOULD 得到不同房间。房间配置 MUST 通过内核 `ctx.fork()` 的群作用域提供，切换群时释放旧作用域。

#### Scenario: 同群稳定
- GIVEN 群 id 不变
- WHEN 多次进入场景
- THEN 房间种子一致
- AND 地板色带位置一致

#### Scenario: 作用域释放
- GIVEN 已为某群打开场景作用域
- WHEN 切换到其他群或退出场景
- THEN 旧作用域被 dispose
- AND 根内核不再持有该群配置
