---
date: 2026-08-05
topic: chat-default-responder-context
branch: master
status: active
---

# Epitaph: 聊天群默认响应者 + chatbot 原生窗口上下文

## Built

- **默认响应者**：`admin_member_id` 可设为活跃 **Agent 或 chatbot**；未设置则无 @ 时不自动回复；用户不能当兜底
- **UI**：聊天群按钮「设为默认响应 / 撤销默认响应」；徽章「默认响应」
- **Chatbot 上下文**：`build_chatbot_user_message` 整段塞进 user（豆包式原生窗口，非向量记忆）
- **窗口分流（续）**：`chat_context_message_limit` 默认 **12**（聊天群/chatbot）；工作群 Agent 仍用 `context_message_limit` 默认 40；聊天字符预算 8k vs 24k。仍无摘要/RAG。
- **时间戳**：历史行 `[YYYY-MM-DD HH:MM] 名: 内容`（服务器本地时区），chatbot/Agent 共用。
- **滚动摘要**：chatbot 路径在未摘要消息超过窗口（默认 12）时触发一次折叠写入 `chat_context_summaries`，其后在摘要上累加最近原文；LLM 失败则摘录回退。非向量 RAG。
- **keep-alive**：仅 Agent 管理员开启；chatbot 管理员不保活

## Key files

| 文件 | 说明 |
|---|---|
| `src-tauri/src/db.rs` | `member_is_default_responder_candidate` / resolve 兜底 |
| `src-tauri/src/adapters/chatbot.rs` | `build_chatbot_user_message` |
| `src-tauri/src/scheduler.rs` | ExecutionContext.recent_chat / root_task |
| `src/App.tsx` | 成员栏设默认响应 |

## Do not regress

- 未设 admin → 无默认 run
- @ 仅用户 → 管理员不插嘴
- chatbot 不走 CLI / 不开 keep-alive

## Open

- D1 表情/主题隔离仍待拍板
- 生产 promote 另批
