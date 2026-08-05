# WorkPanel 平台要求（来自 PanelLive）

> PanelLive 是 WorkPanel 上的 **Extend** 服务。本文件列出平台侧需提供的能力；实现代码在 `/AI/WorkPanelLive`，不在平台仓内完成 PanelLive 业务。

## 1. Extension Host（加卸载）

| 能力 | 说明 |
|---|---|
| 运行设置 · Live 开关 | 开启 load PanelLive；关闭 unload，Live 页签置灰 |
| 后端生命周期 | `load` / `unload` / `health` |
| 前端贡献点 | 注册 `tab://live`（与「聊天」「项目」平级）；页签 UI 由 PanelLive 前端提供，平台只做路由索引 |
| 清单格式 | 读取 Extend 包内 `extension.manifest.json` |

## 2. 服务分层

| 类型 | 示例 |
|---|---|
| Base | 群聊、WorkPanelConnecter（监控/协调，**不转音频**） |
| Extend | PanelLive（可装卸） |

## 3. A2A / 泛 Agent skills（控制面）

PanelLive 以外部 Agent 形态参与交互。建议 skills：

| Skill | 方向 | 载荷 |
|---|---|---|
| `live.transcribe.result` | PanelLive → 平台/ChatBot | 增量/最终文本（**无 PCM**） |
| `live.synthesize.request` | 平台/ChatBot → PanelLive | 待播文本片段 |
| `live.session.cancel` | 双向 | 打断：停 LLM + 停 TTS + 清空播放 |
| `live.session.start` / `stop` | 双向 | Live 会话生命周期 |

**禁止**将原始音频放进 A2A/群消息体。媒体面由 PanelLive 本地（或直连云）处理。

## 4. 目标交互（真云阶段）

```text
用户语音 → PanelLive 直连云 STT → 文字流
  → A2A/平台 → 指定管理员 Agent 调度
  → ChatBot 总结
  → PanelLive 直连云 TTS → 语音回传用户
```

MVP（方案 A）用 Mock STT/TTS 验证同一控制流。

## 5. 平台 API 缺口清单（需 WorkPanel 实现）

- [x] `PUT /api/groups/{id}/extensions/panellive` — enable/disable（load 前探活 PanelLive `:8790`）  
- [x] `GET /api/groups/{id}/extensions` — 列表与 health  
- [x] 前端 Extension 路由表 / 页签注册（`tab://live` → iframe PanelLive entry）  
- [x] A2A 总线：`POST /api/a2a/dispatch`（`live.*` skills；禁 PCM 字段）  
- [ ] （可选）短时云凭证下发：若 Key 由平台保管；**MVP 不需要**  

## 6. PanelLive 模式 LLM 输出上限（强制）

模型：`fun-asr-flash-2026-0615` + `cosyvoice-v3.5-flash`（省着用，尤其 TTS）。

Live 开启时，平台必须给 ChatBot / 管理员 Agent 注入短回复约束（也可 `GET PanelLive/v1/llm-prompt`）：

- **每次最终输出 < 50 个汉字**
- PanelLive 在送 TTS 前会再硬截断 50 字（双保险）

详见 `/AI/WorkPanelLive/docs/llm-prompt-panellive.md`。

**平台实现（已落地，对齐方案 T1–T3）**：

- **Live 会话态**：`SchedulerState.live_sessions`
  - A2A `live.session.start|stop|cancel` 置位/清除
  - **同源代理** `POST …/v1/session/start|cancel` 若带 `X-Linlis-Group-Id` 同样置位/清除（iframe Live UI 路径）
- **提示词**：`live_prompt.rs` — `GET /v1/llm-prompt`（1.5s 超时、60s 缓存、内置 fallback；端口来自 manifest）
- **注入条件**：仅 **会话激活** 的群 + chatbot / 群管理员 Agent；ChatBot Live 时 `max_tokens=128`
- **聊天一致**：`LivePanel` 向 iframe 同步群聊 `messages`；STT 文本经 `postMessage` → `sendMessage` 写入同一群（@默认响应者）
- **麦克风**：需安全上下文（HTTPS 或 localhost）；`http://公网IP` 下 `mediaDevices` 为 undefined
- 方案：`docs/superpowers/plans/2026-08-05-live-short-reply-injection.md`

## 7. 非目标

- Connecter 调用云 STT/TTS  
- 在 WorkPanel 核心进程内嵌重采样/转码  
 
