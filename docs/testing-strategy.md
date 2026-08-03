---
date: 2026-08-03
topic: automated-testing-strategy
status: active
---

# LinlisWorkPanel 自动化测试策略

## 目标

在**不碰生产数据 / 不破坏双槽位发布**的前提下，用可重复的自动化门禁挡住关键回归，并把后续补测路径写清楚。

| 原则 | 说明 |
|---|---|
| 金字塔 | 单测为主 → 少量集成 → 极少手工/冒烟 |
| 门禁绑 canary | `deploy-canary.sh` **构建前**强制跑 `scripts/test-gate.sh` |
| 内存受限 | `CARGO_BUILD_JOBS=1`、`NODE_OPTIONS=--max-old-space-size=1024`、`ulimit -v` 约 1.8GB |
| 不测真 CLI | Codex/Claude/Cursor 真机调用走尽力 smoke，**不进门禁** |
| 数据隔离 | 测试只用 tempfile / `data-canary`；永不写 `/AI/LinlisWorkPanel/data` |

## 当前基线（脚手架落地后）

| 层 | 工具 | 现状 |
|---|---|---|
| 前端单测 | Vitest | `mentions` / `messageContent` |
| Rust 单测 | `cargo test --lib` | `message_content` / `adapters` / `parse` / `db` / `scheduler::plan_queued_starts` / `auth` |
| 集成测 | 规划中 | Web API / 全链路 mock run（Phase 2） |
| E2E | 规划中 | Playwright 对 canary（Phase 3，可选） |
| 适配器 smoke | `scripts/smoke-adapters.ps1` | 尽力、不阻塞交付 |
| CI | 无 GitHub Actions | 门禁在本机 canary 部署路径 |
| 产品内 Ops | 项目视图「质量与发布」 | `POST /api/ops/test-gate` / `deploy-canary`；Promote 不进 UI |

## 测试金字塔

```text
        ┌─────────────┐
        │  手工 / 冒烟 │  canary 登录 + mock @；真 CLI 可选
        ├─────────────┤
        │ 集成 (少)    │  Web auth+API、scheduler+mock 执行（Phase 2）
        ├─────────────┤
        │ 单测 (多)    │  Rust 纯逻辑 + Vitest 纯函数  ← 门禁必须绿
        └─────────────┘
```

### L1 — 单元测试（门禁必跑）

**Rust**（`cd src-tauri && cargo test --no-default-features --lib`）

| 模块 | 覆盖重点 | Do not regress |
|---|---|---|
| `message_content` | parts 追加 / legacy 升级 / replace | content JSON 兼容 |
| `adapters::parse` | channel、session_id 启发式 | CLI JSON 漂移默认 final |
| `adapters` | build_args 快照、Cursor 路径候选 | 不改 command 签名 |
| `db` | 中断 run → interrupted；`cli_session_id` helpers | schema 仅加法迁移 |
| `scheduler::plan_queued_starts` | 同 Agent 串行、跨 Agent 可并行、available=0 | 同 Agent 串行语义 |
| `auth` | password / JWT roundtrip | 登录链路基础 |

**前端**（`pnpm test` / Vitest）

| 模块 | 覆盖重点 |
|---|---|
| `mentions.ts` | @ 解析、长名优先 |
| `messageContent.ts` | 与 Rust 对称的 parts 工具 |

新增纯函数优先同目录 `*.test.ts` / `#[cfg(test)]`，不引入重型 UI 测试库（除非 Phase 2 明确需要）。

### L2 — 集成测试（Phase 2，骨架预留）

| 场景 | 建议做法 | 备注 |
|---|---|---|
| Web 登录 + 建群读消息 | `axum` + tempfile DB + `tower::ServiceExt` | 不启真实端口 |
| mock Agent 跑通一轮 | `SchedulerState` + `EventSender::Web` + mock adapter | 短超时；取消 token |
| WS `chat-event` channel | 订阅 broadcast，断言 thinking/final | 跟 v1.4 流式契约 |

放置建议：`src-tauri/tests/` 或模块内 `#[tokio::test]`；仍走 `--no-default-features`。

### L3 — Canary 冒烟（部署后，非门禁编译测）

部署成功后人工或脚本：

```bash
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8081/
# 登录 root/root；mock 成员连续 @ → 串行 + 气泡分区
```

真 CLI smoke：`powershell -File scripts/smoke-adapters.ps1`（缺 CLI → SKIP，exit 0）。

### L4 — E2E（Phase 3，可选）

仅当 Web UI 回归成本明显上升时再上 Playwright，目标环境 **仅 canary `:8081`**，禁止指向生产 `:8080`。

## 覆盖矩阵（优先级）

| ID | 风险点 | 层 | 状态 |
|---|---|---|---|
| R1 | 同 Agent 并行抢跑 | L1 `plan_queued_starts` | 已有骨架 |
| R2 | parts / legacy content 破坏 | L1 message_content 双侧 | 已有 |
| R3 | Cursor `--resume` / session 清空重试 | L1 parse + adapter args | 部分；契约测 Phase 2 |
| R4 | workspace_path=`/` 拒跑 | L1/L2 scheduler/execute | Phase 2 |
| R5 | JWT / 登录失败放开 API | L1 auth + L2 web | L1 已有 |
| R6 | promote 覆盖生产 DB | 发布脚本评审 + 手工清单 | 脚本约束，非单测 |
| R7 | 生产读 workspace `dist` | systemd unit / epitaph | 运维约束 |
| R8 | 适配器 CLI 参数漂移 | L1 快照 + 尽力 smoke | 已有部分 |

## 门禁：绑 `deploy-canary.sh`

```text
deploy-canary.sh
  │
  ├─ scripts/test-gate.sh     ← 失败则中止，不构建、不重启 canary
  │    ├─ pnpm test
  │    └─ cargo test --no-default-features --lib
  ├─ pnpm build:web
  ├─ cargo build --release ...
  └─ 安装到 canary 槽 + systemctl restart
```

| 项 | 约定 |
|---|---|
| 入口 | [`scripts/test-gate.sh`](../scripts/test-gate.sh) |
| 调用点 | [`scripts/deploy-canary.sh`](../scripts/deploy-canary.sh) 构建之前 |
| 跳过 | 仅破窗：`LINLIS_SKIP_TEST_GATE=1`（打印醒目警告；日常禁止） |
| `BUILD=skip` | **仍跑门禁**（产物可跳过编译，质量门禁不跳） |
| 本地等价 | `pnpm run test:gate` 或直接跑 `scripts/test-gate.sh` |

`promote-canary.sh` / `freeze-prod.sh` **不**重复跑完整编译测（假定 canary 已门禁通过）；晋升前仍做 HTTP 冒烟。

## 命令速查

```bash
# 前端
pnpm test

# Rust（与 epitaph / 门禁一致）
cd src-tauri && CARGO_BUILD_JOBS=1 cargo test --no-default-features --lib

# 一键门禁（部署前）
./scripts/test-gate.sh
# 或
pnpm run test:gate

# 灰度（含门禁）
./scripts/deploy-canary.sh
```

## 内存与并发

- 门禁与 canary 构建均：`CARGO_BUILD_JOBS=1`、`NODE_OPTIONS=--max-old-space-size=1024`
- `test-gate.sh` 仅对 **cargo** 子 shell 设 `ulimit -v 1800000`（Vitest/Wasm 需要更大虚拟地址空间，不套同一限制）
- 禁止在门禁里拉起全量 release 构建或并行大型套件

## 目录与命名

| 类型 | 位置 |
|---|---|
| 前端单测 | `src/**/*.test.ts` |
| Rust 单测 | 同模块 `#[cfg(test)]` |
| Rust 集成（未来） | `src-tauri/tests/*.rs` |
| 策略本文 | `docs/testing-strategy.md` |
| 门禁脚本 | `scripts/test-gate.sh` |

## 分期路线

| 阶段 | 内容 | 完成定义 |
|---|---|---|
| **Phase 1（本次）** | 策略文档 + `test-gate.sh` + canary 绑定 + scheduler/auth 骨架 | 门禁绿；`deploy-canary` 失败会阻断 |
| **Phase 2** | Web API 集成、mock 全链路、`/` workspace 拒跑、session 契约 | 关键路径有集成断言 |
| **Phase 3** | 可选 Playwright canary E2E；若上云再补 GHA 镜像同一脚本 | UI 回归可自动捕 |

## Do not regress（测试也要守）

- 不改 `tauri::command` 签名；`ChatEvent` 仅加可选字段
- promote 永不覆盖生产 DB
- 生产不读 workspace `dist/`
- 同 Agent 串行语义
- 门禁与测试不得依赖或写入生产数据目录
