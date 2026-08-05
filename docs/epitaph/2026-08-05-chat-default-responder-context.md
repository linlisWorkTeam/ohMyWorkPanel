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
- **Chatbot 上下文**：`build_chatbot_user_message` 把最近群聊（`context_message_limit` 默认 40）整段塞进 user（豆包式原生窗口，非向量记忆）
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
