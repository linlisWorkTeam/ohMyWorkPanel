---
date: 2026-08-15
topic: context-seams-handoff
status: implementing
---

# Context Seams：交接运行时注入（TOP1 切片）

## 问题

群公告与 Wiki 已进调度 prompt；`docs/epitaph/` 与 `version-pipeline.md` 仍是人读文件。Agent 不保证读到，Experience/Logs 也无法对照「模型看见了什么」。

## 本切片做

1. **有序 seam**：把已有注入收成带 `name/source/chars` 的 section 列表（announcement → epitaph → live → memory → wiki → experience）。
2. **epitaph 摘要**：读工作区 `docs/epitaph/README.md` 的 **Active** 表前 5 行；再读最新一篇的标题 + `## Do not regress`（无则取文首若干行）。整块上限 1200 字。只允许 `docs/epitaph/*.md`，fail-open。
3. **记账**：`logs` 表 `source=context_seams`，`details` 为 ledger JSON（不含正文）；prompt 末行 `【已注入上下文】name:chars · …`；WS 事件 `context_injected`（`delta`=同一行）。
4. **resume**：短续跑 prompt 同样注入 announcement + epitaph 摘要。

## 不做

- 不注入 epitaph / version-pipeline **全文**
- 不新建 `run_events` 表、不改 `task_runs` 对外 SELECT 形状
- 不搬 dsh/Cordis；不做 Wave 闭环（TOP2）
- 不改 Experience UI（Logs 面板已能按 source 过滤）

## 验收

- Rust 单测：空目录 fail-open；Active 解析；路径不逃逸；空 section 不进 ledger；ledger JSON 无 body。
- 灰度：有 `docs/epitaph/` 的工作群跑一次 @，prompt 含 `【交接 / 墓志铭` 与 `【已注入上下文】`；Logs 能搜到 `context_seams`。
