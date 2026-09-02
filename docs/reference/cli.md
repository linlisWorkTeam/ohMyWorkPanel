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

### Codex 认证模式

`codex` 默认通过本机 Responses 代理使用项目配置的 API Key。若运行服务的系统账号已经通过 Codex CLI 登录，可在启动 ohMyWorkPanel 前设置 `OHMYWORKPANEL_CODEX_NATIVE_AUTH=1`，让适配器保留 CLI 自身的登录 provider：

```powershell
$env:OHMYWORKPANEL_CODEX_NATIVE_AUTH = "1"
pnpm tauri dev
```

Web 服务部署时应把该变量设置在服务进程环境中。两种模式都不会把凭据写入项目仓库；启用前先在同一系统账号下运行一次 `codex exec` 验证登录态。

Windows 上如果成员配置解析到 `codex.cmd` / `codex.bat`，较长的结构化 prompt 仍可能受到 `cmd.exe` 的命令行解析限制。此模式建议把成员的可执行文件显式设置为已安装的原生 `codex.exe` 路径；启动日志和成员检测应显示该 `.exe`，再执行内容活动。

<!-- TODO: 根据项目实际补充每个适配器的版本要求、最小验证命令和参数映射。 -->
