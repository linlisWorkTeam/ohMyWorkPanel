---
date: 2026-08-05
topic: presence-unread
status: approved
---

# Design: 用户在线状态 + 群未读标识

## Goal

- 左侧群列表显示未读数；有未读的群排前面；进入群后清零。
- 成员栏对 `kind=user` 显示在线绿点（WS 在线）。

## Approach

SQLite `group_read_cursors` + WS presence registry（方案 1，已批准）。

## Data

```sql
CREATE TABLE group_read_cursors (
  user_id TEXT NOT NULL,
  group_id TEXT NOT NULL,
  last_read_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, group_id)
);
```

Unread = count of messages in group where `created_at > last_read_at` and `sender` is not the viewer's member in that group (approx: all messages after cursor; optionally exclude own — prefer exclude own via join members.auth_user_id).

## API

- `GET /api/groups` → each group includes `unreadCount: number`
- `PUT /api/groups/{id}/read` → set `last_read_at = now()` for current user
- Presence: WS upgrade associates JWT user; on connect/disconnect broadcast `{ kind: "presence", userId, online }`
- Optional `GET /api/presence` → online user ids (for initial paint)

## Frontend

- Sort groups: unread first, then existing order
- Badge on group row; clear on select + PUT read
- WS message for other groups → increment local unread + resort
- MemberRow: green dot if user online

## Out of scope

Per-message receipts, push notifications, agent online.
