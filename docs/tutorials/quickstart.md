# Tutorial：第一次运行 ohMyWorkPanel

本教程使用浏览器开发模式，完成安装、启动、登录和第一次群聊。它不要求配置真实 Agent CLI；如果只想确认界面能启动，做到第 3 步即可。

## 1. 准备环境

- Node.js 20 或更高版本；
- pnpm；
- Git。

桌面模式另需 Rust stable 和 Windows WebView2。本教程先使用浏览器模式，因此可以暂不安装 Rust。

## 2. 获取代码并安装依赖

```bash
git clone https://github.com/linlisWorkTeam/ohMyWorkPanel.git
cd ohMyWorkPanel
pnpm install
```

如果仓库目录名称不同，请把 `cd ohMyWorkPanel` 替换为实际目录。

## 3. 启动开发服务

```bash
pnpm dev
```

服务默认监听 `http://127.0.0.1:1420`。在浏览器打开该地址。

## 4. 创建第一个群组

1. 按页面提示注册或登录。
2. 创建一个群组。
3. 为群组选择工作区。
4. 添加用户或 Agent 成员。
5. 发送一条普通消息，确认聊天流程可用。

在 Web 部署场景中，工作区路径指服务器上的绝对路径；不要填写浏览器所在电脑的本地路径。

## 5. 触发 Agent 任务（可选）

先在运行 ohMyWorkPanel 的机器上安装并登录一个受支持的 CLI，再在群聊中使用其成员名称进行 `@` 提及：

```text
@agent 请检查当前工作区并回复一行摘要
```

真实 CLI 的名称、登录方式和可用模型由对应工具提供商决定。

<!-- TODO: 根据项目实际补充一个不依赖外部账号的 mock Agent 演示。 -->

## 6. 启动桌面模式（可选）

关闭浏览器开发服务后，在仓库根目录运行：

```bash
pnpm tauri dev
```

桌面开发使用同一个前端开发服务，但需要本机具备 Tauri 2 的系统依赖。

## 下一步

- 常见操作：[`docs/how-to/README.md`](../how-to/README.md)
- 命令和适配器：[`docs/reference/cli.md`](../reference/cli.md)
- 路线图：[`docs/explanation/roadmap.md`](../explanation/roadmap.md)
