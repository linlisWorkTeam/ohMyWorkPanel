---
date: 2026-08-01
topic: agent-streaming-parts-queue-session
status: approved
---

# Design: Agent 流式分区 + 同 Agent 串行排队 + Cursor Session 复用

## Problem

1. Agent 虽有 `message_delta`，UI 在空内容时长期显示「排队等待中 / 正在回应」。
2. 思考 / 中间产物 / 最终回复混在同一 `content` 字符串，无法分区展示。
3. 调度是群级并发，不是「同一 Agent 串行」；连续 `@` 同一成员会并行抢跑。
4. **Cursor CLI 每次 `@` 都新建 session**（`build_args` 仅 `-p`，无 `--resume`），群历史被整段塞进 prompt，上下文断裂且浪费。
5. 在 Cursor CLI 后台页面对话时的回复不会自动回到 WorkPanel（渠道不同）；群内 `@` 路径的回复应写回消息气泡。本期修的是群内调用链，不是把本 CLI 聊天镜像到群。

## Goals

| # | Goal |
|---|---|
| G1 | 有首包输出即渲染；去掉空气泡长期占位 |
| G2 | 同气泡内分区：thinking（可折叠）/ artifact / final（方案 A） |
| G3 | 同 `agent_member_id` 串行排队；不同 Agent 仍可并行（受 `max_concurrent_runs`） |
| G4 | Cursor（及后续可扩展的适配器）在**同一群组同一成员**上复用 CLI session |
| G5 | 完成后只部署灰度 `:8081`，不 promote 生产 |

## Non-goals

- 不自动 `promote-canary`
- 不新建 `message_parts` 表
- 不强求所有 CLI 都能完美识别 thinking（无信号 → 整段落 `final`）
- 不把「当前这条 Cursor IDE/CLI 人工对话」同步进 WorkPanel 群聊

## Architecture

```
@mention → task_runs(queued)
        → schedule_group（群并发 + 同 agent 串行）
        → execute_run
            → 解析 adapter；Cursor 则加载 cli_session_id
            → run_streaming（可选 --resume；解析 channel + session）
            → append_delta(channel) → messages.content JSON parts
            → emit message_delta{channel,delta}
            → 持久化新的 cli_session_id（若有）
        → finish → schedule_group（拉起同 agent 下一队列）
```

## Data model

### Message content (no new table)

`messages.content` 两种形态：

- **Legacy**：纯文本 → 渲染为单一 `final`
- **Structured JSON**：

```json
{
  "v": 1,
  "parts": [
    { "channel": "thinking", "text": "..." },
    { "channel": "artifact", "text": "..." },
    { "channel": "final", "text": "..." }
  ]
}
```

规则：

- `channel` ∈ `thinking` | `artifact` | `final`
- 同 channel 的多次 delta **追加到已有 part.text**；parts 顺序按首次出现
- 读取：`content` 以 `{` 开头且 parse 成功且含 `parts` → 结构化；否则整段当 final
- Helper：`parts_to_plain_text` 供 prompt/审查拼接（final 优先，缺则拼接全部）

### Session persistence (additive column)

`agent_profiles` 增加可空列（与现有 `ALTER TABLE … ADD COLUMN` 风格一致）：

| Column | Type | Meaning |
|---|---|---|
| `cli_session_id` | TEXT NULL | Cursor `--resume` 的 chatId；按 member（已属 group）唯一 |

- 仅 Cursor 适配器读写本期；其他 adapter 忽略
- member 删除时随 `ON DELETE CASCADE` 清理
- 无破坏性 API：IPC command 签名不变；Member 序列化可多一个可选字段

## Event protocol

`ChatEvent` 增加可选字段：

| Field | Type | Default |
|---|---|---|
| `channel` | string? | 缺省 = `final` |

`message_delta` 携带 `messageId` + `channel` + `delta`。旧前端忽略 `channel` 时仍可把 delta 拼进 content（后端已写入正确 part；全量 refresh 后 UI 正确）。

## Adapter parsing

`parse_agent_event(line) -> ParsedEvent { channel, text, session_id? }`：

| Heuristic | channel |
|---|---|
| type/role 含 thinking / reasoning / thought | thinking |
| tool/call/result、command 输出类 | artifact |
| assistant / message / text / content / delta 正文 | final |
| 无法识别的纯文本 | final |

Session 提取（Cursor `stream-json`）：

- 从行内常见字段抓 chat/session id（如 `session_id` / `chatId` / `id` 在 session 类事件上）
- 跑完后若得到新 id → `UPDATE agent_profiles SET cli_session_id=?`

Cursor `build_args` 演进：

```
# 无 session
agent --trust -p <prompt> --output-format stream-json --stream-partial-output

# 有 session
agent --trust --resume <cli_session_id> -p <prompt> --output-format stream-json --stream-partial-output
```

Resume 失败（进程非 0 且 stderr/典型错误表明 session 无效）：清空 `cli_session_id`，**同一次任务**用无 resume 重试一次；仍失败则 `finish_failed`。

### Prompt shape when resuming

- **无 session**：保持现有完整 system + 最近群聊上下文（兼容其他 adapter / 首次 Cursor）
- **有 session**：短 prompt = 身份一行 + **本条任务根消息** + 必要指令（结果/风险/@协作）；**不再倾倒整段历史**（历史已在 CLI session 内）

## Scheduler: per-agent serial

在 `schedule_group` 选 queued runs 时：

1. 仍受 `max_concurrent_runs - running` 限制
2. 候选按 `created_at` 排序
3. 若该 `agent_member_id` 在同群已有 `status='running'` → **跳过**（留给下次 schedule）
4. 不同 agent 可同时 running（在并发上限内）
5. 任一 run 结束仍调用 `schedule_group`，从而拉起同 agent 下一队列

## Frontend UX

同一气泡自上而下：

1. **思考**：`<details>`；默认折叠；**流式中且该区有内容时暂展开**，结束后折叠
2. **中间产物**：有内容才显示；次要/等宽样式
3. **最终回复**：主正文

状态：

- `queued` 且尚无输出 → 可显示「排队中」（同 agent 串行时合理）
- 任意 channel 有内容 → 立即渲染分区，去掉「正在回应」空气泡
- `running` 且尚无 delta → 极短「…」或「正在思考」，有输出后立刻替换

旧纯文本：整段 final。

前端 `message_delta`：按 `messageId` + `channel` 追加到本地 parts（若本地仍是字符串则先升级为结构化）。

## Release

```
实现 → 测试 → deploy-canary.sh → 在 :8081 验证
```

- 不自动 promote
- 生产 `:8080` / `/AI/LinlisWorkPanel/data` 不动

## Testing

| Area | Cases |
|---|---|
| parse | thinking/artifact/final 启发式；纯文本 → final；抽出 session id |
| content | 旧文本兼容；多 channel 追加；`parts_to_plain_text` |
| schedule | 同 agent 第二条保持 queued；不同 agent 可并行；结束后自动启动 |
| cursor args | 无 session / 有 session 的 argv；resume 失败清空并重试（单测 mock） |
| frontend | 有 delta 不显示空气泡；折叠行为（组件级或轻量测） |

## Risks

| Risk | Mitigation |
|---|---|
| CLI JSON 形态漂移，分区不准 | 默认定 final；按适配器后续增强 |
| resume id 字段名不准 | 多 key 启发式 + 失败重试无 session |
| content JSON 被外部当纯文本 | 提供 plain helper；审查/经验路径改用 helper |
| 同 agent 串行降低吞吐 | 产品要求；跨 agent 仍并行 |
| `--stream-partial-output` 行为差异 | 灰度验证；异常则回退仅 stream-json |

## Locked decisions

| Item | Choice |
|---|---|
| UI 分区 | A：同气泡内 thinking / artifact / final |
| 存储 | content JSON parts；不加 parts 表 |
| 方案 | 结构化 parts + channel 事件（原推荐方案 1） |
| Session | 同群同 member 复用 Cursor `cli_session_id` + `--resume` |
| 发布 | 仅 canary `:8081` |

## Out of scope follow-ups

- Codex / Claude 等各自的 session/resume 协议
- Promote 生产
- 把 IDE 侧人工 CLI 对话双向同步到群
