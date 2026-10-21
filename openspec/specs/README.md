# Capability 清单

行为契约按 capability 组织。状态说明：

- **seeded**：已有 spec，可直接作为契约依据
- **partial**：部分行为已迁入，缺口在文件内标注
- **todo**：行为仍散落在旧文档/代码，等待迁移

| Capability | 覆盖范围 | 代码入口 | 状态 |
|---|---|---|---|
| [kernel](kernel/spec.md) | 服务注册表、依赖声明、可撤销副作用 | `src-tauri/src/kernel/` | seeded（v0） |
| [extensions](extensions/spec.md) | 扩展发现、清单契约、同源反代、页签可见性 | `src-tauri/src/extensions.rs`、`src/extensions/` | seeded |
| accounts | 注册、登录、邀请、会话、在线状态 | `src-tauri/src/accounts/`、`src/accounts/` | todo |
| groups | 建群、工作区绑定、群公告、群类型 | `src-tauri/src/groups/`、`src/groups/` | todo |
| members | 成员增删、模型与 API 配置、队列可见性 | `src-tauri/src/members/`、`src/members/` | todo |
| messages | 消息发送、提及、流式增量、历史窗口 | `src-tauri/src/db.rs`、`src/chat/` | todo |
| runs | 任务调度、串行约束、取消/重试、重启恢复 | `src-tauri/src/scheduler.rs` | todo |
| workflow | Roadmap / Feature / Wave / 版本发布 | `src-tauri/src/workflow.rs`、`src/workflow/` | todo |
| marketing | 内容 campaign 生成、校验、审核、导出 | `src-tauri/src/marketing/`、`src/marketing/` | todo |
| operations | 日志、指标、发布 drain、保活 | `src-tauri/src/operations/`、`src/observability/` | todo |
| agent-runtime | CLI 适配器、模型目录、Connecter 远程 provider | `src-tauri/src/agents/` | todo |

迁移顺序：先迁移正在被改动的 capability，不做一次性重写。新增 capability 必须直接以 spec 形式落地。
