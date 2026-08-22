---
date: 2026-08-15
topic: v1.3-context-seams-handoff
branch: master
status: active
---

# Epitaph: 交接运行时注入（Context Seams）

## Built this session
- 调度 prompt 增加 `docs/epitaph` Active 摘要 + Do not regress；空目录 fail-open。
- 注入记账：`logs.source=context_seams`（ledger JSON 无正文）+ prompt/WS `【已注入上下文】`。
- 不注入全文，不改 `task_runs` SELECT，不做 Wave 闭环。

## Key files
| 文件 | 说明 |
|---|---|
| `src-tauri/src/context_seams.rs` | section / ledger / epitaph 解析 |
| `src-tauri/src/scheduler.rs` | 组 prompt + resume + 打日志/事件 |
| `docs/superpowers/specs/2026-08-15-context-seams-handoff-design.md` | 设计 |

## Locked product decisions
| 项 | 选择 |
|---|---|
| 注入深度 | 摘要（Active 5 行 + 最新约束），不是全文 |
| 记账 | 现有 `logs` + WS `context_injected` |
| 缺目录 | fail-open，不挡 run |

## Known pitfalls
- 工作区没有 `docs/epitaph/README.md` 的群不会注入交接（预期）。
- Active 表改格式会导致解析为空（仍 fail-open）。

## How to run / verify
```bash
cd src-tauri && cargo test --no-default-features --lib context_seams
# Logs 面板 source 含 context_seams；prompt 含【交接 / 墓志铭】与【已注入上下文】
```

## Do not regress
- 勿把 epitaph 全文塞进 prompt。
- 勿因缺 epitaph 目录而失败整个 run。
- 勿把 `../` 链接读出 `docs/epitaph/`。

## Open follow-ups
- Wave 闭环（TOP2）；Phase 2 集成测（TOP3）。
- Experience UI 展示 ledger（非本切片）。
