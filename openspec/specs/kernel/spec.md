# Kernel Specification

## Purpose

插件内核提供三件地基：**服务容器**、**依赖声明**、**可撤销副作用**。它让能力以可加载、可卸载的单元组合，而不是继续堆进入口文件。

v0 只覆盖进程内、单进程、同步语义；事件分发与清单 v2 属于后续增量。

## Requirements

### Requirement: 服务注册表
内核 MUST 允许以稳定 key 注册服务，并按 key 做类型安全取回。同一个 key MUST NOT 被注册两次。

#### Scenario: 注册后取回
- GIVEN 上下文为空
- WHEN 用 key `greeter` 注册一个服务实例
- THEN 用同一 key 可以取回该实例
- AND 取回值的类型与注册时一致

#### Scenario: 重复注册
- GIVEN key `greeter` 已被注册
- WHEN 再次注册同一 key
- THEN 返回 `Duplicate` 错误
- AND 原有服务保持不变

#### Scenario: 类型不匹配
- GIVEN key `greeter` 注册的是 `Greeter` 类型
- WHEN 以 `u32` 类型取回
- THEN 返回 `TypeMismatch` 错误

### Requirement: 依赖声明
插件 MUST 能够一次性声明所需服务；内核 MUST 在依赖缺失时返回**全部**缺失项，而不是只报第一个。

#### Scenario: 多个依赖缺失
- GIVEN 已注册服务 `a`
- WHEN 声明依赖 `[a, b, c]`
- THEN 返回 `Missing(["b", "c"])`
- AND 顺序与声明一致

#### Scenario: 依赖齐全
- GIVEN 声明的依赖全部已注册
- WHEN 执行依赖检查
- THEN 返回成功

### Requirement: 可撤销副作用
所有注册 MUST 登记 disposer；上下文释放时 MUST 按**逆序**执行，且每个 disposer 只执行一次。重复释放 MUST 安全。

#### Scenario: 逆序释放
- GIVEN 依次登记副作用 1、2
- WHEN 释放上下文
- THEN 执行顺序为 2、1
- AND 两个副作用句柄都标记为已释放

#### Scenario: 重复释放
- GIVEN 上下文已释放
- WHEN 再次释放
- THEN 不重复执行任何 disposer
- AND 不返回错误

#### Scenario: 共享上下文只释放一次
- GIVEN 同一个上下文的两个克隆
- WHEN 两个克隆分别释放
- THEN 每个 disposer 仍只执行一次

### Requirement: 释放后拒绝注册
已释放的上下文 MUST 拒绝新服务与新副作用，且原有服务 MUST 不可再取回。

#### Scenario: 释放后注册
- GIVEN 上下文已释放
- WHEN 注册服务或登记副作用
- THEN 返回 `Disposed` 错误
- AND 取回旧服务返回 `NotFound`

### Requirement: 事件分发
内核 MUST 提供五种分发语义，且语义与调用点一致：

| 模式 | 顺序 | 等待 | 返回值 |
|---|---|---|---|
| `emit` | 注册顺序 | 逐个等待 | 无（返回调用数量） |
| `parallel` | 并发 | 全部完成 | 每个监听器的可选结果 |
| `serial` | 注册顺序 | 逐个等待 | 每个监听器的可选结果 |
| `bail` | 注册顺序 | 遇到首个返回值即停 | 首个返回值 |
| `waterfall` | 注册顺序 | 值链式传递 | 最终值 |

#### Scenario: emit 按注册顺序调用全部监听器
- GIVEN 依次注册 a、b、c 三个监听器
- WHEN 以 `emit` 分发
- THEN 调用顺序为 a、b、c
- AND 返回被调用数量 3

#### Scenario: bail 短路
- GIVEN 第一个监听器返回 `None`，第二个返回 `Some(allow)`，第三个返回 `Some(deny)`
- WHEN 以 `bail` 分发
- THEN 结果为 `allow`
- AND 第三个监听器不被调用

#### Scenario: serial 保序收集
- GIVEN 两个依次返回 1、2 的监听器
- WHEN 以 `serial` 分发
- THEN 结果为 `[Some(1), Some(2)]`

#### Scenario: parallel 收集全部结果
- GIVEN 两个返回不同值的监听器
- WHEN 以 `parallel` 分发
- THEN 两个结果都出现
- AND 顺序不作为契约

#### Scenario: waterfall 链式传递
- GIVEN 监听器 A 返回 `value + 1`、监听器 B 返回 `value * 2`
- WHEN 以 `waterfall` 分发初值 3
- THEN 结果为 8
- AND 每个监听器收到的是上一个的返回值

#### Scenario: waterfall 不修改
- GIVEN 某个监听器返回 `None`
- WHEN 以 `waterfall` 分发
- THEN 当前值原样传给下一个监听器

#### Scenario: 注销监听器
- GIVEN 同一事件注册了两个监听器
- WHEN 注销其中一个
- THEN 只剩一个监听器参与分发
- AND 重复注销返回未找到

### Requirement: 释放清空监听器
上下文释放时 MUST 清空事件总线，避免已卸载插件继续收到事件。

#### Scenario: 释放后不再分发
- GIVEN 上下文注册了监听器
- WHEN 释放上下文后再分发同名事件
- THEN 监听器不会被调用

### Requirement: 按依赖顺序装载插件
装载器 MUST 先安装依赖的提供者，再安装消费者，无论输入顺序如何；插件 id MUST 唯一。

#### Scenario: 输入顺序颠倒
- GIVEN `consumer` 依赖 `svc.a`，`provider` 提供 `svc.a`
- WHEN 以 `[consumer, provider]` 的顺序装载
- THEN `provider` 先安装
- AND 报告中的顺序为 `[provider, consumer]`

#### Scenario: 重复 id
- GIVEN 两个插件声明同一个 id
- WHEN 装载
- THEN 返回 `DuplicateId`
- AND 不安装任何插件

### Requirement: 装载失败整体回滚
任一插件安装失败、依赖缺失、依赖成环，或声明提供的服务实际未注册时，装载器 MUST 释放上下文，不允许半套插件继续运行。

#### Scenario: 安装函数报错
- GIVEN 插件安装返回错误
- WHEN 装载
- THEN 返回 `Install` 错误
- AND 上下文已释放

#### Scenario: 声明未兑现
- GIVEN 插件声明 `provides = ["svc.x"]` 但安装后没有注册
- WHEN 装载
- THEN 返回 `ProvidesNotRegistered`
- AND 上下文已释放

#### Scenario: 依赖成环
- GIVEN `a` 依赖 `svc.b`、`b` 依赖 `svc.a`，两者各自提供对方需要的服务
- WHEN 装载
- THEN 返回 `Cycle`
- AND 上下文已释放

#### Scenario: 依赖无人提供
- GIVEN 插件依赖一个没有任何插件提供的服务
- WHEN 装载
- THEN 返回 `MissingDependency` 并列出缺口

### Requirement: 插件清单 v2
每个插件目录 MUST 提供 `plugin.manifest.json`，包含 `id`、`version`、`entry`；`provides`、`inject`、`permissions` 可选。

清单 MUST 被校验：必填字段不得为空；同一字段不得重复；同一 key MUST NOT 同时出现在 `provides` 与 `inject`。装载器 MUST 拒绝注册表中不存在的 `entry`，且不安装任何插件。

#### Scenario: 合法清单
- GIVEN 清单含 `id`、`version`、`entry` 与一个 `provides` key
- WHEN 解析
- THEN 返回结构化清单
- AND 可选字段取默认值

#### Scenario: 空 id
- GIVEN 清单 `id` 为空白
- WHEN 解析
- THEN 返回 `EmptyField("id")`

#### Scenario: key 冲突
- GIVEN 同一个 key 同时出现在 `provides` 与 `inject`
- WHEN 解析
- THEN 返回 `ConflictingKey`

#### Scenario: 未注册的入口
- GIVEN 清单声明 `entry = "builtin:missing"`
- WHEN 映射为插件规格
- THEN 返回 `UnknownEntry`
- AND 不安装任何插件

### Requirement: 清单目录发现
系统 MUST 只扫描包含 `plugin.manifest.json` 的根目录；没有清单的目录 MUST 被跳过。

#### Scenario: 混合根目录
- GIVEN 根 A 含清单、根 B 为空
- WHEN 执行发现
- THEN 只返回根 A
- AND 根 B 不产生错误

### Requirement: 前端同构内核
前端 MUST 提供与 Rust 侧同构的内核实现（`src/kernel/`）：服务容器、`inject` 依赖校验、可撤销副作用、五种事件分发、依赖拓扑装载与失败回滚。

前端实现 MUST 与 Rust 侧共享同一组行为用例：注册/重复注册、缺失依赖一次性列出、副作用逆序释放、`emit`/`parallel`/`serial`/`bail`/`waterfall` 语义、装载顺序、重复 id、依赖成环、声明未兑现。

#### Scenario: 注册后取回
- GIVEN 上下文已 `provide("svc.a", value)`
- WHEN `require("svc.a")`
- THEN 返回同一个值

#### Scenario: 装载失败整体释放
- GIVEN 某个插件 `install` 抛错
- THEN 返回 `install` 类错误
- AND 上下文 `isDisposed()` 为真
- AND 已注册服务不再可用

#### Scenario: 依赖顺序
- GIVEN 消费者先于提供者出现在输入里
- WHEN 装载
- THEN 提供者先安装、消费者后安装

### Requirement: 作用域 fork
上下文 MUST 提供 `fork()` 派生子作用域：查找服务时沿父链逐级向上，但子作用域拥有独立的注册表、副作用栈与事件总线；子作用域 `dispose()` MUST 只回收自己的副作用，不影响父作用域。父作用域释放后，子作用域对父服务的查找 MUST 自然失效。

#### Scenario: 继承父服务
- GIVEN 父上下文已注册 `svc.a`
- WHEN 子作用域 `require("svc.a")`
- THEN 取回父作用域的值

#### Scenario: 子作用域覆盖
- GIVEN 父子都注册了同一 key
- WHEN 各自 `require`
- THEN 各自取回自己的值，互不影响

#### Scenario: 子作用域释放
- GIVEN 子作用域登记了副作用
- WHEN 子作用域 `dispose()`
- THEN 只执行子作用域的副作用
- AND 父作用域服务仍可用

#### Scenario: 父作用域释放
- GIVEN 父上下文已释放
- WHEN 子作用域查找父服务
- THEN 返回「服务不存在」

## 非目标（v0）

- 动态库插件与热重载（v0 只支持编译期注册）
- 异步依赖就绪等待
- 权限与沙箱
