---
date: 2026-08-21
topic: cli-adapter-manifest
spec: true
status: accepted
track: A
---

# CLI 适配器 Manifest（插件化接入）— 锁定设计

> **本文是轨道 A「内部多 CLI 插件化」的 SSOT。** 实现必须按切片灰度；禁止一次拆光 `AdapterKind`。  
> 动机：公司内有多类自研/采购 CLI，不能每接一家就改枚举并发版。  
> 宿主：**只在 ohMyWorkPanel 适配器层**；不放 IM connector，不放 Extend 页签。

## 1. 锁定结论（勿漂移）

| # | 决定 |
|---|---|
| L1 | 插件化做在 **ohMyWorkPanel**，不是 connector、不是 Extend |
| L2 | CLI 唯一 spawn 入口仍是 **`adapters::run_streaming`**（cwd=群工作区、取消、drain、同 Agent 串行） |
| L3 | **终态全部 CLI 声明化**：随包或 `OHMYWORKPANEL_ADAPTER_ROOTS` 下的 `*.adapter.json`；枚举只当迁移 fallback，迁完删除 |
| L4 | **异步切片**到达终态；每一刀独立灰度；同名 json 覆盖内置，稳了再删代码 |
| L5 | argv **只允许数组 + `{prompt}`/`{model}`/`{session}` 占位符**；禁止 `sh -c` / 任意 shell 字符串 |
| L6 | **`mock` 保留**，不进 CLI manifest（不 spawn，默认体验 / 测试 / 新建成员缺省） |
| L7 | **chatbot 与 CLI 调度平级、执行不平级**：`kind=chatbot` 走 HTTP，不进 `*.adapter.json` |
| L8 | 不把 dsh 内核编进本仓；配置包不携带第三方二进制或登录态 |
| L9 | P0 未落地前，现网 `@Cursor`/`@OpenCode` 零回归（内置 fallback） |

## 2. 成员模型 vs 执行入口

成员 `kind` 三选一，**调度上平级**（都能 `@`、进 `schedule_group`）：

```text
user | agent | chatbot
         │        │
         ▼        ▼
    CLI 执行面   HTTP 执行面
    run_streaming  chatbot::run_chatbot_completion
    （含 mock 短路）
```

| | CLI Agent（`kind=agent`） | Chatbot（`kind=chatbot`） | mock |
|---|---|---|---|
| 入口 | `resolve_adapter` → `run_streaming` | `run_chatbot_completion` | `run_mock_stream` |
| 工作区 | 要 | 不要 | 不要 |
| 插件化 | **本文范围** | 否 | 否（留内核短路） |
| 群 | 工作群为主 | 仅聊天群可建 | 任意 |

`adapter` 字符串缺失时回落 `"mock"`。`chatbot-*` 即使误标在 agent 上，调度仍走 chatbot 快路径（现有 `is_chatbot_adapter`）。

## 3. ohMyWorkPanel vs connector

按「这条程序在干什么」分类，不按「都叫 CLI」。

```text
外部 IM / 开放平台          工作区本机进程
QQ / 微信 / Welink     ≠    acme / cursor / opencode
        │                           │
        ▼                           ▼
   connector（轨道 D）         ohMyWorkPanel adapters（轨道 A）
   统一消息进出群              spawn + 流式回气泡 + 取消
```

| 判据 | ohMyWorkPanel Manifest | connector（尚未实现） |
|---|---|---|
| 触发 | 群里 `@成员` | 平台 webhook / 长连接推消息进群 |
| 工作区 | `cwd=群 workspace` | 无业务 workspace |
| 输出 | stdout → Agent 气泡 | 外部消息 → 群聊用户消息 |
| 文档 | 本文 | `plans/2026-08-05-chatgroup-platform-mapping-research.md` |

Welink/QQ 同步桥 → connector。内部 coding/ops CLI → 本文。HTTP 无工作区 → chatbot 或 A2A。会说 ACP → 轨道 G，不是 connector。

**两边都不做**：页签 UI（Extend）；dsh 内核；登录态打包。

## 4. 发现与解析

| 来源 | 说明 |
|---|---|
| `OHMYWORKPANEL_ADAPTER_ROOTS` | `:` / `;` 分隔目录，扫描 `*.adapter.json` |
| 随仓 `adapters/*.adapter.json` | 迁出的内置（OpenCode 起） |
| 代码枚举 | **仅迁移期 fallback**；同名以文件为准 |

重启 Web 后生效（P0 不做进程内热替换）。`resolve_adapter(id)`：文件表 → 枚举 → 未知则 4xx。前端 `GET /api/adapters` = 内置 ∪ 扫描；`<select>` 不再写死列表（P0 实现时改）。

## 5. Manifest 形状

### P0（新内部 CLI 够用）

```jsonc
{
  "id": "acme-cli",                 // 成员 adapter；[a-z0-9-]+
  "displayName": "Acme CLI",
  "executables": ["acme", "acme-cli"],
  "args": ["run", "{prompt}", "--json"],
  "stream": "jsonl",                // jsonl | plain | cursor-stream-json
  "resumeFlag": null,
  "modelFlag": "--model",
  "timeoutSecs": 600
}
```

占位符只替换为**单独 argv 元素**。

| `stream` | 行为 |
|---|---|
| `jsonl` | 现有 `parse_agent_event` |
| `plain` | 整行当 `final` 增量 |
| `cursor-stream-json` | 复用 Cursor 解析 |

### P1 增补（为迁 Cursor/Codex）

- `session: persist`：调度读写 CLI session、短 resume prompt（今 Cursor 特例）
- `envFrom: memberApiKey`：注入环境变量（今 Codex `OPENAI_API_KEY`）
- `stream: openclaw-json`：多行 JSON 缓冲（今 OpenClaw 特例）

这些字段是**把内核特例搬进契约**，不是永久留在 `match kind`。

## 6. 时序（CLI 路径，终态与现状相同入口）

```text
User → UI → POST /api/messages → DB 入队 → schedule_group
  → run_agent
      ├ chatbot? → HTTP 补全
      ├ mock?    → run_mock_stream
      └ CLI      → resolve_adapter → resolve_executable
                 → run_streaming(build_args) → spawn → on_delta → 气泡
```

插件只替换 `resolve` / `build_args` / 流方言；**不**新开 scheduler。

## 7. 异步切片（改到位的路径）

| 切片 | 做什么 | 过线 |
|---|---|---|
| **P0** | 查表 + 模板 spawn + 目录 API + 单测（未知 id、拒绝 shell） | 现网 `@Cursor`/`@OpenCode` 零回归；新 CLI 可不改 Rust |
| **P0.1** | 随仓 `opencode.adapter.json`，枚举变 fallback | 删文件仍能跑 |
| **P1** | `session` / `envFrom` / 扩 `stream` | Cursor、Codex、Claude 各一份 json |
| **P1.1** | OpenClaw 方言；dsh **只声明 argv**（仍外挂二进制） | `@` 走同一 resolve |
| **P2** | 删除 `AdapterKind` CLI 分发 | 门禁绿；mock/chatbot 入口仍在 |
| **P3** | ACP（仅会说 ACP 的）；配置包 `files`/install | 不阻塞 P2 |

禁止：一个 PR 删光枚举又上全部 json。

**为何 P0 仍写死 Cursor/OpenCode**：怕扫文件/模板失败打挂现网。OpenCode 最先迁（模板已够）；Cursor 等 P1 `session: persist`。不是「永远不能插件化」。

## 8. 测试（P0 起必须有）

- 未知 `id` → 错误字符串含「不支持」
- `args` 含空格拼 shell / 含 `sh` `-c` → 加载失败
- 模板 `{prompt}` 出现在独立 argv 元素
- OpenCode 内置 fallback 与 json 覆盖同名时以文件为准（P0.1）
- **不**把门禁绑到真 CLI 二进制（沿用 `docs/testing-strategy.md`）

## 9. 非目标

Docker 化整机 Agent；Git 里塞 cursor-agent 二进制；配置包带 authId/邮箱；IM connector 当 CLI 插件；chatbot/mock 塞进 `*.adapter.json`；agent 无批准 promote。

## 10. 风险

- 非 jsonl 的内部 CLI，P0 只能 `plain`，思考/工具行会糊成一篇。
- 业务方要 `command: "acme && foo"` → **拒绝**。
- Manifest 不替代「PATH 上有 CLI 且已登录」。
- 文档已锁定、**代码未开工**；落地须门禁 → 灰度 `:8081` → 人批准才能生产。
