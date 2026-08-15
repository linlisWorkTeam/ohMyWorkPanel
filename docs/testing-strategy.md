---
date: 2026-08-05
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
| 内存受限 | `CARGO_BUILD_JOBS=1`、门禁内 `NODE_OPTIONS=--max-old-space-size=768`、cargo 子 shell `ulimit -v` ≈1.8GB |
| 不测真 CLI | Codex/Claude/Cursor 真机调用走尽力 smoke，**不进门禁** |
| 数据隔离 | 测试只用 tempfile / `data-canary`；永不写 `/AI/LinlisWorkPanel/data` |

## 当前基线（2026-08-05 复核）

| 层 | 工具 | 现状（门禁实测） |
|---|---|---|
| 前端单测 | Vitest | **15** 文件 / **53** 用例（见下表；含 queueCounts） |
| Rust 单测 | `cargo test --lib` | **58** 用例（adapters/a2a/db/extensions/memory/metrics/…） |
| 集成测 | 规划中 | Web API / 全链路 mock run（Phase 2）— **仍未落地** |
| E2E | 规划中 | Playwright 对 canary（Phase 3）；本机偶发手工用 headless shell，**未进门禁** |
| 适配器 smoke | `scripts/smoke-adapters.ps1` | 尽力、不阻塞交付 |
| 发布脚本 | canary announce / promote 门禁 | 行为靠脚本+清单；无自动化契约测 |
| CI | 无 GitHub Actions | 门禁在本机 canary 部署路径 |
| 产品内 Ops | 项目视图「质量与发布」 | `POST /api/ops/test-gate` / `deploy-canary`；Promote 不进 UI |

### 前端 Vitest 清单（门禁）

| 文件 | 覆盖重点 | 是否仍贴合代码 |
|---|---|---|
| `mentions.test.ts` | @ 解析、长名优先 | ✅ |
| `messageContent.test.ts` | parts / legacy / 列表投影 | ✅ |
| `messageHistory.test.ts` | 热窗合并、上滑加载 | ✅ |
| `realtimeWs.test.ts` | heartbeat 忽略、`ws_reconnected` resync、退避 | ✅ 基本；未断言 `run_heartbeat` / link pubsub |
| `releasingState.test.ts` | 60s 窗口、**30s 静默横幅** | ✅（2026-08-05 已改） |
| `heartbeatPolicy.test.ts` | Auto 聚焦/后台/内存降档 | ✅ |
| `extensions.test.ts` | PanelLive entry / 同源 baseUrl / 缺 tabs 容错 | ✅ |
| `memberForm.test.ts` | chatbot 槽位、加入已有用户 | ✅ |
| `authSession.test.ts` | 发送者成员解析 | ✅ |
| `agentModels.test.ts` | 模型目录（含 cursor grok **4.6**/4.5、kimi） | ✅ |
| `sendKey` / `chatUi` / `markdownLite` / `roadmapUi` | 发送键、折叠、MD、路线图 UI 纯函数 | ✅ |
| `queueCounts.test.ts` | 同 Agent running/queued 聚合、忙闲文案、展开列表排序 | ✅（2026-08-05） |

**结论（前端用例）**：无发现「断言错误实现」的过时用例；`docs` 旧基线（只写 mentions/messageContent）**已过时**，以本表为准。

### Rust `#[cfg(test)]` 清单（门禁）

| 模块 | 覆盖重点 | 是否仍贴合 |
|---|---|---|
| `adapters::*` / `parse` / `model_catalog` | build_args、Cursor 候选、JSONL、**各适配器 final 文本契约**、Cursor `--list-models` 解析 | ✅ |
| `a2a` | Live skills、禁 PCM | ✅ |
| `extensions` | manifest、enable、proxy path sanitize | ✅ |
| `db` | interrupted runs、joinable user、seed `is_system`、scoped user | ✅ |
| `memory` | 群内锁定 + **种子群跨 workspace** | ✅ |
| `metrics` | classify + latest 缓存 | ✅ 未锁 `PERF_SAMPLE_SECS=20` |
| `scheduler` | 同 Agent 串行、announcement | ✅ 无 `run_heartbeat` / seq 单测 |
| `fs_browse` / `orchestrator` / `auth` / `codex_proxy` | 路径、编排、JWT、shim | ✅ |

**结论（Rust 用例）**：现有断言未与当前行为冲突；缺口主要在「新能力无测」，不是「旧测撒谎」。

## 测试金字塔

```text
        ┌─────────────┐
        │  手工 / 冒烟 │  canary 登录 + mock @；§F 前端壳；真 CLI 可选
        ├─────────────┤
        │ 集成 (少)    │  Web auth+API、scheduler+mock 执行（Phase 2，未做）
        ├─────────────┤
        │ 单测 (多)    │  Rust 纯逻辑 + Vitest 纯函数  ← 门禁必须绿
        └─────────────┘
```

新增纯函数优先同目录 `*.test.ts` / `#[cfg(test)]`，不引入重型 UI 测试库（除非 Phase 2 明确需要）。  
**已知例外**：React hooks 顺序（login→ready / #310）目前靠代码注释 + 手工/Playwright 冒烟，门禁无 RTL 用例。

## 覆盖矩阵（优先级 · 2026-08-05）

| ID | 风险点 | 层 | 状态 |
|---|---|---|---|
| R1 | 同 Agent 并行抢跑 | L1 `plan_queued_starts` | ✅ |
| R1b | 成员栏排队数可见/串计数 | L1 `queueCounts` | ✅ |
| R2 | parts / legacy content 破坏 | L1 message_content 双侧 | ✅ |
| R3 | Cursor `--resume` / session 清空重试 | L1 parse + adapter args | 部分；契约测 Phase 2 |
| R4 | workspace_path=`/` 拒跑 | L1/L2 | Phase 2 |
| R5 | JWT / 登录失败放开 API | L1 auth | L1 ✅；L2 Web 未做 |
| R6 | promote 覆盖生产 DB | 脚本+清单 | 非单测 |
| R7 | 生产读 workspace `dist` | systemd / epitaph | 运维约束 |
| R8 | 适配器 CLI 参数漂移 | L1 快照 + smoke | 部分 |
| R8b | Agent 回显≠预期（OpenClaw stderr / 原始信封） | L1 `resolve_adapter_final_text` + parse 契约 | ✅（2026-08-10） |
| R9 | 种子群 `is_system` / 跨 workspace | L1 db + memory | ✅ |
| R10 | PanelLive 同源 proxy / A2A 禁 PCM | L1 extensions + a2a | ✅；proxy HTTP 集成未做 |
| R11 | 发布断连 60s + **30s 静默横幅** | L1 releasingState | ✅；stub 探活/时序未测 |
| R12 | run_heartbeat / 设置心跳 | L1 heartbeatPolicy | 前端 ✅；后端 emit 未测 |
| R13 | metrics 20s + `/api/metrics/latest` | L1 metrics | 部分；路由集成未做 |
| R14 | React #310（hooks after early return） | 手工 / 未来 RTL | **缺口** |
| R15 | promote stop→start 中断留 dead | 清单+经验 | 非单测（2026-08-05 已踩） |
| R16 | 发版砍 running Agent | L1 drain + 启动 requeue + drain-wait 脚本 | ✅（2026-08-15） |

## 门禁：绑 `deploy-canary.sh`

```text
deploy-canary.sh
  │
  ├─ scripts/test-gate.sh     ← 失败则中止，不构建、不重启 canary
  │    ├─ pnpm exec vitest run --pool=forks --maxWorkers=1
  │    └─ cargo test --no-default-features --lib
  ├─ pnpm build:web
  ├─ cargo build --release ...
  └─ 安装到 canary 槽 + systemctl restart canary
```

| 项 | 约定 |
|---|---|
| 入口 | [`scripts/test-gate.sh`](../scripts/test-gate.sh) |
| 调用点 | [`scripts/deploy-canary.sh`](../scripts/deploy-canary.sh) 构建之前 |
| 跳过 | 仅破窗：`LINLIS_SKIP_TEST_GATE=1`（日常禁止） |
| `BUILD=skip` | **仍跑门禁** |
| 本地等价 | `pnpm run test:gate` |

`promote-canary.sh` / `freeze-prod.sh` **不**重复跑完整编译测；晋升前做 HTTP 冒烟，且 **promote 全程勿中断**（见 `docs/release-checklist.md`）。

灰度推包后另跑：`./scripts/canary-announce-a2a.sh`（流程约定，非门禁）。

## 命令速查

```bash
pnpm test
cd src-tauri && CARGO_BUILD_JOBS=1 cargo test --no-default-features --lib
./scripts/test-gate.sh   # 或 pnpm run test:gate
./scripts/deploy-canary.sh
```

## 内存与并发

- canary 构建：`CARGO_BUILD_JOBS=1`、`NODE_OPTIONS=--max-old-space-size=1024`（deploy 脚本）
- `test-gate.sh`：Vitest 用 `max-old-space-size=768`；仅对 **cargo** 子 shell `ulimit -v 1800000`
- 禁止在门禁里拉起全量并行大型套件

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
| **Phase 1** | 策略 + `test-gate` + canary 绑定 + 核心 L1 | ✅ 门禁绿；deploy 失败会阻断 |
| **Phase 2** | Web API 集成、mock 全链路、`/` workspace 拒跑、session 契约、health/metrics 路由 | 关键路径有集成断言 |
| **Phase 3** | 可选 Playwright canary E2E（登录白屏/#310、releasing 30s 静默） | UI 回归可自动捕 |

## 下次补测建议（按性价比）

1. **高**：`metrics::PERF_SAMPLE_SECS == 20` 常量锁；`ChatEvent` emit 带 `seq` 的纯函数/单测。  
2. **高**：Phase 2 最小集成：`GET /api/health`（无鉴权）+ `GET /api/metrics/latest`（鉴权）。  
3. **中**：Playwright canary：登录后 `#root` 非空（防 #310）；断 WS 前 30s 无横幅文案。  
4. **低**：`canary-announce-a2a.sh` dry-run（mock HTTP）。

## Do not regress（测试也要守）

- 不改 `tauri::command` 签名；`ChatEvent` 仅加可选字段
- promote 永不覆盖生产 DB；promote 勿在 stop/start 间中断
- 生产不读 workspace `dist/`
- 同 Agent 串行语义
- 门禁与测试不得依赖或写入生产数据目录
- 重连横幅：60s 窗口内 **前 30s 静默**
