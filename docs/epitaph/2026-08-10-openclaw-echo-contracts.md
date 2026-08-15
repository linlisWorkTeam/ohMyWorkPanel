---
date: 2026-08-10
topic: openclaw-stderr-echo-contracts
branch: master
status: active
---

# Epitaph: OpenClaw stderr 回显 + 适配器响应契约测

## Built this session

- **Root cause**: OpenClaw 2026.3 常把 JSON 结果打到 **stderr**（gateway 1006 → embedded），平台只读 stdout → 空回显 / 「已完成。」；偶发把整段 envelope 当 final。
- **Fix**: `parse` 支持 bare `{payloads,meta}` + stderr 混杂 JSON；`run_streaming` stdout 无 final 时回读 stderr；有 payload 时容忍非 0 exit。
- **Tests**: `resolve_adapter_final_text` + `adapter_final_response_contracts`（OpenClaw/Cursor/Codex/Claude/OpenCode → `PONG_*`）。
- **Docs**: `docs/superpowers/specs/2026-08-10-openclaw-stderr-echo-fix.md`；`docs/testing-strategy.md` R8b。

## Key files

| 文件 | 说明 |
|---|---|
| `src-tauri/src/adapters/parse.rs` | OpenClaw 信封 / mixed stderr |
| `src-tauri/src/adapters/mod.rs` | stderr 回退 + `resolve_adapter_final_text` 契约测 |
| `docs/testing-strategy.md` | R8b |

## Locked product decisions

| 项 | 选择 |
|---|---|
| 真 CLI | 仍不进门禁；契约用 fixture |
| Promote | 须人类批准；Agent 只部署 canary |

## Known pitfalls

- Gateway `:18789` 1006 / session 文件锁仍会导致慢或失败；本修只保证有 payload 时能回显。
- 非 0 + 已解析 payload → 仍算成功。

## How to run / verify

```bash
pnpm run test:gate
# 或单独：
cd src-tauri && cargo test --no-default-features --lib adapters::tests::adapter_final_response_contracts
./scripts/deploy-canary.sh
# canary :8081 @OpenClaw → 气泡应有可读文本
```

## Do not regress

- 不得把 `runId`/`payloads` 原始 JSON 写入聊天气泡
- OpenClaw 必须在 stdout 空时尝试 stderr
- 门禁不得跳过；不得写生产 `data/`

## Open follow-ups

- 真 OpenClaw smoke（网关健康）可选挂脚本，仍不阻塞门禁
- 未要求则不 promote 生产
