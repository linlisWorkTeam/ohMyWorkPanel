# Reference：代码目录组织

目录是本项目最先被阅读的架构文档。维护者应当只看两层目录，就能判断产品有哪些能力、一个改动从哪里开始，以及哪些代码不应互相依赖。

## 组织原则

1. **按产品领域组织**：顶层优先使用 `accounts`、`chat`、`groups`、`members`、`agents`、`workflow`、`extensions`、`observability` 等业务名称，不以 `helpers`、`common`、`misc` 掩盖归属。
2. **领域内局部闭合**：组件、纯逻辑和同目录测试放在一起；修改聊天能力时，主要改动应集中在 `src/chat/` 与对应后端领域。
3. **入口保持薄**：`App.tsx`、`lib.rs` 和 `mod.rs` 只负责组装与公开边界，不继续吸收领域实现。
4. **平台代码才按技术职责组织**：双运行时 API、Tauri stub、数据库连接和 HTTP server 等真正跨领域能力才放平台层。
5. **历史交给 Git**：源码目录禁止新增 `*.bak`、`*.old`、`*_final` 等备份副本。
6. **兼容优先**：目录迁移不得顺带修改 Tauri IPC、Web API 或 SQLite schema；行为变化单独立项。

## 前端目录

```text
src/
├── App.tsx / main.tsx       # 薄入口
├── accounts/                # 登录、邀请、会话
├── agents/                  # Agent 配置、适配器目录、模型目录
├── chat/                    # 消息、提及、历史、输入行为
├── groups/                  # 群设置、工作区选择、群排序
├── members/                 # 成员表单、运行队列投影
├── workflow/                # 版本、Roadmap、Wave、项目管理
├── extensions/              # Extend、Live bridge、Live voice
├── observability/           # 日志、经验、心跳、发布连接状态
├── shell/                   # 主壳组合组件
├── components/ui/           # 无业务含义的 UI 原子组件
├── contrib/                 # UI contribution 契约
└── stubs/                   # Web 构建使用的 Tauri stub
```

业务文件不得重新平铺回 `src/`。新增功能先判断属于哪个领域；只有无法归入任何既有领域、且有独立产品语义时，才新建顶层目录。

## Rust 目录

Cargo crate 入口仍位于 `src-tauri/src/`，但实现按领域下沉：

```text
src-tauri/src/
├── lib.rs / main.rs         # crate 与进程入口
├── accounts/                # 认证、在线状态
├── agents/                  # Agent 配置、模型目录、CLI adapters
├── operations/              # 日志、指标、Ops、保活、发布 drain
├── workflow.rs              # 待后续按领域内部继续拆分
├── extensions.rs            # 待后续按领域内部继续拆分
├── web.rs                   # 待后续拆为领域 routes
├── db.rs                    # 待后续拆为领域 repositories
└── scheduler.rs             # 待后续拆 planner/runner/recovery
```

本轮保留了 `crate::auth`、`crate::adapters`、`crate::metrics` 等兼容 re-export，避免目录治理变成破坏性 API 重构。新代码优先使用 `crate::accounts::*`、`crate::agents::*`、`crate::operations::*` 领域路径。

## 命名

- `*Page`：完整用户页面；`*Panel`：壳层内嵌面板；`*Section`：页面局部；`*Dialog`：弹窗。
- `use*`：React hook；`*Policy`：无副作用决策；`*Service`：领域用例编排；`*Repository`：持久化访问。
- 测试与实现同目录，命名为 `name.test.ts` 或 Rust 文件内 `#[cfg(test)]`。
- 不新增含义模糊的缩写；已存在的 IPC 命名因兼容性保留。

## 新文件放置检查

提交前回答：

1. 文件名是否无需打开即可理解？
2. 父目录是否准确表达它的产品领域？
3. 同一功能的实现和测试是否被无意义地分散？
4. 是否正在扩大入口文件或宽泛目录？
5. 是否能通过移动而不改变运行行为？

若答案不清楚，先调整目录设计再添加实现。
