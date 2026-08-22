---
date: 2026-08-15
topic: smooth-release-drain
branch: master
status: active
---

# Epitaph: 平滑发版 Drain + 重启重入队

## Built this session
- Drain：`PUT/GET /api/ops/drain`；开启后拒新 task_run、不启动 queued；running 继续
- 启动恢复：`queued/running` → `queued` + `phase=recovering`（不再永久 interrupted）
- `scripts/lib/drain-wait.sh` 接入 `deploy-canary` / `promote-canary`
- 设计：`docs/superpowers/specs/2026-08-15-smooth-release-drain-design.md`

## Do not regress
- promote 仍须审批；勿覆盖 prod DB
- drain 超时仍可发版，依赖重启重入队

## Verify
```bash
pnpm run test:gate
curl -X PUT :8081/api/ops/drain -d '{"enabled":true}'  # admin
./scripts/deploy-canary.sh
```
