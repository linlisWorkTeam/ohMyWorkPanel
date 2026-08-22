# ohMyWorkPanel

Local-first multi-agent collaboration panel: bring group chat, workspaces, and Agent tasks into one interface.

[简体中文](README.md) | **English**

[![Build](https://img.shields.io/badge/build-not_configured-lightgrey)](#developer-guide)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

## Overview

ohMyWorkPanel provides both Web and Tauri desktop modes. Use groups to manage members, workspaces, and task execution status. Send messages in a group, or use `@member` to assign work to a configured Agent.

The project stores local application data in SQLite. Agents run through locally installed and authenticated CLI tools or the built-in Mock adapter. Built-in adapters currently include:

- Mock;
- Codex CLI;
- Claude Code;
- OpenCode;
- OpenClaw;
- Cursor CLI.

For complete commands, configuration, and Web API details, see the [`docs/`](docs/index.en.md). This README only provides the project entry point and the shortest path to a working setup.

## Use Cases

- Personal or small-team collaboration in a shared workspace;
- Viewing chat, Agent execution status, and task results together;
- Bringing several local Agent CLIs into one group interface;
- Using the same workflow through a browser or Tauri desktop app.

## Scope and Limitations

- This is not a hosted SaaS. It does not provide cloud Agents, cloud databases, or managed operations. Deployment, backups, and access control remain the operator's responsibility.
- The project does not install, authenticate, or purchase external Agent CLIs for you. Before using a real Agent, prepare its CLI, login session, and required credentials on the machine running ohMyWorkPanel.
- A project group's workspace path is an absolute path on the server, not a local path on the browser computer. Do not enter a client path such as `C:\...` or `/Users/...` for a remote deployment.
- Agent tasks run with the permissions of the selected CLI and operating system. Do not give an Agent an untrusted workspace, secret files, or a production directory without human review.
- The Web build is not mobile-first. A phone browser may work, but narrow screens can require horizontal scrolling.
- The project does not promise multi-node high availability, cross-node consensus, or automatic disaster recovery. Design backups, reverse proxy, TLS, and access controls separately for production.

## Quick Start

### Prerequisites

Browser development and the Web service build require:

- Git;
- Node.js 20 or newer;
- pnpm.

Tauri desktop mode and the Rust server additionally require stable Rust. Windows desktop mode also requires WebView2. Real Agent adapters require the corresponding CLI to be installed and authenticated; use Mock if you only want to verify the UI or workflow.

### Option 1: Browser development preview

This is suitable for frontend development and visual preview. It only starts the Vite frontend; use the Web service or Tauri mode for login, groups, and Agents.

```bash
git clone https://github.com/linlisWorkTeam/ohMyWorkPanel.git
cd ohMyWorkPanel
pnpm install
pnpm dev
```

Open <http://127.0.0.1:1420>.

### Option 2: Complete Web service

This is the minimum complete browser setup for registration, login, groups, and messages:

```bash
git clone https://github.com/linlisWorkTeam/ohMyWorkPanel.git
cd ohMyWorkPanel
pnpm install
pnpm run build:web
cd src-tauri
cargo run --no-default-features --bin ohmyworkpanel-server
```

The service listens on <http://127.0.0.1:8080> by default. Check its health endpoint:

```bash
curl http://127.0.0.1:8080/api/health
```

Expected response:

```json
{"ok":true,"service":"ohmyworkpanel"}
```

Open <http://127.0.0.1:8080> and register or sign in.

### Option 3: Tauri desktop app

```bash
git clone https://github.com/linlisWorkTeam/ohMyWorkPanel.git
cd ohMyWorkPanel
pnpm install
pnpm tauri dev
```

## Basic Usage

1. Register or sign in.
2. Create a project group with an absolute workspace path on the server, or create a chat group without a workspace.
3. Add users or Agent members.
4. Send a normal message, or use `@member name` to trigger an Agent task.
5. Follow task status and message updates; cancel or retry when appropriate.

The complete visual walkthrough is available in the [Quick Start tutorial](docs/tutorials/quickstart.en.md).

### Agent Integration

- **Mock**: no external CLI required; useful for verifying the UI and task flow.
- **Local CLI**: install and authenticate Codex, Claude Code, OpenCode, OpenClaw, or Cursor CLI on the machine running the service, then select the matching adapter.
- **Configuration files**: use the Agent configuration feature for batch import/export. See [`docs/reference/configuration.md`](docs/reference/configuration.md) for fields and environment variables.

Never commit API keys, CLI login files, or production databases. See [`docs/reference/cli.md`](docs/reference/cli.md) for adapter parameters and known limitations.

## FAQ

### The page opens with `pnpm dev`, but login does not work. Why?

`pnpm dev` only starts the Vite frontend and does not start the Rust Web backend. Use the complete Web service option or run `pnpm tauri dev`.

### An Agent does not reply. What should I check?

Confirm that the CLI is installed on the machine running ohMyWorkPanel, authentication is complete, the selected adapter matches the CLI, and the Agent workspace exists and is accessible to the service process.

### Which workspace path should I enter for a Web deployment?

Enter an absolute path on the server filesystem. A path on the browser computer is not automatically mapped to the server.

### Where is application data stored?

Set `OHMYWORKPANEL_DATA_DIR` to choose the data directory. If unset, Windows uses `%APPDATA%\ohmyworkpanel` and Linux uses `$HOME/.local/share/ohmyworkpanel`. Back up SQLite files before changing the data directory.

### Where are the API, configuration, and deployment details?

Start at the [English documentation index](docs/index.en.md). The README does not contain the complete API, configuration tables, or internal architecture documentation.

---

## Developer Guide

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the development environment, commit conventions, and PR workflow. Common commands:

```bash
# Install dependencies and start the frontend
pnpm install
pnpm dev

# Type-check and production build
pnpm build

# Web-only build
pnpm run build:web

# Frontend unit tests
pnpm test

# Rust unit tests
cd src-tauri
cargo test --no-default-features --lib

# Rust server compile check
cargo check --no-default-features --bin ohmyworkpanel-server
```

Before opening a PR:

1. Create a topic branch from the latest default branch;
2. Submit only changes related to the issue;
3. Run relevant tests and record the commands and results in the PR description;
4. Do not commit secrets, login sessions, SQLite databases, build output, or local runtime directories;
5. Update user documentation and [`CHANGELOG.md`](CHANGELOG.md) when behavior changes.

## License

This project is available under the [MIT License](LICENSE). You may use, modify, distribute, and commercially use it, provided that the copyright and license text are retained.
