---
date: 2026-08-06
topic: codex-proxy-tools
branch: master
status: active
---

# Epitaph: Codex 代理 tools / tool_calls 修复

## Built this session
- `scripts/codex-deepseek-proxy.cjs`：转发 `tools`/`tool_choice`；Chat `tool_calls` ↔ Responses `function_call`（流式+非流式）；`function_call_output` 回灌；`tool_choice=required` 遇上游拒绝回退 `auto`；assistant 工具轮带空 `reasoning_content`（DeepSeek thinking）
- 备份：`scripts/codex-deepseek-proxy.cjs.bak.*`
- 验证：`:18888` 非流式/流式 `get_weather` → `function_call`；output 回灌后文本回复
- Rust `codex_proxy.rs` 同步映射单测

## Do not regress
- 勿再只转发 `delta.content` 而丢弃 `tool_calls`
- 双槽位代理端口：prod `:18888` / canary `:18889`，改脚本后需重启两边 node
