# Plan: Roadmap orchestration (implemented)

## Files

- `src-tauri/src/orchestrator.rs` — assignee/checklist logic + start/pause/resume/cancel + run terminal hook
- `src-tauri/src/web.rs` — REST routes + schedule after send_message
- `src-tauri/src/scheduler.rs` — call orchestrator on run completed/failed
- `src/ProjectWorkflowView.tsx` — Start/Pause/Resume/Cancel
- `src/api-web.ts` / types — client API

## Verify

```bash
pnpm run test:gate
./scripts/deploy-canary.sh
# :8081 项目视图 → 路线图项关联 Feature+checklist+assignee → 启动
```
