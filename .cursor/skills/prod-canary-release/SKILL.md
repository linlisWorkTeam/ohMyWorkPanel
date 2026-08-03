---
name: prod-canary-release
description: >-
  LinlisWorkPanel production/canary dual-slot release: freeze, deploy canary, promote.
  Use when deploying, promoting canary to prod, freezing production, or when the user
  mentions 生产/灰度/canary/promote/发布.
---

# LinlisWorkPanel — 生产 / 灰度发布

项目内副本；个人全局 skill 名为 `linlis-prod-canary`（`~/.cursor/skills/linlis-prod-canary/`）。内容以仓库脚本为准。

## 槽位

| | prod | canary |
|---|---|---|
| Port | 8080 | 8081 |
| Artifacts | `/opt/linlis-workpanel/prod` | `/opt/linlis-workpanel/canary` |
| Data | `/AI/LinlisWorkPanel/data` | `/AI/LinlisWorkPanel/data-canary` |
| Unit | `linlis-work-panel.service` | `linlis-work-panel-canary.service` |

## Workflow

```bash
# 灰度（低内存）
export CARGO_BUILD_JOBS=1 NODE_OPTIONS=--max-old-space-size=1024
./scripts/deploy-canary.sh          # or: skip if already built
# 测通 :8081 后
./scripts/promote-canary.sh         # bin+dist only; never touches prod DB
# 紧急冻结
./scripts/freeze-prod.sh from-running && systemctl restart linlis-work-panel
```

## Hard rules

- Prod must not serve workspace `dist/` or `target/release`
- Never share data dirs between prod and canary
- Promote never overwrites `/AI/LinlisWorkPanel/data`
- Keep group **LinlisWorkPanel** + `root`/`root` on prod
- Sync `deploy/systemd/*.service` after unit edits; use `/bin/cp -f`

## Verify

```bash
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/
curl -sS -X POST http://127.0.0.1:8080/api/auth/login \
  -H 'Content-Type: application/json' -d '{"username":"root","password":"root"}'
```

After release milestones, write epitaph (`epitaph` skill).
