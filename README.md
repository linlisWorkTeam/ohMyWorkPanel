# LinlisWorkPanel

本地优先的多 Agent 协作面板：把工作群、工作区和 Agent 任务放在同一个界面中。

## Badges

[![Build](https://img.shields.io/badge/build-not_configured-lightgrey)](#)
[![License](https://img.shields.io/badge/license-not_declared-lightgrey)](#)

## Features

- 群聊与工作区绑定。
- 在消息中使用 `@` 触发 Agent 任务。
- 支持 mock、Codex、Claude Code、OpenCode、Cursor 等适配器；实际可用性取决于本机 CLI 和登录状态。
- 支持 Web 开发模式与 Tauri 桌面开发模式。
- 使用 SQLite 保存本地应用数据。

## Quick Start

要求：Node.js 20+、pnpm。只运行浏览器开发模式时不需要 Rust。

```bash
pnpm install
pnpm dev
```

打开 <http://127.0.0.1:1420>，按页面提示注册或登录，然后创建群组并开始使用。

完整的可复制入门流程见 [`docs/tutorials/quickstart.md`](docs/tutorials/quickstart.md)。

## Installation

安装依赖：

```bash
pnpm install
```

启动 Tauri 桌面开发模式：

```bash
pnpm tauri dev
```

桌面模式还需要 Rust stable、Tauri 2 所需系统依赖；Windows 需要 WebView2。Agent CLI 是可选依赖，未安装或未登录时对应适配器不能执行真实任务。

## Basic Usage

1. 启动 Web 或桌面开发模式并登录。
2. 创建群组，选择要绑定的工作区。
3. 添加用户或 Agent 成员；需要真实 CLI 执行时，先在本机安装并登录对应 CLI。
4. 在群聊中发送普通消息，或使用 `@成员` 触发 Agent。
5. 在任务状态和消息流中查看执行结果；必要时使用取消或重试操作。

Web 部署场景中的工作区路径必须是服务器绝对路径，不是浏览器所在电脑的路径。

## Documentation

- [文档首页](docs/index.md)：按 Tutorials、How-to、Explanation、Reference 分类浏览。
- [完整入门教程](docs/tutorials/quickstart.md)
- [操作指南](docs/how-to/README.md)
- [路线图](docs/explanation/roadmap.md)
- [参考手册](docs/reference/README.md)
- [现有 Web API 薄索引](docs/api-web.md)
- [测试策略](docs/testing-strategy.md)
- [发布检查清单](docs/release-checklist.md)

## Roadmap

路线图位于 [`docs/explanation/roadmap.md`](docs/explanation/roadmap.md)。正式计划使用预估季度，Backlog 项目不会被视为已承诺交付。

## Changelog

变更记录见 [`CHANGELOG.md`](CHANGELOG.md)，遵循 Keep a Changelog 与语义化版本约定。

## Contributing

请阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md)。提交前至少运行与改动相关的测试；文档改动也请检查链接和命令是否仍可复制执行。

## License

当前仓库未声明许可证。

<!-- TODO: 根据项目实际补充 LICENSE 文件、版权主体和许可证链接。 -->
