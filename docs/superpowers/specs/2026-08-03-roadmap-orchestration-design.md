---
date: 2026-08-03
topic: roadmap-orchestration
status: approved-for-impl
---

# Design: Roadmap item orchestration (checklist-driven)

## Locked decisions

| Item | Choice |
|------|--------|
| Driver | System orchestrator |
| Assignee | Feature.assignee → group admin → fail |
| Cadence | One roadmap item; Features by sortOrder; FeatureTasks serial |
| Completion | `task_runs.status == completed` → mark FeatureTask done |
| Failure | Pause orchestration; human resume/cancel |
| Dispatch | Owner sends `@Agent` chat message → existing scheduler |

## Data

`roadmap_orchestrations`: id, group_id, roadmap_item_id, status (`running|paused|completed|failed|cancelled`), cursor_feature_id, cursor_task_id, current_run_id, error_message, created_at, updated_at.

## API

- `POST /api/roadmap-items/{id}/start`
- `POST /api/roadmap-orchestrations/{id}/pause|resume|cancel`
- `GET /api/groups/{groupId}/roadmap-orchestrations`
- WS `orchestration_status`

## UI

Roadmap strip: Start / Pause / Resume / Cancel + status hint.
