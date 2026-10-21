# Marketing Specification

## Purpose

让工作群基于绑定仓库的**可追溯证据**生成对外内容，先在群聊里由人审核，再导出。本阶段不连接发布平台。

## Requirements

### Requirement: 证据快照
生成 MUST 基于受限的仓库快照（README / CHANGELOG / docs / commit / diff），并保留证据引用；未提交改动 MUST NOT 默认当作已发布事实。

#### Scenario: 生成快照
- GIVEN 群绑定了 Git 工作区
- WHEN 发起 campaign
- THEN 快照包含可追溯的 commit / 文件引用
- AND 快照大小有上限

#### Scenario: 未提交改动
- GIVEN 工作区存在未提交改动
- WHEN 生成
- THEN 不把改动当作已发布事实
- AND 需要显式选择才纳入

### Requirement: 编排
系统 MUST 先产出 Content Brief，再生成五个渠道草稿，并做事实与风格校验；当证据不足时 MUST 允许返回 `no_content`。

#### Scenario: 正常生成
- GIVEN 足够的证据
- WHEN 编排完成
- THEN 得到 Brief 与五个渠道草稿
- AND 校验结果可读

#### Scenario: 证据不足
- GIVEN 仓库几乎没有可用证据
- WHEN 编排
- THEN 允许返回 `no_content`
- AND 不生成空壳草稿

### Requirement: 群聊审核
campaign MUST 能在群聊中发起、查看、要求修改、批准，并导出 Markdown；修改 MUST 递增 revision。

#### Scenario: 要求修改
- GIVEN 一份草稿
- WHEN 人类要求修改
- THEN revision 递增
- AND 重新生成草稿

#### Scenario: 批准与导出
- GIVEN 草稿已无阻断项
- WHEN 批准并导出
- THEN 得到 Markdown 文本
- AND 状态为已批准

### Requirement: 发布语义
系统 MUST 明确 `publish` 仅表示「值得生成待审草稿」，不代表已经发布到任何平台。

#### Scenario: 语义边界
- GIVEN 一份已批准的 campaign
- WHEN 查看状态
- THEN 不声称已发布
- AND 不包含自动发布副作用

### Requirement: 只读查询
campaign 列表与详情 MUST 按群隔离；跨群 MUST NOT 可见。

#### Scenario: 跨群隔离
- GIVEN 群 A 的 campaign
- WHEN 查询群 B 的列表
- THEN 不包含该 campaign
