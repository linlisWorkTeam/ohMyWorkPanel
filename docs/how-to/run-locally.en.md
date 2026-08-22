# How-to: Run the Project Locally

[简体中文](run-locally.md) | **English**

## Browser Development Mode

Run these commands from the repository root:

```bash
pnpm install
pnpm dev
```

Open <http://127.0.0.1:1420>. The Vite development server uses a fixed port; stop the process using that port first if it is already occupied.

Stop the service by pressing `Ctrl+C` in the terminal where it is running.

## Tauri Desktop Development Mode

Install stable Rust and the Tauri 2 system dependencies, then run:

```bash
pnpm install
pnpm tauri dev
```

Windows also requires WebView2.

## Enable a Real Agent

Real Agents are not installed as part of this project. Install and authenticate the selected CLI first, then choose the matching adapter in the application.

Adapters visible in the current repository include `mock`, `codex`, `claude-code`, `opencode`, `openclaw`, `cursor`, and `dsh`. The actual executable, authentication requirements, and version compatibility depend on the local environment and the adapter status shown in the application.

<!-- TODO: Add official installation links and minimum verification commands for each CLI after they are confirmed. -->
