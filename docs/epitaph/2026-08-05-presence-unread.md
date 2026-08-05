---
date: 2026-08-05
topic: presence-unread
branch: master
status: active
---

# Epitaph: 群未读角标 + 用户在线状态

## Built

- `group_read_cursors` + `unreadCount` on `GET /api/groups`；未读优先排序
- `PUT /api/groups/{id}/read`；`GET /api/group` 亦清未读
- WS presence registry；`presence` / `presence_snapshot`；成员栏用户绿点
- 前端：角标、进群清零、他群 `message_created` / `run_status=completed` 本地 +1

## Do not regress

- 不把 Agent/chatbot 标在线
- promote 不碰 prod DB（游标在各槽位库各自累计）
