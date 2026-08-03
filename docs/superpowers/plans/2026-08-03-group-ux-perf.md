# Plan: 群 UX + 性能（执行清单）

> Spec: `docs/superpowers/specs/2026-08-03-group-ux-perf-design.md`

## Files
- Backend: `models.rs`, `db.rs`, `commands.rs`, `web.rs`, `adapters/*`, `chatbot.rs`, `scheduler.rs`, `metrics.rs`（新）, `main_server.rs`, `lib.rs`
- Frontend: `types.ts`, `api.ts`, `api-web.ts`, `api-tauri.ts`, `App.tsx`, `memberForm.ts`, `agentModels.ts`, `sendKey.ts`, `styles.css`, tests

## Tasks
1. Schema + Group/Member serde + create/archive/model APIs
2. Adapter model flags + chatbot model + scheduler wire
3. Perf metrics loop
4. Frontend: send key, archive UI, group kinds, model select, chatbot multi
5. Tests + `pnpm run test:gate` + canary deploy
