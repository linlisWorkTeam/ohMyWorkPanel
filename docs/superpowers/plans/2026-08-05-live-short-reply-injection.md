# PanelLive 短回复提示词注入（平台侧）

- 日期：2026-08-05
- 状态：已实现（Cursor Agent；灰度）
- 轨道：C PanelLive（version-pipeline.md）
- 来源：root 指令（STT fun-asr-flash / TTS cosyvoice-v3.5-flash 已选，TTS 务必省用）

## 需求（root 强制规则）

1. Live 模式下，给 ChatBot/管理员 Agent 注入短回复提示：**每次最终输出 < 50 个汉字**
2. PanelLive 送 TTS 前硬截断 50 字（双保险）——**PanelLive 侧**（/AI/WorkPanelLive 已就绪，见其 docs/llm-prompt-panellive.md）
3. 提示词拉取：`GET http://127.0.0.1:8790/v1/llm-prompt`（**已实测在线**，返回 `{mode, prompt, ttsMaxChars:50}`）
4. 文档：`/AI/WorkPanelLive/docs/llm-prompt-panellive.md`（已存在）+ `docs/panellive-platform-requirements.md §6`（本次补）

## 勘察结论（WorkPanel 侧缺口）

| 项 | 现状 | 缺口 |
|---|---|---|
| Live 会话状态 | `live.session.start/stop/cancel` 仅代理转发 PanelLive（a2a.rs:122-160），**WorkPanel 无任何 live 激活记录** | 需要"该群 Live 是否激活"标记 |
| 注入点（chatbot） | scheduler.rs `run_agent` chatbot 快路径：`system`=人设+公告（L459-466） | system 无短回复约束 |
| 注入点（CLI Agent） | `get_execution_context` 末尾拼 `prompt`（L382-396） | prompt 无短回复约束 |
| 提示词来源 | 无 | 需拉 8790 + fallback |

## 方案

### T1 Live 会话状态（内存级）
- `SchedulerState` 加 `live_sessions: Mutex<HashMap<String /*group_id*/, i64 /*started_at*/>>`
- `live.session.start` → 置位；`live.session.stop/cancel` → 清除（a2a.rs dispatch_live_skill 内 hook）
- 服务重启即清（可接受：PanelLive 会话本就随进程死）

### T2 提示词拉取模块（新 `live_prompt.rs`）
- `fetch_live_prompt(port) -> String`：`GET /v1/llm-prompt`，timeout 1.5s，解析 `prompt` 字段
- **60s 缓存**（避免每 run 打一次）；失败/非 200 → 内置默认文案（与端点同文，硬编码常量）
- 端口复用 `extensions::panellive_upstream_port(manifest)`，不硬编码 8790
- 单测：解析成功 / 超时 fallback / 缓存命中

### T3 注入（仅 Live 激活群 + 仅 chatbot / 管理员 Agent）
- `run_agent` chatbot 快路径：live 激活 && (kind=chatbot || agent==group.admin) → `system` 末尾追加 `\n\n{live_prompt}`
- CLI Agent：同上条件 → `prompt` 末尾追加 `\n\n{live_prompt}`
- 其他 Agent / 非激活群：不注入（零影响）
- 纯函数 `should_inject_live(live_active, kind, agent_id, admin_id)` + 单测

### T4 文档 + 发版
- `docs/panellive-platform-requirements.md §6`（注入规则、端点、fallback）
- epitaph 一篇；门禁绿；灰度 :8081 → A2A 公告 → commit；生产另批

## 验收标准

1. 灰度聊天群开 Live 会话 → 让 chatbot 回长文 → 输出 < 50 字且语气简洁
2. 停 Live 会话 → 同 chatbot 恢复正常长回复
3. 杀掉 8790 → 聊天不报错，仍注入内置默认提示词
4. 非 Live 群 / 非 chatbot 非管理员 Agent：prompt 与之前完全一致（回归）
5. `test:gate` 绿

## 风险

- 模型可能不完全遵守 50 字 → 双保险靠 PanelLive TTS 前硬截断兜底（已确认其侧实现）
- 内存态 live 标记在重启后丢失 → 需要 PanelLive 重新 session.start（其会话同样丢失，行为一致）
- 注入只约束"最终输出"；chatbot 中间过程不受影响
