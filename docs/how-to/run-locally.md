# How-to：本地运行项目

## 浏览器开发模式

在仓库根目录执行：

```bash
pnpm install
pnpm dev
```

然后打开 <http://127.0.0.1:1420>。Vite 开发服务器监听固定端口；如果该端口已被占用，先停止占用它的进程。

停止服务：在运行命令的终端按 `Ctrl+C`。

## Tauri 桌面开发模式

准备 Rust stable 和 Tauri 2 系统依赖后执行：

```bash
pnpm install
pnpm tauri dev
```

Windows 还需要 WebView2。

## 启用真实 Agent

真实 Agent 不是项目安装步骤的一部分。请先安装并登录所选 CLI，再在应用中选择对应适配器。

当前仓库代码中可见的适配器包括 `mock`、`codex`、`claude-code`、`opencode`、`openclaw`、`cursor` 和 `dsh`；实际可执行文件、登录要求和版本兼容性以本机环境及应用中的适配器状态为准。

<!-- TODO: 根据项目实际补充各 CLI 的官方安装链接和最小验证命令。 -->
