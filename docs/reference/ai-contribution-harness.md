# Reference：AI 编码与提交 Harness

本规范适用于 AI Agent 编写、修改、审查并提交到本仓库的所有代码。目标不是让 AI 生成更多文字，而是把“先理解、按领域落位、保留兼容、给出证据”变成可执行约束。

## Harness 边界

`scripts/ai-harness.sh` 是 AI 贡献的统一入口：

```bash
# 编码过程中：快速检查目录、备份文件、Markdown 链接和脚本语法
pnpm run check:ai

# 提交前：单独验证计划使用的 commit subject
./scripts/ai-harness.sh commit-message "refactor: split web routes by domain"

# commit 后、创建 PR 前：要求工作树干净并执行完整构建和门禁
pnpm run submit:ai
```

Harness **不会**创建 commit、push、部署 canary、生成生产批准令牌或 promote。AI 必须分别执行这些动作，并遵守发布审批边界。

## AI 编码规范

### 1. 先确定事实和范围

1. 阅读 `docs/version-pipeline.md`、epitaph 索引和与改动相关的最新 active handoff。
2. 确认改动已有流水线归属；没有归属时先补占位，不能用实现倒逼立项。
3. 写清不变量：Tauri IPC、Web API、SQLite schema、生产槽位及其他不能顺带改变的契约。
4. 检查工作树，不能覆盖或“顺手整理”其他人的改动。

### 2. 代码首先给人阅读

- 文件和目录使用产品领域名称；一个功能的实现、测试和小型辅助逻辑尽量局部闭合。
- `App.tsx`、`lib.rs`、`mod.rs` 等入口只组装，不吸收新的领域实现。
- 函数、类型和文件名表达业务意图；禁止用 `data2`、`handlerNew`、`misc`、`helpers` 等转移理解成本。
- 注释说明原因、不变量、外部契约或 workaround 的删除条件，不逐行翻译代码。
- 新增 `TODO` / `FIXME` 必须引用 issue、spec 或文档；不能留下无责任边界的占位。
- 不以格式化整个文件、无关重命名或大面积 import 重排污染功能 diff。
- 不在 import 周围添加 `try/catch`；平台差异使用显式 adapter、alias 或 stub。

### 3. 兼容和安全优先

- 不静默修改 Tauri command 名称/参数、Web 路由/schema 或 SQLite schema。
- 密钥、JWT、CLI 登录态、数据库、生产令牌和构建产物不得进入 Git、测试输出或 PR 正文。
- 不绕过 `test:gate`，不伪造 canary/生产验证，不把“命令已启动”写成“测试通过”。
- 目录迁移使用兼容 re-export 或小步调用方迁移，兼容层的删除必须另行验证。

### 4. 测试与文档随代码同行

- 行为变化先更新用户文档；架构/治理变化更新 explanation/reference 和 active handoff。
- 纯逻辑优先同目录单测；跨运行时契约补集成或契约测试。
- 每个声称“通过”的检查必须有实际退出码 0；警告和环境限制不能伪装成成功。
- 可感知 Web UI 改动除自动化测试外还需要截图或发布清单中的壳层验证。

## AI 提交规范

### Commit

- 一次 commit 只表达一个可评审意图；不混入无关格式化、部署状态或本地文件。
- subject 使用：`type(optional-scope): imperative summary`。
- 允许的 type：`feat`、`fix`、`refactor`、`docs`、`test`、`chore`、`build`、`ci`、`perf`、`revert`。
- subject 最长 72 字符，不以句号等标点结尾。
- commit 前运行相关测试和 `pnpm run test:gate`；commit 后运行 `pnpm run submit:ai`。

### Pull Request

PR 使用 `.github/pull_request_template.md`，必须包含：

1. Motivation：为什么做、对应哪个流水线/spec/issue；
2. Summary：做了什么，不使用“若干优化”等模糊表述；
3. Compatibility：IPC、Web API、SQLite、发布边界；
4. Documentation and handoff：文档与 epitaph；
5. Testing：带状态符号的精确命令；
6. Known limitations：已知限制，没有则写 `None`。

AI 不得声称已创建远端 PR，除非 PR 工具实际返回成功结果；只生成 title/body 时必须明确它是 PR 元数据。

## 自动检查内容

`check` 当前阻断：

- 新业务文件回流 `src/` 或新增 Rust 根级业务模块；
- Git 跟踪的 `*.bak`、`*.old`、`*_final*`；
- 新增且不引用 issue/spec/docs 的源码 TODO/FIXME；
- 失效的 Markdown 相对链接；
- 无法通过 `bash -n` / `node --check` 的脚本。

`submit` 在上述检查之外还会阻断：

- 不符合规范的 HEAD commit subject；
- 未提交或未清理的工作树；
- 颜色纯度、前端构建或完整测试门禁失败。

Harness 是最小可执行底线，不替代代码评审、设计判断、canary 验证或 root 生产批准。
