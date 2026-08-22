---
date: 2026-08-05
topic: link-existing-user-member
branch: master
status: active
---

# Epitaph: 添加用户支持「加入已有账号」

## Built

- UI：添加成员 → 用户 →「创建新账号」｜「加入已有账号」下拉
- API：`GET /api/users/joinable?groupId=`；`POST .../members` 支持 `existingAuthUserId`
- 共享逻辑：`db::list_joinable_users` / `resolve_user_member_auth_id`

## Do not regress

- 非管理员不可列 joinable / 加成员
- 链接已在本群的用户须拒绝
- 创建路径仍校验用户名占用与保留名 `root`

## Verify

```bash
pnpm run test:gate
# 灰度：创建用户 A → 另一群「加入已有账号」选 A
```
