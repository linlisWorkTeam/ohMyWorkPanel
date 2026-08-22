---
name: prod-canary-release
description: >-
  ohMyWorkPanel production/canary dual-slot release: freeze, deploy canary, promote.
  Use when deploying, promoting canary to prod, freezing production, or when the user
  mentions 生产/灰度/canary/promote/发布.
---

# ohMyWorkPanel — 生产 / 灰度发布

项目内副本；个人全局 skill 名为 `linlis-prod-canary`（`~/.cursor/skills/linlis-prod-canary/`）。内容以仓库脚本为准。

## 槽位

| | prod | canary |
|---|---|---|
| Port | 8080 | 8081 |
| Artifacts | `/opt/ohmyworkpanel/prod` | `/opt/ohmyworkpanel/canary` |
| Data | `/AI/ohMyWorkPanel/data` | `/AI/ohMyWorkPanel/data-canary` |
| Unit | `ohmyworkpanel.service` | `ohmyworkpanel-canary.service` |

## Workflow

```bash
# 灰度（低内存）— Agent 默认可做的上限
export CARGO_BUILD_JOBS=1 NODE_OPTIONS=--max-old-space-size=1024
./scripts/deploy-canary.sh          # or: skip if already built
# 测通 :8081 后 —— 以下两步必须人类 root 批准，Agent 不得自行执行 promote
./scripts/approve-prod-release.sh "linli: promote after canary OK"
./scripts/promote-canary.sh         # bin+dist only; never touches prod DB
# 紧急冻结（同样需要批准令牌）
./scripts/approve-prod-release.sh "linli: emergency freeze"
./scripts/freeze-prod.sh from-running
# restart prod 也须人类明确授权
```

## Hard rules

- **生产发版门禁**：无 `/opt/ohmyworkpanel/PROD_APPROVE`（由 `approve-prod-release.sh` 写入，15 分钟一次性）则 `promote`/`freeze` 直接拒绝。Agent 不得创建该文件或设 `OHMYWORKPANEL_ALLOW_PROD_WITHOUT_APPROVAL`。
- `deploy-canary` 只同步 canary unit、只重启 canary；禁止改写 prod unit、禁止杀死 `:18888`
- Canary Codex `:18889`；prod Codex `:18888`（勿共用）
- Prod must not serve workspace `dist/` or `target/release`
- Never share data dirs between prod and canary
- Promote never overwrites `/AI/ohMyWorkPanel/data`
- Keep group **ohMyWorkPanel** + `root`/`root` on prod
- Sync canary unit after edits with `/bin/cp -f`；prod unit 变更须单独批准

## Verify

```bash
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/
curl -sS -X POST http://127.0.0.1:8080/api/auth/login \
  -H 'Content-Type: application/json' -d '{"username":"root","password":"root"}'
```

After release milestones, write epitaph (`epitaph` skill).
