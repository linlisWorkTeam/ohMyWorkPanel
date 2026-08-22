---
date: 2026-08-05
topic: doubao-voice-ux-host
status: active
source: /AI/WorkPanelLive/docs/superpowers/specs/2026-08-05-doubao-voice-ux-contract.md
---

# Host：豆包式聊天语音 UX

## 承接

ohMyWorkPanel（Host）实现主聊天「按住说话」与气泡「播放」；媒体契约见 WorkPanelLive 契约文。

## 行为

| 项 | 实现 |
|---|---|
| 显隐 | `panellive` enabled + healthy（`liveTabEnabled`） |
| 按住说话 | Host WAV → `POST /api/extensions/panellive/v1/stt` → 文本 |
| 松手 | **B 松手即发**；草稿与转写拼接；有默认响应者时 `@` 后发送 |
| 气泡播放 | `POST …/v1/tts?format=json` + `purpose=playback`（300 字顶） |
| 禁止 | 直连 `:8790`；PCM 进 A2A/群消息 |

## 关键代码

- `src/liveVoice.ts` / `src/liveVoice.test.ts`
- `src/App.tsx`（composer 按住钮、气泡 ▶）
- `src/styles.css`
- `src-tauri/src/web.rs` / `extensions.rs`：同源代理**转发 query**（`?format=json`）

## 灰度验收

1. 开 Live → 发送键左侧出现按住说话；关则消失  
2. 按住说话 → 转写发出  
3. 气泡 ▶ → 能听；长文截断可感知  
4. Live 页签短回仍 ≤50（Extend 侧 `purpose=live`）
