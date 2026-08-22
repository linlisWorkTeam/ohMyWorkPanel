# ohMyWorkPanel

本地优先的多 Agent 协作面板：把群聊、工作区和 Agent 任务放在同一个界面中。

[![Build](https://img.shields.io/badge/build-not_configured-lightgrey)](#开发者指南)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

## 项目介绍

ohMyWorkPanel 提供 Web 和 Tauri 桌面两种使用方式，用群组管理成员、工作区和任务执行状态。你可以在群聊中直接发送消息，也可以通过 `@成员` 把任务交给已配置的 Agent。

项目使用 SQLite 保存本地应用数据，Agent 由本机已安装并登录的 CLI 或内置 Mock 适配器执行。支持的内置适配器包括：

- Mock；
- Codex CLI；
- Claude Code；
- OpenCode；
- OpenClaw；
- Cursor CLI。

完整命令、配置项和 Web API 请查看 [`docs/`](docs/index.md)，README 只保留入口和最短上手路径。

## 适用场景

- 个人或小团队在同一工作区内协作处理开发任务；
- 需要同时查看聊天、Agent 运行状态和任务结果；
- 想把多个本地 Agent CLI 统一放进一个群组界面；
- 需要在浏览器或 Tauri 桌面应用中使用同一套工作流。

## 边界与限制

- 这不是托管 SaaS，也不提供云端 Agent、云端数据库或官方运维服务；部署、备份和访问控制由使用者负责。
- 项目不会替你安装、登录或购买外部 Agent CLI。使用真实 Agent 前，必须在运行 ohMyWorkPanel 的机器上准备对应 CLI、登录态和必要密钥。
- 项目群的工作区路径是服务器上的绝对路径，不是浏览器所在电脑的本地路径；不要把 `C:\...` 或 `/Users/...` 等客户端路径直接填入远程部署的项目群。
- Agent 任务会使用对应 CLI 及其操作系统权限执行。不要把不可信工作区、密钥文件或生产目录直接交给 Agent；执行破坏性操作前请人工确认。
- Web 构建不是移动端优先界面；手机浏览器可以访问，但窄屏下可能需要横向滚动。
- 本项目不承诺多节点高可用、跨节点状态共识或自动灾备。生产部署请单独设计数据备份、反向代理、TLS 和权限策略。

## 快速上手

### 环境依赖

浏览器开发模式和 Web 服务构建需要：

- Git；
- Node.js 20 或更高版本；
- pnpm。

Tauri 桌面模式和 Rust 服务端还需要 Rust stable。Windows 桌面模式还需要 WebView2。真实 Agent 适配器另需安装并登录相应 CLI；只想验证界面或流程时可以使用 Mock 适配器。

### 方式一：启动浏览器开发界面

适合前端开发和界面预览。该方式只启动 Vite 前端开发服务器；要使用登录、群组和 Agent 等完整功能，请使用下面的 Web 服务或 Tauri 方式。

```bash
git clone https://github.com/linlisWorkTeam/ohMyWorkPanel.git
cd ohMyWorkPanel
pnpm install
pnpm dev
```

打开 <http://127.0.0.1:1420>。

### 方式二：启动完整 Web 服务

这是可在浏览器中完成注册、登录、创建群组和发送消息的最小完整启动方式：

```bash
git clone https://github.com/linlisWorkTeam/ohMyWorkPanel.git
cd ohMyWorkPanel
pnpm install
pnpm run build:web
cd src-tauri
cargo run --no-default-features --bin ohmyworkpanel-server
```

服务默认监听 <http://127.0.0.1:8080>。可以用下面的命令确认服务已启动：

```bash
curl http://127.0.0.1:8080/api/health
```

预期返回：

```json
{"ok":true,"service":"ohmyworkpanel"}
```

然后在浏览器打开 <http://127.0.0.1:8080>，按页面提示注册或登录。

### 方式三：启动 Tauri 桌面应用

```bash
git clone https://github.com/linlisWorkTeam/ohMyWorkPanel.git
cd ohMyWorkPanel
pnpm install
pnpm tauri dev
```

桌面模式需要 Rust stable 和对应平台的 Tauri 系统依赖；Windows 还需要 WebView2。

### 第一次使用

1. 注册或登录账号。
2. 创建项目群，并选择运行服务所在机器上的工作区绝对路径；如果只需要聊天，可创建不绑定工作区的聊天群。
3. 添加用户或 Agent 成员。
4. 发送普通消息，或在消息中使用 `@成员名称` 触发 Agent 任务。
5. 在任务状态和消息流中查看执行结果；必要时取消或重试任务。

完整的逐步教程见 [`docs/tutorials/quickstart.md`](docs/tutorials/quickstart.md)。

### Agent 集成方式

- **Mock**：不依赖外部 CLI，适合验证界面和任务流。
- **本地 CLI**：在运行服务的机器上安装并登录 Codex、Claude Code、OpenCode、OpenClaw 或 Cursor CLI，再在面板中选择对应适配器。
- **配置文件**：需要批量导入或导出 Agent 配置时，使用面板中的 Agent 配置功能；字段和环境变量见 [`docs/reference/configuration.md`](docs/reference/configuration.md)。

不要把 API 密钥、CLI 登录文件或生产数据库提交到 Git。Agent 适配器的具体参数和已知限制见 [`docs/reference/cli.md`](docs/reference/cli.md)。

## FAQ

### 为什么 `pnpm dev` 打开了页面，但登录不可用？

`pnpm dev` 只启动 Vite 前端开发服务器，不会启动 Rust Web 后端。要使用完整功能，请执行 Web 服务启动方式，或直接运行 `pnpm tauri dev`。

### Agent 没有回复，应该检查什么？

确认以下项目：

1. 对应 CLI 已安装在运行 ohMyWorkPanel 的机器上；
2. CLI 已完成登录或配置 API 密钥；
3. 选择的适配器与实际 CLI 一致；
4. Agent 的工作区路径存在，并且服务进程有权限访问。

### Web 部署时应该填写哪里的工作区路径？

填写服务端文件系统中的绝对路径。浏览器所在电脑的本地路径不会自动映射到服务器。

### 数据保存在哪里？

可以通过 `OHMYWORKPANEL_DATA_DIR` 指定数据目录。未设置时，Windows 默认使用 `%APPDATA%\ohmyworkpanel`，Linux 默认使用 `$HOME/.local/share/ohmyworkpanel`。修改数据目录前请先备份 SQLite 文件。

### 如何查看 API、配置和部署说明？

从 [`docs/index.md`](docs/index.md) 进入文档目录。README 不包含完整 API、配置表或内部架构说明。

---

## 开发者指南

开发环境、提交规范和 PR 流程见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。常用命令如下：

```bash
# 安装依赖并启动前端开发服务
pnpm install
pnpm dev

# 前端类型检查与生产构建
pnpm build

# 纯 Web 构建
pnpm run build:web

# 前端单元测试
pnpm test

# Rust 单元测试
cd src-tauri
cargo test --no-default-features --lib

# Rust 服务端编译检查
cargo check --no-default-features --bin ohmyworkpanel-server
```

提交 PR 前请：

1. 从最新默认分支创建独立分支；
2. 只提交与问题相关的代码或文档；
3. 运行与改动相关的测试，并在 PR 描述中记录命令和结果；
4. 不提交密钥、登录态、SQLite 数据库、构建产物或本地运行目录；
5. 行为发生变化时同步更新用户文档和 [`CHANGELOG.md`](CHANGELOG.md)。

测试策略、发布检查和完整参考资料位于 [`docs/`](docs/index.md)。

## License

本项目使用 [MIT License](LICENSE)。你可以自由使用、修改、分发和商业使用，但必须保留原版权声明和许可证文本。
