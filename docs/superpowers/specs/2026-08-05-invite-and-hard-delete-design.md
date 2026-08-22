---
date: 2026-08-05
topic: invite-and-hard-delete
status: implemented
---

# Design: 邀请入群 + 成员永久删除

## A. Invite

- Add-user mode `invite`: create pending user member (`auth_user_id` NULL) + `group_invites` row (token, expires +24h).
- Link: `/invite/{token}` → login/register → `POST /api/invites/{token}/accept` binds user.
- Member UI: pending shows **链接中**.
- Public preview: `GET /api/invites/{token}`（无 JWT）.

## B. Remove vs Delete

- Remove: soft `is_active=0` (existing).
- Delete (`DELETE .../purge`): 无消息/任务引用时真正 `DELETE` 行；有历史引用时设 `roster_hidden=1`（名单不可见，保留 FK）。群主不可删。

## Out of scope

Email delivery; multi-use invites; deleting global user accounts.
