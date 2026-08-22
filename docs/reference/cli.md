# Reference：CLI 与 Agent 适配器

## 内置适配器

当前仓库代码和已有项目文档中出现的适配器如下：

| 适配器 | 可执行文件或方式 | 说明 |
| --- | --- | --- |
| `mock` | 无 | 本地模拟流式回复。 |
| `codex` | `codex` | 需要本机已安装并登录 Codex CLI。 |
| `claude-code` | `claude` | 需要本机已安装并登录 Claude Code CLI。 |
| `opencode` | `opencode` | 需要本机已安装并配置 OpenCode CLI。 |
| `openclaw` | HTTP/配置 | 以项目当前适配器实现和运行环境为准。 |
| `cursor` | `agent`，回退 `cursor-agent` | 需要本机具备可用 Cursor CLI。 |
| `dsh` | `dsh` | 需要安装并配置 DeepSeek Harness。 |

应用还可能显示 chatbot 类 HTTP 适配器；它们不是本地 CLI 插件。

## 选择建议

- 想先验证界面和调度流程：选择 `mock`。
- 想执行真实任务：先单独确认对应 CLI 可以在终端运行，再在应用中选择适配器。
- 未安装、未登录或权限不足时，真实适配器可能无法执行任务。

## 命令参数

不同适配器的参数由仓库中的适配器实现决定，不建议仅凭本文猜测命令行参数。

<!-- TODO: 根据项目实际补充每个适配器的版本要求、最小验证命令和参数映射。 -->
