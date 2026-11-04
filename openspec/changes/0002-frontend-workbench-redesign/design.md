# 架构与迁移设计

## 1. 事实与复杂度
基线 App.tsx 约 94,089 个字符，已经引用 Shell、ChatTranscript、Composer、Roster、GoalStrip 与各领域面板；项目已有 accounts/chat/groups/members/agents/workflow/marketing/goals/scene/extensions/observability 等领域目录。package.json 仅依赖 React 与 Tauri 基础 API，没有状态框架或路由库。复杂度主要来自并发运行、实时增量、权限、多群作用域、插件生命周期及双传输，并非页面数量本身。

推荐「领域模块化单体 + 显式用例 hook + 可插拔视图」，沿现有目录演化。规模不足以承担微前端、多包 monorepo、完整 DDD 实体仓储分层或整站 Vue 重写的迁移成本。采用 UI 重写、领域能力复用的路线。

## 2. 架构决策
| 决策 | 推荐 | 原因 / 重新评估条件 |
|---|---|---|
| 视图库 | 保留 React 19 / TS 5.8 / Vite 7 | 双端构建、插件与测试已有基础；换栈不能解决状态归属 |
| UI 基础 | 扩展 PageLayout 与现有 UI 原子组件 | 保留语义 token、可访问性与门禁；不并行引入第二套主题系统 |
| 状态 | React 局部 state/reducer + 小范围 context | 先拆所有权；仅当跨域订阅性能有测量证据时再评估外部 store |
| 服务器数据 | 领域 query hook，经 api.* 请求 | 暂不引入 query 库；读模型按 groupId、资源 id 缓存 |
| 导航 | shell 的显式 view 状态与可序列化位置 | 登录/邀请保持已有入口；深链接/Router 作为独立后续方案，不先假定服务器 history fallback |
| 插件 | 复用 kernel / contrib / svc.scene | 不造第二个插件宿主；失败局限在插件边界 |
| 样式 | styles.css / themes.css / shell token SSOT | 先归整现有职责；拆 CSS 文件另立小步变更，禁止源码迁移同时换 palette |

## 3. 目标目录与边界（建议新增/迁移，不是本 PR 实现）
| 目录 | 责任 | 迁移来源 |
|---|---|---|
| shell/ | WorkbenchShell、GroupHeader、GroupNavigation、Inspector、位置恢复 | App 的布局与导航，保留 Shell 兼容出口 |
| accounts/ | 会话、登录、注册、邀请、退出清理 | App 登录分支与已有 InviteLanding/authSession |
| chat/ | ChatPage、MessageItem、RunDetails、Composer、消息/运行 reducer 与订阅 | shell/ChatTranscript、shell/Composer、App 消息逻辑 |
| groups/ | 群列表、建群、公告/工作区/归档 | App 群处理与 GroupSettingsView |
| members/ | 成员列表、详情、增删/邀请、队列投影 | shell/Roster、furniture 中成员/队列 |
| agents/ | 配置、自检、模型与 CLI 环境 | AgentConfigView 与现有 catalog |
| workflow/ | DeliveryPage、RoadmapPanel、VersionPanel、Ask/Wave/Release | 复用 VersionView/ProjectWorkflowView/PmPanel |
| goals/ | GoalsPage、GoalSummary、RoundLedger、推进用例 | GoalStrip，hooks/useGoalBar 的版本条归 workflow |
| marketing/ | campaign 列表/详情/修订/批准/导出 | 既有 MarketingMessage 与 API |
| observability/ | 连接管理、日志、经验、指标与 Ops 面板 | 既有 ws/heartbeat/releasing 与 App 定时器 |
| scene/、extensions/ | 可选视图、只读投影、同源嵌入、资源释放 | 保留当前插件生命周期 |
| components/ui/ | 无领域权限判断的 UI 原子 | 继续复用 Modal/FormField/Badge/Toast/ContextMenu |

App 最终只装配会话、主壳、领域视图与错误边界；目标 <250 有效代码行是评审提示，不通过压缩代码满足。纯逻辑测试与领域同目录，禁止新增 misc/helpers/common 收容目录。api.ts/api-web.ts/types.ts 继续作为兼容入口，不批量重命名。

依赖方向：App → shell 与领域公开入口；领域 → api/type/UI/自身纯逻辑；shell 仅编排领域公开接口，不读取领域内部 store；UI 原子不 import 领域；scene 从只读投影取数。跨域协作以 ID、回调和只读 selector 交换，不直接修改对方状态。聊天消息与 run 的事实由 chat 维护，成员队列与场景仅派生，Goal 轮次事实由 goals 维护，不合并为 run 状态。

## 4. 状态所有权
| 状态 | 唯一所有者 | 生命周期 / key |
|---|---|---|
| 登录 token、账号 | accounts，沿用传输层存取 | 用户退出清除，禁止出现在日志 |
| 当前群/页签/右栏/滚动锚点 | shell | userId + groupId；切群恢复位置 |
| 消息、run、流片段 | chat | groupId/messageId/channel；未加载的 channel 不伪造内容 |
| 版本、Wave、Roadmap | workflow | groupId/versionId；写后读回 |
| Goal、轮次、推进状态 | goals | groupId/goalId；轮次以服务端为准 |
| 表单未提交内容 | 对应领域 | dialogId/resourceId；失败保留输入 |
| 主题 | 既有 theme 管理 | 保留现有存储 key、data-theme 值与 palette |
| 草稿 | chat 草稿 hook | 服务地址 + userId + groupId；退出清除该账号缓存 |

## 5. 实时与并发
1. Web 复用当前事件适配层，桌面复用 Tauri listen；同一应用仅一个物理订阅入口，按 groupId 路由，注销函数在退出/卸载执行。页面不可自行再开 WS。
2. HTTP 初始快照与事件按资源作用域合并；群切换令 generation 递增，旧 generation 的返回值不得覆盖当前群。AbortController 只是取消等待，仍需 generation 防竞态。
3. message_delta 按 messageId + channel 应用，replace=true 覆盖；seq 仅在确认服务端序列作用域后去重，缺 seq 不按文本重复猜测丢包。未知事件忽略并可诊断。
4. 重连时缓冲当前群增量，重新取群快照与当前领域资源；快照完成后合并确定未覆盖的事件。无法判定覆盖区间则再次拉取，不把历史 delta 无条件追加到完整文本。
5. status 以实际枚举为准，见功能矩阵；服务端终态与持久化消息读回覆盖瞬时流。run 心跳不产生另一个任务记录。
6. 禁止自动重试非幂等写操作（发送、推进、发布、部署）。网络中断显示「结果待确认」，先读回匹配资源；缺少服务端幂等键时不能承诺恰好一次。
7. Goal advance 的 HTTP 等待可达数分钟；离开页面不等于停止 Goal。前端中止请求不等于后端终止；通过既有 pause/abort 表达业务操作并读回状态，禁止杜撰 cancel-advance API。
8. 活动操作在本地按 resourceId 单飞。其他标签页/自动推进仍可能竞争，409 后重新读回，不乐观伪造成功。

## 6. 双端能力与失败边界
业务组件始终调用 api.*。推荐由现有 adapter 导出的本地 capability 描述控制「仅 Web」提示，保持后端 schema 不变；需要新增导出时两端同时实现，并对 build alias 做契约检查。UI 不从 user-agent 猜权限。
登录、邀请、扩展宿主、模型刷新、metrics、编排及 purge 存在实际双端差异（见矩阵），不可承诺桌面完全等价。

最外层错误边界提供重载与错误摘要；领域错误边界只替换领域区域；scene/iframe 出错回到聊天。扩展故障不能重置聊天或 WS。桌面 updater 仍调用既有 API，不在 Web 展示安装器按钮。

## 7. 迁移顺序与回退
- M0：锁定契约与核心回归截图，记录当前七主题和双端能力；建立变更清单。
- M1：先拆状态与 subscription，视觉暂不变；保留旧 Shell 入口，验证无重复发送/监听。
- M2：替换 shell 与 chat；受控本地前台开关默认为旧壳，按群恢复草稿/位置。
- M3：依次交付 workflow/goals → members/agents → marketing/observability/settings。
- M4：接回 scene/extension/Live；切群、卸载、重连、移动端验收。
- M5：旧/新壳基于相同数据与 API 比较，达到 acceptance 后切默认；至少一个发布周期保留旧壳回退。
开关是前端设计建议，不新增后端设置字段。两个壳不可同时挂载业务副作用。回退只换前端产物，不回滚数据库，不自动部署或 promote。

## 8. 本设计审阅点
允许信息架构改版，但既有默认响应者、发送键偏好、引用、Live 语音、桌面更新、Connecter 远程 Agent 都不得因主功能表未详细介绍而被删除。若实现需新增后端能力，另开 change；本设计不授权后端修改。

