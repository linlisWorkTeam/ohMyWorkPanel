# Delta for Plugin Migrations

## ADDED Requirements

### Requirement: 适配器目录插件
系统 MUST 以内置插件形式提供 CLI 适配器目录服务（`svc.adapters`），其列表 MUST 与既有自由函数 `catalog()` 保持一致。

#### Scenario: 服务可用
- GIVEN 内核装载成功
- WHEN 读取适配器目录
- THEN 返回与自由函数相同的列表
- AND 列表非空

#### Scenario: 内核不可用
- GIVEN 插件装载失败
- WHEN Web 请求 `/api/adapters`
- THEN 回退自由函数
- AND 仍返回完整列表

### Requirement: 适配器解析服务
适配器目录服务 MUST 提供 `resolve(name)`，其结果 MUST 与自由函数 `resolve_adapter` 一致；执行路径 MUST 优先使用该服务，未装载时回退自由函数。

#### Scenario: 解析一致
- GIVEN 一个内置适配器名
- WHEN 分别通过服务与自由函数解析
- THEN 两者的启动规格一致

#### Scenario: 未知适配器
- GIVEN 不存在的适配器名
- WHEN 通过服务解析
- THEN 返回错误
- AND 执行路径不会启动任何进程

### Requirement: 候选可执行文件
适配器目录服务 MUST 提供 `candidates(name)`：清单适配器返回清单里的 `executables`，内置适配器返回内置候选表；Agent 自检 MUST 优先使用该服务。

#### Scenario: 内置候选
- GIVEN 适配器 `cursor`
- WHEN 通过服务取候选
- THEN 与内置候选表一致
- AND 非空

#### Scenario: 未知适配器
- GIVEN 不存在的名字
- WHEN 取候选
- THEN 返回错误

### Requirement: 工作流写用例
版本创建 MUST 能通过工作流服务执行，结果与自由函数一致（`kind=draft`、`status=planning`）；Web 入口 MUST 优先使用服务。

#### Scenario: 通过服务创建版本
- GIVEN 群里有管理员 Agent
- WHEN 通过服务创建版本
- THEN 返回 `kind=draft`、`status=planning`
- AND 版本板能看到它

#### Scenario: 回退
- GIVEN 插件未装载
- WHEN 创建版本
- THEN 使用自由函数
- AND 行为不变

### Requirement: 插件清单一致
每个内置插件 MUST 声明 `id`、`entry`、`provides`，且 `entry` 必须能在注册表中解析。

#### Scenario: 清单可解析
- GIVEN 内置插件清单
- WHEN 序列化后按 v2 规则解析
- THEN `entry` 与注册表 key 一致
- AND `provides` 与安装后实际注册的服务一致

### Requirement: Marketing 生成 / 修订
Marketing 服务 MUST 提供生成与修订能力；需要调度器时 MUST 由传输层传入 `SchedulerState`，插件自身不持有调度器状态。

#### Scenario: 生成
- GIVEN 群与成员合法
- WHEN 通过服务创建 campaign
- THEN 行为与自由函数一致

#### Scenario: 调度器注入
- GIVEN 插件未装载
- WHEN 发起生成
- THEN 回退自由函数
- AND 行为不变

## MODIFIED Requirements

（无）

## REMOVED Requirements

（无）
