# 设计与决策

## 1. 事实
- Leader = 群管理员 Agent（`groups.admin_member_id`）。
- Leader 运行结束时会从输出内容解析 `@目标`，为每个目标创建 depth+1 的子 run（`delegate_from_admin`）。
- 子 run 失败走 `scheduler::finish_failed`：标记 failed、写 run_event、广播 `run_status`、通知 orchestrator/marketing；**没有**路由动作。
- `task_runs` 已有 `parent_run_id`、`depth`、`status`、`agent_member_id`，足以推导重试次数，无需改 schema。

## 2. 分类（责任：`routing::classify`）
| 分类 | 触发（错误文本小写匹配） | 默认动作 |
|---|---|---|
| Retryable | timeout / timed out / connection / reset / broken pipe / eof / rate limit / 429 / 500/502/503/504 / overloaded / temporarily / spawn | 重试 |
| Permanent | 401/403 / unauthorized / forbidden / api key / credential / login / subscription / quota / unsupported / not installed / not found / no such file / invalid config | 升级 |
| Cancelled | cancelled / canceled / aborted by user | 停止 |
| Unknown | 其余 | 重试（受次数限制） |

分类是启发式：宁可重试一次，也不把可恢复错误直接升级；永久错误不浪费重试。

## 3. 决策（责任：`routing::decide`）
输入：`has_parent`、`is_reviewer`、`is_leader_run`、`depth`、`max_depth`、`attempts`（含本次失败的失败次数）、`max_retries`、`FailureClass`。

```
if !has_parent || is_reviewer || is_leader_run        -> Stop
if depth >= max_depth                                  -> Stop
if class == Cancelled                                  -> Stop
if class == Permanent                                  -> Escalate
if attempts <= max_retries                             -> Retry
else                                                   -> Escalate
```

默认 `max_retries = 1`（即最多两次尝试），常量放在 `routing`，不新增设置项。

## 4. 落地（责任：`routing::route_failed_run`）
1. 读取失败 run 与群；执行上述守卫。
2. **Retry**：用同一 `root_message_id`/目标/父 run/深度创建新 run；`run_events(route)` 记录 `{action:"retry", class, attempts}`；返回新 run id，由调用方 `schedule_group`。
3. **Escalate**：
   - 以 Leader 为 sender 插入消息：`[路由升级] 目标 <name> 调用失败（<class>）：<error≤200字>。请重新指派或直接接手。`；
   - 以该消息为 root、Leader 为 agent、原父 run 为 parent、原深度创建 run；
   - `run_events(route)` 记录 `{action:"escalate", class, escalationRunId}`；
   - 返回新 run id。
4. **Stop**：不创建任何 run；可选写 `run_events(route)` 便于诊断（当前版本不写，避免噪音）。

### 为什么升级是「新消息 + Leader 新 run」
- Leader 的 prompt 由 root message 构建；把失败上下文写成显式消息，Leader 才能在不读隐藏状态的前提下重新决策。
- 升级 run 与失败目标同深度、以原 Leader run 为父，属于同一轮委派的兄弟，不会突破深度上限。

### 防循环
- Leader 自身失败（`agent == admin_member_id`）直接 Stop，避免「升级 → Leader 失败 → 再升级」。
- 升级 run 失败时同理 Stop。
- 重试次数按 `(parent_run_id, agent_member_id, status='failed')` 计数：第 1 次失败触发重试，第 2 次失败触发升级，之后不再升级（升级 run 属于 Leader，受保护）。
- 审阅运行（`reviewer_member_id` 非空或 `review_status='pending'`）不参与路由，避免评审请求被自动重派。

## 5. 可观测性
- `run_events`：`route` 事件，payload 含 action/class/attempts/escalationRunId。
- 升级消息在群聊可见，用户可看到「谁失败了、为什么、接下来由 Leader 处理」。
- 既有 `run_status(failed)` 事件顺序不变，路由动作发生在其后。

## 6. 测试策略
- 纯逻辑：`classify` 各类样本；`decide` 全分支（无父/审阅/Leader/超深/取消/永久/重试/耗尽）。
- DB 集成（tempfile）：委派子 run 失败 → 生成同目标重试 run；重试再失败 → 生成升级消息 + Leader run；Leader run 失败 → 不升级；用户根 run 失败 → 不动。
- 门禁：`cargo test --no-default-features --lib`、clippy、fmt；不新增前端改动。
