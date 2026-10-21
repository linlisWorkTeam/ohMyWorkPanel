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

### Requirement: 插件清单一致
每个内置插件 MUST 声明 `id`、`entry`、`provides`，且 `entry` 必须能在注册表中解析。

#### Scenario: 清单可解析
- GIVEN 内置插件清单
- WHEN 序列化后按 v2 规则解析
- THEN `entry` 与注册表 key 一致
- AND `provides` 与安装后实际注册的服务一致

## MODIFIED Requirements

（无）

## REMOVED Requirements

（无）
