---
name: epitaph
description: >-
  Write and maintain Epitaph (墓志铭) handoff notes under docs/epitaph/ for LinlisWorkPanel.
  Use when the user mentions 墓志铭/epitaph/交接/handoff, or when starting/ending a work session.
---

# 墓志铭 (Epitaph)

项目内副本；与 `~/.cursor/skills/epitaph/SKILL.md` 保持同构。

## 读（接手前）

1. `docs/epitaph/README.md` → 最新 active
2. 遵守 `Do not regress` / `Locked product decisions`
3. 涉及部署 → 同时读 `prod-canary-release`（或全局 `linlis-prod-canary`）

## 写（结束时）

- 路径：`docs/epitaph/YYYY-MM-DD-vX.Y-<topic>.md`
- 归档：`docs/epitaph/archive/`
- 更新：`docs/epitaph/README.md`（Active 新行在前）

```yaml
---
date: YYYY-MM-DD
topic: vX.Y-<短主题>
branch: <branch>
status: active
---
```

章节：`Built this session` → `Key files` → `Locked product decisions` → `Known pitfalls` → `How to run / verify` → `Do not regress` → `Open follow-ups`

原则：写给无会话上下文的下一个 agent；路径/命令/验证结果要具体。

双槽位相关墓志铭必写：prod `:8080` + data 路径、canary `:8081`、promote 不碰 DB、生产群组 LinlisWorkPanel。
