# OpenClaw 回显修复 + Agent 响应契约测

**日期**: 2026-08-10  
**状态**: canary 待验  

## 问题

1. OpenClaw CLI **2026.3.x** 在 gateway→embedded 回退时，把 JSON 结果写到 **stderr**，stdout 为空。平台只解析 stdout → 聊天气泡变成空/「已完成。」。
2. 偶发把整个 `{ runId, result: { payloads } }` 信封写进 final。
3. 自动化测试只覆盖 CLI 参数/`parse` 形状，**未验收各适配器最终可见文本是否等于预期 token**。

## 修复

| 层 | 行为 |
|---|---|
| `parse_openclaw_envelope` | 支持 bare `{ payloads, meta }`（无 `runId`） |
| `parse_openclaw_mixed_output` | 从 stderr 混杂日志中抽出 JSON，取 `payloads[].text` |
| `run_streaming` | OpenClaw stdout 无 final 时回读 stderr；有 payload 时即使 exit≠0 也成功返回 |
| `resolve_adapter_final_text` | 纯函数折叠 stdout/stderr → final，供契约测 |

## 验收（门禁）

- `adapters::tests::adapter_final_response_contracts`：OpenClaw / Cursor / Codex / Claude / OpenCode fixture → 精确等于 `PONG_*`，且不泄漏 envelope / Gateway 噪音。
- 真 CLI 仍走尽力 smoke，不进门禁。

## 灰度手工

在 canary `:8081` @OpenClaw 发短消息（如 `只回复：PONG_OPENCLAW`），气泡应出现可读文本而非空/原始 JSON。

## 风险

- Gateway 仍可能 1006 / session 锁；本修只保证 **有 payload 时能回显**。
- 非 0 退出但已解析 payload 时视为成功——若 CLI 半截输出可能掩盖真失败（可接受，优于空回显）。
