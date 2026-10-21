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

## 非目标（v0）

- 事件分发（emit / waterfall / parallel / serial / bail）
- 插件清单 v2 与插件加载器
- 按群作用域 fork 子上下文
- 异步依赖就绪等待
- 权限与沙箱
