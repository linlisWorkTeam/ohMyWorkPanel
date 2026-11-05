# 验收追踪

| 场景 | 规格 | 证据 |
|---|---|---|
| 首次可重试失败 → 同目标重试 | R1/R2 | `routing::tests::first_failure_creates_retry_run`（新 run：agent=a、parent=r-lead、depth=1、queued；route 事件 1 条） |
| 重试耗尽 → 升级 | R3 | `second_failure_escalates_to_leader`（Leader run、depth=1、`[路由升级]` 消息） |
| 永久失败 → 直接升级 | R1/R3 | `permanent_failure_escalates_immediately`（401 → Leader run） |
| Leader 自身失败不升级 | R4 | `leader_failure_does_not_escalate` |
| 根运行失败不路由 | R4 | `root_failure_is_not_routed` |
| 分类样本 | R1 | `classify_samples`（超时/连接/429/401/未安装/额度/取消/未知） |
| 决策全分支 | R2/R3/R4 | `decide_*` 5 例（重试、耗尽、永久、取消、守卫、无 Leader） |

## 命令
```bash
cd src-tauri && cargo test --no-default-features --lib routing::
cargo clippy --all-targets --all-features -- -D warnings
cd .. && pnpm run test:gate
```

## 未覆盖
- 真实 CLI 失败（需可复现的凭据/网络故障）；
- 多 Leader/多目标并发下的升级竞争（当前按 `(parent, agent)` 失败计数，极端重复委派会提前升级，已在 tasks 1.2 标注待评审）。
