# Epitaph: PanelLive 短回复注入（§6）

**Date**: 2026-08-05  
**Status**: active（灰度）

## What

Live（`group_extensions.panellive=1`）时，平台向 ChatBot / 管理员 Agent 注入 <50 汉字约束：

- 拉取 `GET http://127.0.0.1:8790/v1/llm-prompt`
- 失败用 `PANELLIVE_LLM_PROMPT_FALLBACK`
- ChatBot system + Agent 全量/续接 prompt

## Files

- `src-tauri/src/extensions.rs` — fetch/parse/inject helpers + tests
- `src-tauri/src/scheduler.rs` — wire-up
- `docs/panellive-platform-requirements.md` §6

## Risks

- 非管理员 CLI Agent 不注入（按契约）
- TTS 侧截断在 PanelLive；平台只负责提示词
- `COSYVOICE_VOICE` / Key 仍由 PanelLive 环境保管，勿入库
