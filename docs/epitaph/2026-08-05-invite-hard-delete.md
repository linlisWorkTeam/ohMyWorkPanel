# Epitaph: 邀请入群 + 成员永久删除

**Date**: 2026-08-05  
**Status**: active（灰度；未 promote 生产）

## What shipped

- `group_invites` + `members.roster_hidden`
- 添加用户模式「邀请链接」→ `/invite/{token}` 落地页登录/注册后 `accept`
- 成员面板：待接受显示「链接中」；活跃→「移除」；灰/待邀请→「删除/撤销邀请」→ `purge`
- SPA fallback：`ServeDir.not_found_service(index.html)` 以支持 invite 路径

## Spec

`docs/superpowers/specs/2026-08-05-invite-and-hard-delete-design.md`

## Risks

- 有消息/任务引用时 purge 为隐藏而非物理删行
- 邀请无邮件投递，需管理员手动复制链接
- 生产未自动 promote

## Next

灰度验证邀请全流程后，用户确认再 promote。
