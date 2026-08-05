# PanelLive Mock MVP Implementation Plan

> **For agentic workers:** implement `/AI/WorkPanelLive` mock service + platform requirement docs; notify LinlisWorkPanel group via roadmap API + `@` message. No real cloud STT/TTS.

**Goal:** Runnable PanelLive Extend skeleton + explicit WorkPanel platform backlog.

**Architecture:** PanelLive owns media + mock STT/TTS; WorkPanel will later host Extension load/unload + A2A bus; Connecter stays observation-only.

## File map

| Path | Responsibility |
|---|---|
| `/AI/WorkPanelLive/package.json` | package metadata / start script |
| `/AI/WorkPanelLive/src/server.mjs` | HTTP server + mock STT/TTS |
| `/AI/WorkPanelLive/public/live.html` | WeChat-like push-to-talk UI |
| `/AI/WorkPanelLive/extension.manifest.json` | tab + skills contribution |
| `/AI/WorkPanelLive/README.md` | run instructions |
| `/AI/WorkPanelLive/docs/workpanel-platform-requirements.md` | API/protocol asks for WorkPanel |
| `docs/roadmap.md` | platform roadmap bullets |
| `docs/superpowers/specs/2026-08-05-panellive-mock-mvp-design.md` | design |

## Tasks

### Task 1: Scaffold WorkPanelLive mock server + UI

- Create files above; `npm start` serves UI and mock APIs.
- Smoke: curl health + stt/tts endpoints.

### Task 2: Docs + WorkPanel roadmap.md

- Platform requirements doc; update `docs/roadmap.md` with Extension Host / Live tab / A2A skills.

### Task 3: A2A notify LinlisWorkPanel group

- `POST /api/roadmap-items` for platform work items.
- `POST /api/messages` mentioning `@Codex` / `@Cursor Agent` / `@OpenClaw` with requirement summary.
