# Workflow Specification

## Purpose

版本与 Wave 工作流：把「这次要做什么」固化成带 Git 坐标的版本，用 Roadmap 说清 What / Who / How，用 Wave 把执行切成可播放的段落，并用 Ask 模式在动手前对齐。

## Requirements

### Requirement: 版本
版本 MUST 属于一个群，包含名称、类型（kind）、状态与 Roadmap 三要素（What / Who / How）；新建版本 MUST 为 `kind=draft`、`status=planning`。

#### Scenario: 新建版本
- GIVEN 一个项目群
- WHEN 创建版本
- THEN `kind` 为 `draft`、`status` 为 `planning`
- AND 出现在该群的版本板上

#### Scenario: 跨群隔离
- GIVEN 群 A 的版本
- WHEN 查询群 B 的版本板
- THEN 不包含群 A 的版本

### Requirement: Roadmap
系统 MUST 允许更新版本的 What / Who / How 与一句话描述；空值 MUST 被允许，视为尚未填写。

#### Scenario: 更新 Roadmap
- GIVEN 一个版本
- WHEN 写入 What / Who / How
- THEN 再次读取返回相同内容

### Requirement: Ask 模式
进入 Ask MUST 把版本置为 `asking` 并记录当前 asking 版本；取消 Ask MUST 退出该状态。Ask 期间 MUST 只允许被提问的版本触发 Agent 运行。

#### Scenario: 进入与取消
- GIVEN 版本为 `draft`
- WHEN 开始 Ask
- THEN 状态为 `asking`
- AND 版本板记录 asking 版本 id
- WHEN 取消 Ask
- THEN 不再有 asking 版本

#### Scenario: 非 Ask 版本
- GIVEN 版本未处于 asking
- WHEN 判断是否允许 Agent 运行
- THEN 拒绝

### Requirement: Wave
批准 Wave MUST 按 Roadmap 生成有序段落；播放/暂停 MUST 反映在状态上。同一版本同一时刻 MUST 只有一个 `running` 或 `paused` 的 Wave。

#### Scenario: 批准
- GIVEN 有 Roadmap 的版本
- WHEN 批准 Wave
- THEN 生成按 idx 排序的 Wave 列表

#### Scenario: 播放与暂停
- GIVEN 一个 Wave
- WHEN 播放再暂停
- THEN 状态依次为 `running`、`paused`

#### Scenario: 单 Wave 播放
- GIVEN 版本已有运行中的 Wave
- WHEN 播放另一个 Wave
- THEN 前者被暂停或拒绝（不允许两个同时运行）

### Requirement: 发布标记
标记发布 MUST 写入 `released` 状态与发布时间，并可携带 Git tag / SHA。

#### Scenario: 标记发布
- GIVEN 一个已批准的版本
- WHEN 标记发布
- THEN 状态为 `released`
- AND `releasedAt` 非空

### Requirement: 版本板聚合
版本板 MUST 聚合 Git 快照、版本列表、Wave 列表、asking 版本、管理员与工作区共享信息。

#### Scenario: 读取版本板
- GIVEN 一个项目群
- WHEN 读取版本板
- THEN 返回上述字段
- AND 不修改任何状态
