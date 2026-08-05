---
date: 2026-08-05
topic: doubao-voice-ux-host
branch: master
status: active
---

# Epitaph: Host 豆包式按住说话 / 气泡播放

## Built this session
- **Host UI**: panellive healthy 时 composer「按住说话」（松手即发）+ 气泡 ▶（`purpose=playback`）
- **Proxy**: 转发 query（`?format=json`），否则 TTS 返回裸 mp3 导致前端 JSON 解析失败
- **灰度**: `:8081` 已部署；代理 TTS JSON + UI bundle 含控件字符串；生产未 promote

## Key files
| 文件 | 说明 |
|---|---|
| `src/liveVoice.ts` | WAV/STT/TTS 代理客户端 |
| `src/App.tsx` | 控件接线 |
| `src-tauri/src/web.rs` | `with_proxy_query` |
| `docs/superpowers/specs/2026-08-05-doubao-voice-ux-host.md` | Host 契约摘要 |

## Locked product decisions
| 项 | 选择 |
|---|---|
| 松手行为 | B 松手即发（草稿拼接） |
| 边界 | UI Host / 媒体 Extend |

## Known pitfalls
- 非 HTTPS 无麦克风
- 播放同 messageId 进行中忽略连点
- 读消息 >300 字由 Extend 截断

## Do not regress
- 勿直连 `:8790`；勿把 PCM 塞进 A2A
- Live 短回仍 `purpose=live` / 50 字
