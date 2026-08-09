---
date: 2026-08-08
topic: wiki-retrieve-inject
status: active
---

# Epitaph：Wiki retrieve + 调度注入（跨 Agent 遵从）

## 做了什么

**WorkPanelWiki（W0）**

- `cli.py retrieve`：稳定 JSON；`--tags` / `--must-read` / `--excerpt-chars`
- 知识：`must_read` 运作规则 + `跨Agent协作契约模板.md`
- 设计：`docs/superpowers/specs/2026-08-08-cross-agent-memory-compliance-design.md`

**LinlisWorkPanel（W1）**

- `src-tauri/src/wiki_context.rs`：run 前调 Wiki retrieve，拼【全局知识·Wiki】进所有 CLI Agent prompt
- 环境：`LINLIS_WIKI_ROOT` / `LINLIS_WIKI_RETRIEVE` / `LINLIS_WIKI_RETRIEVE_TIMEOUT_MS`
- canary unit 已加 Wiki 环境变量

## 未做（W2+）

- WikiAgent 隐藏成员 / `wiki` adapter
- 全局 experience 写回 Wiki
- 结构化 collab_contract 消息类型

## 风险

- 首次 jieba 冷启动可能接近超时；失败则跳过注入（fail-open）
- 生产 unit 尚未加 `LINLIS_WIKI_*`（需 promote / 单独改 unit）
