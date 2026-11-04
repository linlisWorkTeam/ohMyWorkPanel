# Workbench experience delta

## ADDED Requirements

本 capability 新增前台体验契约；既有 workflow/marketing/kernel/extensions/goals/scene 服务行为继续有效。以下 MUST 仅定义目标体验，合并设计文档不代表已交付。

### Requirement: 入场与邀请（S01）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 入场与邀请
- GIVEN 未登录用户持有效邀请，桌面另以无需Web登录启动
- WHEN 用户登录后接受邀请或邀请已经失效
- THEN Web返回原邀请并入群；失效邀请不可接受；桌面不被强制登录

### Requirement: 群作用域导航（S02）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 群作用域导航
- GIVEN 群A已加载且草稿未发，A存在未完成读请求
- WHEN 用户切到群B后A请求才返回
- THEN 只显示B资源；回A恢复其草稿与位置；不得在B显示A消息

### Requirement: 服务器工作区选择（S03）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 服务器工作区选择
- GIVEN 管理员创建project群，浏览器运行在远端电脑
- WHEN 用户选择工作区或创建子目录
- THEN 选择器明确服务器路径；错误保留输入；chat群不强制目录

### Requirement: 成员管理风险（S04）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 成员管理风险
- GIVEN 普通成员与管理员分别查看成员，桌面purge实际非永久删除
- WHEN 尝试删除锁定成员或永久清除
- THEN 显示权限/锁定原因；危险确认注明影响；桌面不以移除冒充永久删除

### Requirement: 消息输入与草稿（S05）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 消息输入与草稿
- GIVEN 输入法处于composing，草稿含引用和成员提及
- WHEN 按Enter或发送失败后切群返回
- THEN composing不发送；提及携带成员ID；失败不丢草稿；保留发送键偏好

### Requirement: 流式与重连（S06）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 流式与重连
- GIVEN 同一消息有多个channel，用户正在查看历史
- WHEN 收到replace增量、最终落地、重复事件并断线重连
- THEN channel独立合并，不重复消息、不强制滚底；重连读回后收敛到服务端事实

### Requirement: 运行与审批（S07）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 运行与审批
- GIVEN run处于awaiting_review，另一个请求取消时网络中断
- WHEN 用户批准或查看取消结果
- THEN 审批与run终态分开；结果未知显示待确认，不自动重试写请求

### Requirement: Agent环境（S08）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: Agent环境
- GIVEN 成员使用catalog提供的远程adapter，凭据已设置
- WHEN 打开配置、自检失败或在桌面请求Web专属能力
- THEN 不丢远程字段；不回显明文凭据；错误可读；不支持与空数据区分

### Requirement: 计划与编排（S09）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 计划与编排
- GIVEN 已有Roadmap/Feature/Task，桌面无编排实现
- WHEN 编辑关联、完成子任务或请求启动编排
- THEN 保留层级关联；写失败保留输入；桌面明确能力限制

### Requirement: 版本交付（S10）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 版本交付
- GIVEN 版本需要管理员Agent，发布可能发生Git冲突
- WHEN 请求Ask、Wave推进或发布
- THEN 无管理员给设置指引；按服务器状态操作；409读回，不宣告tag成功

### Requirement: 目标推进与账本（S11）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 目标推进与账本
- GIVEN Goal active且advance挂起，自动推进可能同时工作
- WHEN 请求超时、切页返回或预算触线
- THEN 同goal本地单飞；先读回；请求取消不称业务停止；终态只读且显示stopReason

### Requirement: 内容审批与导出（S12）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 内容审批与导出
- GIVEN campaign未批准且来源含未提交改动
- WHEN 生成/修订失败或请求导出
- THEN 来源影响可见、保留输入，批准前不提供成功导出

### Requirement: 场景降级与清理（S13）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 场景降级与清理
- GIVEN 场景有20名成员或插件缺失
- WHEN 切群、退出、插件抛错
- THEN 成员不越界；旧作用域释放；提供等价列表，失败退回聊天

### Requirement: 扩展与Live隔离（S14）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 扩展与Live隔离
- GIVEN 扩展经同源代理嵌入，麦克风权限可能拒绝
- WHEN 代理409/502/503或语音失败
- THEN 显示对应恢复动作；聊天仍可发文本；iframe不直连本机端口

### Requirement: 可观测与运维（S15）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 可观测与运维
- GIVEN 普通用户查看Ops，管理员准备灰度，日志含筛选
- WHEN 尝试重复部署或清空日志
- THEN 权限由后端兜底；确认目标；pending阻止重复；不新增生产promote

### Requirement: 主题、响应式与桌面入口（S16）
系统 MUST 支持以下可观察体验，并保持既有服务契约。

#### Scenario: 主题、响应式与桌面入口
- GIVEN 任一七主题、320宽/200%缩放/粗指针，或Web运行时
- WHEN 切主题、键盘导航、打开表单或更新设置
- THEN 聊天流和草稿保留；焦点可见、目标≥44px；Web不显示可用桌面安装

