# 功能与契约映射

## 来源与冲突处理
代码读取于 b2ce66c33052a3be08b2739ade325df796d08ac1，附件是输入说明，存在简写。HTTP以src/api-web.ts为调用依据，IPC以src/api.ts为调用依据，共享模型以src/types.ts与marketing/types.ts为准。此表是交互到既有边界的映射，非新增API授权。

| 差异 | 基线事实 | 设计处理 |
|---|---|---|
| run 状态 | queued/running/awaiting_review/changes_requested/completed/failed/cancelled/interrupted | 不把附件done/canceled写进请求或枚举 |
| Web与桌面登录 | requiresAuth在Web=true，桌面=false | 桌面不显示强制JWT登录 |
| purge | Web有独立purge；桌面purgeMember当前映射remove_member | 桌面不能声称永久清除 |
| metrics/扩展/模型刷新/编排 | 桌面部分方法空结果或明确不支持 | 能力不可用≠空业务数据 |
| adapter目录 | 支持catalog与connecter-remote等扩展 | 不硬编码仅七种adapter |
| 自动推进 | 后端循环持续工作，UI非调度器 | 关闭页面不等于停止，禁止前端定时调用advance替代 |
| 统计数量 | 附件HTTP/IPC总数为输入标注 | 未逐路由计数，不把109/97作为验证结论 |

## 功能矩阵
权限采用服务端最终判断；P=平台管理员，O=群主，M=普通成员。管理员Agent是执行身份，不等同于当前人类有管理权。UI可预判权限，不替代403处理。未由已读代码证明的细项按服务端响应决定，实施M0须逐路由锁定。
| ID / 页面 | 功能与请求边界 | 权限 / 前置 | 状态与验收 |
|---|---|---|---|
| F01/P01 | auth login/register/verify；invites预览/accept | Web身份；邀请有效 | 登录/失效/返回邀请，S01 |
| F02/P02 | api.bootstrap/getGroupState/markGroupRead；groups列表 | 可见成员关系 | 未读/归档/切群隔离，S02 |
| F03/P02 | createGroup、listServerDir、createServerDir | 建群P；project服务器路径 | 表单校验/错误保留，S03 |
| F04/P02,P04 | addMember/listJoinableUsers/removeMember/purgeMember/setAdmin | 后端管理授权；systemLocked保护 | 人/Agent/chatbot与邀请；危险确认，S04 |
| F05/P02 | 群公告、workspace、archive、成员workspace/model | 按P/O与现有授权 | 服务器路径/越界拒绝；公告注入规则提示 |
| F06/P03 | sendMessage + listMessagesBefore；messages历史 | 当前群发送成员有效 | IME/@/草稿/默认响应者/引用，S05 |
| F07/P03,P04 | getMessageChannelPart/getRunPhases、WS message_delta/message_created/run_status | 懒加载与群作用域 | replace/重连/去重/phase，S06 |
| F08/P04 | cancelRun/retryRun/setRunReview/voteMessage/getMessageFeedback | 资源状态与服务端授权 | 区分审阅和投票，未知结果不重发，S07 |
| F09/P05 | detectAgent、模型catalog/refresh、APIURL/Key、配置status/import/export/install | 全局配置P；平台能力 | 原始错误/凭据不回显；远程provider保留，S08 |
| F10/P06 | list/create/update/delete RoadmapItem/Feature/FeatureTask，getRoadmapState | 现有领域授权 | 关联、完成、排序、删除确认，S09 |
| F11/P06 | startRoadmapItem/list/pause/resume/cancelRoadmapOrchestration | Web能力+执行前置 | Desktop unsupported提示，S09 |
| F12/P06 | getVersionBoard/createProjectVersion/updateVersionRoadmap | 管理授权；工作区 | Git快照/共享告警/What Who How |
| F13/P06 | start/cancelVersionAsk、approveVersionWaves、play/pause/advanceWave、play/pauseVersion | 管理员Agent与当前状态 | 不新增状态机；冲突读回，S10 |
| F14/P06 | releaseVersion（可选tag） | 发布授权与Git条件 | 明确版本发布不等同生产部署，S10 |
| F15/P07 | list/get/create/start/pause/approve/abort/advanceGoal、setGoalAutoAdvance | Goal写P/O；执行必须管理员Agent | 预算/账本/终态/阻塞，S11 |
| F16/P08 | create/list/get/revise/approve/exportMarketingCampaign | 后端授权；planner/writer与证据 | 未批准不可导出，S12 |
| F17/P09 | svc.scene与openGroupScope；成员/run只读投影 | 可选插件 | 20人/切群释放/fail-open，S13 |
| F18/P09 | listGroupExtensions/setExtensionEnabled/setPanelliveEnabled；同源反代 | Web+授权 | 409/502/503局部失败，S14 |
| F19/P03,P09 | OCR/base64、Live语音代理、A2A dispatch | 媒体授权/扩展可用 | 文本输入兜底；不改外部仓，S14 |
| F20/P10 | list/count/clearLogs；save/query/deleteExperience | 后端授权；清空确认 | 作用域/过滤/原始错误，S15 |
| F21/P10 | getMetricsLatest、opsReleaseStatus/Job/testGate/deployCanary/drain | Ops写P；Web能力按实际 | 运行中锁重复操作；无生产promote新入口，S15 |
| F22/P11 | get/updateSettings；theme现有管理；发送键偏好 | 字段按RuntimeSettings | 保存失败保留/切主题保流，S16 |
| F23/P11 | checkForUpdate/downloadUpdate/exitAndInstall | Desktop | 不在Web展示可用安装动作，S16 |

## 保持的协议不变量
- 只调用 api.*，不在组件拼路径、invoke或WS；HTTP路径/方法/字段/鉴权、IPC命令/参数与SQLite均不变。
- Web Bearer与WS query token沿现有实现，不写入错误详情/截图/日志；此设计不新增鉴权机制。
- 错误响应可能纯文本，解析时保留可读内容，不假设全部JSON。
- 消息channel为thinking/artifact/final，replace与seq字段保留，不推导供应商内部思维能力。
- 项目群与聊天群区别保留；普通成员只读Goal，场景只读；禁用/隐藏不是授权实现。
- 公告、成员工作区、systemLocked、已有连接/发版静默策略以代码为准，任何放宽需独立change。

## 实施前必须补齐的证据
逐项对照后端web.rs/commands领域模块核验F04/F05/F09/F10/F12/F16/F20的精确写权限与请求字段；确认Goal各状态允许操作、WS seq作用域、保活写入机制与桌面drain能力。当前文档不编造这些实现细节，实施M0未完成前不得新增按钮调用或扩展后端契约。

