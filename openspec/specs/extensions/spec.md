# Extensions Specification

## Purpose

扩展宿主让外部目录以清单方式接入平台：被发现、被反代、以页签形式出现在对应群里。当前实现是**静态清单 + 进程外运行**，没有生命周期、依赖注入和卸载回滚——那些属于插件内核演进（见 `changes/0001-agent-playground/`）。

## Requirements

### Requirement: 扩展发现
系统 MUST 从配置的根目录中发现包含 `extension.manifest.json` 的扩展，并按 `id` 去重。

扫描根目录由 `OHMYWORKPANEL_EXTENSION_ROOTS` 提供（`:` 或 `;` 分隔）；PanelLive 根目录（`OHMYWORKPANEL_PANELLIVE_ROOT`，默认 `/AI/WorkPanelLive`）MUST 始终参与且优先；`/AI/AIHotel` 若存在清单则 MAY 自动追加。

#### Scenario: 两个根目录含同 id 扩展
- GIVEN 根 A 与根 B 都含 `id=demo` 的清单
- WHEN 执行发现
- THEN 结果中 `demo` 只出现一次
- AND 使用根 A 的清单

#### Scenario: 根目录缺少清单
- GIVEN 某根目录下没有 `extension.manifest.json`
- WHEN 执行发现
- THEN 该根目录被跳过
- AND 其余根目录正常返回

### Requirement: 清单契约
每个扩展 MUST 提供 `extension.manifest.json`，包含 `id`、`name`、`version`、`kind`、`contributes`、`runtime`。

- `contributes.tabs[]`：`id`、`title`、`route`、`entry`；可选 `peerOf`（群类型白名单）与 `disabledWhenUnloaded`。
- `contributes.a2aSkills[]`：该扩展可接的 A2A 技能名。
- `runtime`：`healthPath`（默认 `/health`）、`defaultPort`（默认 `8790`）、`mediaPlane`（可选）。

#### Scenario: 缺省运行时字段
- GIVEN 清单的 `runtime` 只写了 `defaultPort`
- WHEN 加载清单
- THEN `healthPath` 取 `/health`
- AND 扩展仍可用

#### Scenario: 清单不是合法 JSON
- GIVEN 清单文件内容损坏
- WHEN 加载该扩展
- THEN 返回解析失败
- AND 不影响其他扩展被发现

### Requirement: 同源反代
扩展 UI MUST 通过平台的同源代理访问扩展服务，不得让浏览器直连 `127.0.0.1:<port>`。

平台 MUST 暴露 `/api/extensions/{ext_id}/{*path}`，把请求转发到该扩展的上游，并透传查询串。

#### Scenario: 透传查询参数
- GIVEN 客户端请求 `/api/extensions/panellive/v1/tts?format=json`
- WHEN 代理转发
- THEN 上游收到 `/v1/tts` 且携带 `format=json`

#### Scenario: 非法查询参数
- GIVEN 查询串包含空格的非法值
- WHEN 构造代理路径
- THEN 返回错误
- AND 不发起上游请求

### Requirement: 页签可见性
扩展页签 MUST 按群类型过滤：`peerOf` 为空表示所有群可见；非空时只在与白名单匹配的群出现。

#### Scenario: 限定工作群
- GIVEN 页签 `peerOf=["project"]`
- WHEN 当前群是 `chat`
- THEN 该页签不出现
- AND 其他未限定的页签仍然出现

### Requirement: 状态上报
系统 MUST 能报告每个扩展的 `id`、`name`、`version`、`kind`、`enabled`、`healthy`。

#### Scenario: 扩展健康检查失败
- GIVEN 扩展进程未启动
- WHEN 查询扩展状态
- THEN `healthy=false`
- AND 若页签声明了 `disabledWhenUnloaded` 则入口不可用

## 非目标（当前实现）

- 进程内插件加载、热重载、依赖注入、卸载回滚
- 插件权限与沙箱
- 第三方分发与签名校验
