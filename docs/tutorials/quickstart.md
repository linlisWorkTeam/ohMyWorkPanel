# Tutorial：第一次运行 ohMyWorkPanel

[简体中文](quickstart.md) | [English](quickstart.en.md)

本教程带你完成：启动服务、注册或登录、进入工作区、创建群组、发送消息，以及使用 `@` 提及成员。

文中的截图来自当前版本的真实本地 Web 页面。页面文案和布局可能随版本调整；截图用于定位操作区域，不代表所有部署环境都会显示相同的群组和 Agent。

## 你将完成什么

```text
安装依赖 → 启动 Web 服务 → 登录 → 选择工作区 → 创建/进入群组 → 发送消息 → @Agent
```

如果只想确认前端能打开，可以只执行“浏览器开发预览”。如果要注册、登录、创建群组和发送消息，请执行“完整 Web 服务”。

## 1. 准备环境

浏览器开发预览和完整 Web 服务需要：

- Git；
- Node.js 20 或更高版本；
- pnpm。

完整 Web 服务还需要 Rust stable。Tauri 桌面模式另需 Tauri 对应的平台依赖；Windows 还需要 WebView2。

真实 Agent 是可选依赖。没有外部 CLI 时，可以先使用 Mock 适配器验证界面和任务流程。

## 2. 启动 ohMyWorkPanel

### 方式 0：Windows 安装包（推荐）

普通 Windows 用户无需安装 Node.js、Rust 或 pnpm。打开 [v2.1.1 Release](https://github.com/linlisWorkTeam/ohMyWorkPanel/releases/tag/v2.1.1)，下载并运行 `ohMyWorkPanel_2.1.1_x64-setup.exe`。

该安装包包含桌面应用、前端资源、Rust 运行依赖和 WebView2 离线安装器。外部 Agent CLI 不会被打包，使用 Codex、Claude Code、OpenCode、OpenClaw 或 Cursor 前，请在执行任务的机器上单独安装并登录对应 CLI。

### 方式 A：完整 Web 服务（推荐）

此方式包含 Rust 后端、SQLite、登录、群组和 WebSocket，是本教程后续截图对应的运行方式。

```bash
git clone https://github.com/linlisWorkTeam/ohMyWorkPanel.git
cd ohMyWorkPanel
pnpm install
pnpm run build:web
cd src-tauri
cargo run --no-default-features --bin ohmyworkpanel-server
```

服务默认监听 `http://127.0.0.1:8080`。打开浏览器访问：

<http://127.0.0.1:8080>

可以在另一个终端确认服务健康状态：

```bash
curl http://127.0.0.1:8080/api/health
```

预期返回：

```json
{"ok":true,"service":"ohmyworkpanel"}
```

### 方式 B：浏览器开发预览

此方式只启动 Vite 前端，适合前端开发和页面预览，不提供 Rust API 后端。打开页面后，登录、群组和 Agent 功能可能不可用。

```bash
pnpm install
pnpm dev
```

访问 <http://127.0.0.1:1420>。

### 方式 C：Tauri 桌面模式

```bash
pnpm install
pnpm tauri dev
```

桌面模式会启动 Tauri 开发窗口。首次运行前，请确认已安装 Rust stable、Tauri 系统依赖和 Windows WebView2（Windows 环境）。

## 3. 注册或登录

打开 Web 地址后会看到登录页：

![ohMyWorkPanel 登录页](assets/quickstart-login.png)

### 本地演示账号

新数据目录会初始化一个本地演示管理员账号：

```text
用户名：root
密码：root
```

这组账号只适合本机开发和截图演示。部署到共享环境或生产环境后，请立即修改、禁用或移除默认账号，并使用强密码。

### 注册普通用户

点击“没有账号？注册”，填写用户名和密码：

![ohMyWorkPanel 注册页](assets/quickstart-register.png)

注册成功后，普通用户只能看到自己被授权的群组。注册不会自动授予管理员权限，也不会自动创建项目群：

![普通用户的空工作区](assets/quickstart-workspace-empty.png)

如果页面显示“这里还没有群聊”，请联系管理员加入群组；需要创建群组时，请使用管理员账号。

## 4. 登录后认识主界面

使用管理员账号登录后，页面主要分为四个区域：

1. 左侧控制轨：切换主要页面区域；
2. 群组侧栏：查看群组、创建群组和退出登录；
3. 中间工作区：查看消息、任务状态和输入消息；
4. 右侧面板：查看成员、队列、详情和设置。

![登录后的工作区](assets/quickstart-dashboard.png)

右侧面板的“成员”页会显示当前群组中的用户和 Agent。Agent 的“待检测”状态只表示尚未完成环境检测，不代表一定可以执行任务。

## 5. 创建群组

只有管理员可以创建群组。点击左侧“工作区 · 群”旁边的 `＋`，打开“新建群组”表单：

![新建群组表单](assets/quickstart-create-group.png)

按下面的顺序填写：

1. 选择群类型：
   - **项目群**：绑定工作区，可使用项目路线图和编排功能；
   - **聊天群**：不绑定工作区，适合多人和聊天机器人对话。
2. 填写群名称。
3. 填写群主名称。
4. 项目群选择服务器工作目录。
5. 可选：点击预置 Agent 角色右侧的 `＋`，把角色加入新群组。
6. 点击“创建项目群”或“创建聊天群”。

### 工作区路径注意事项

项目群的工作目录必须满足以下条件：

- 是运行 ohMyWorkPanel 的服务器上的绝对路径；
- 目录已经存在，并且服务进程有读写权限；
- 不是浏览器所在电脑的本地路径；
- 不要直接填写示例截图中的路径，按实际服务器系统填写。

例如，Linux 服务器可能使用：

```text
/AI/ohMyWorkPanel
```

Windows 服务器可能使用：

```text
D:\AI\ohMyWorkPanel
```

如果只是想先测试聊天，不想准备工作区，请选择“聊天群”。

## 6. 发送第一条消息

进入群组后，在底部输入框输入消息并按 `Enter`：

```text
你好，请确认当前群组可以正常收发消息。
```

也可以点击“发送”按钮。`Shift+Enter` 用于换行，草稿会自动保存。

![发送消息后的工作区](assets/quickstart-message.png)

上图同时展示了一个真实的 Agent 失败状态：如果群组绑定的 CLI 没有安装、没有登录，或者工作区路径不可用，普通消息仍可能发送成功，但被触发的 Agent 任务会显示失败。请先检查 CLI 和工作区权限，再重试任务。

## 7. 使用 `@` 提及 Agent

在输入框输入 `@`，会出现当前群组成员列表：

![输入 @ 后的成员菜单](assets/quickstart-mention-menu.png)

操作步骤：

1. 输入 `@`；
2. 从菜单中选择要调用的 Agent；
3. 继续输入任务描述；
4. 按 `Enter` 发送。

示例：

```text
@Codex 请检查当前工作区的测试状态，并回复失败项。
```

真实 Agent 执行前，请确认对应 CLI 已安装并登录在运行服务的机器上。不同 CLI 的安装方式、认证方式和参数不同，请参考其官方文档；ohMyWorkPanel 只负责调度和展示结果。

## 8. 添加和检查 Agent

在右侧“成员”面板中可以：

- 查看当前用户和 Agent 成员；
- 邀请成员；
- 打开成员配置；
- 检测 Agent 环境；
- 查看 Agent 的运行状态。

建议按以下顺序接入真实 CLI：

1. 先在服务器终端确认 CLI 可执行；
2. 按 CLI 官方文档完成登录或 API Key 配置；
3. 在面板中添加对应 Agent 适配器；
4. 检测 Agent；
5. 先发送只读任务，例如“列出当前目录文件”，确认执行范围正确后再执行修改任务。

如果不想连接外部服务，优先选择 Mock 适配器完成 UI 和流程验证。

## 9. 任务状态、取消和重试

发送 `@Agent` 任务后：

- 中间区域会显示任务消息和流式状态；
- 右侧“队列”页可以查看排队和运行中的任务；
- 任务失败时，先阅读错误信息，再检查 CLI 登录、可执行文件路径和工作区权限；
- 对可重试的任务使用“重试”；
- 对不应继续执行的任务使用“取消”。

不要因为界面显示 Agent 名称就默认它一定可用。可用性取决于运行服务的机器、CLI 安装状态、登录状态和工作区权限。

## 常见问题

### `pnpm dev` 能打开页面，但登录失败

这是预期行为：`pnpm dev` 只启动前端开发服务器。请改用完整 Web 服务：

```bash
pnpm run build:web
cd src-tauri
cargo run --no-default-features --bin ohmyworkpanel-server
```

### 注册后看不到“新建群聊”按钮

注册用户默认不是管理员。请让管理员把你加入已有群组，或使用管理员账号创建群组。

### 创建项目群时提示“工作目录必须是服务器上的绝对路径”

请检查路径是否为运行 ohMyWorkPanel 的服务器路径、是否以根路径或盘符开头、目录是否存在，以及服务进程是否有权限访问。远程 Web 场景不要填写浏览器本机路径。

### Agent 任务显示“找不到指定的文件”

通常表示工作区不存在、服务进程看不到该路径，或 Agent 的可执行文件路径无效。先在运行服务的同一台机器上检查目录和 CLI：

```bash
# Linux/macOS
command -v codex
ls -ld /path/to/your/workspace

# Windows PowerShell
Get-Command codex
Get-Item D:\path\to\your\workspace
```

### 数据文件在哪里？

可以通过 `OHMYWORKPANEL_DATA_DIR` 指定数据目录。未设置时，Windows 默认使用 `%APPDATA%\ohmyworkpanel`，Linux 默认使用 `$HOME/.local/share/ohmyworkpanel`。修改数据目录前请备份 SQLite 文件。

## 下一步

- [文档首页](../index.md)：按 Tutorials、How-to、Explanation、Reference 浏览；
- [操作指南](../how-to/README.md)：查找具体任务的操作步骤；
- [CLI 参考](../reference/cli.md)：查看适配器和 CLI 相关说明；
- [配置参考](../reference/configuration.md)：查看环境变量和配置项；
- [路线图](../explanation/roadmap.md)：查看正式计划与 Backlog。
