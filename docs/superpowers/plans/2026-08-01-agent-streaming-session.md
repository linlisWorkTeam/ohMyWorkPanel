# Agent Streaming + Parts + Session Implementation Plan

> **For agentic workers:** Execute task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Stream agent output with thinking/artifact/final parts in one bubble, serialize same-agent runs, reuse Cursor CLI sessions per member, deploy canary only.

**Architecture:** `messages.content` stores JSON parts (legacy plain text = final). `ChatEvent.channel` tags deltas. Scheduler skips queued runs whose agent already has a running task. Cursor persists `agent_profiles.cli_session_id` and passes `--resume`.

**Tech Stack:** Rust (rusqlite/tokio), React/TS, deploy-canary.sh

## Global Constraints

- No breaking tauri command signatures; additive ChatEvent/Member fields only
- Additive SQLite column only (`cli_session_id`)
- Deploy canary `:8081` only; never touch prod DB
- Same-agent serial; cross-agent parallel under `max_concurrent_runs`

---

### Task 1: Message parts helpers + parse events

**Files:**
- Create: `src-tauri/src/message_content.rs`
- Modify: `src-tauri/src/adapters/parse.rs`, `src-tauri/src/lib.rs` (or mod declare)
- Test: unit tests in those modules

- [ ] Parts JSON append/read/plain helpers
- [ ] `parse_agent_event` → channel + text + optional session_id
- [ ] `cargo test` parse + message_content

### Task 2: ChatEvent channel + append_delta + schedule serial

**Files:**
- Modify: `models.rs`, `scheduler.rs`, emit sites that construct ChatEvent
- Modify: `src/types.ts`

- [ ] Add `channel: Option<String>` to ChatEvent
- [ ] `append_delta(..., channel)` writes parts JSON
- [ ] `schedule_group` skips agents already running
- [ ] `cargo test` / compile

### Task 3: Cursor session resume + streaming channels

**Files:**
- Modify: `db.rs` (ALTER cli_session_id), member load/save
- Modify: `adapters/cursor.rs`, `adapters/mod.rs`, `scheduler.rs` execute path
- Modify: mock stream for multi-channel smoke

- [ ] Persist/load `cli_session_id`
- [ ] `--resume` + short prompt when set; `--stream-partial-output`
- [ ] Parse session from stream; resume-fail retry once
- [ ] on_delta receives channel

### Task 4: Frontend bubble parts + delta UX

**Files:**
- Create: `src/messageContent.ts` (+ optional test)
- Modify: `src/App.tsx`, `src/styles.css` / `themes.css`, `src/types.ts`

- [ ] Parse/render parts; collapse thinking
- [ ] Apply delta by channel; hide empty typing once content exists
- [ ] Pending placeholder only for queued without output message

### Task 5: Verify + canary deploy + epitaph

- [ ] `cargo test`, frontend test if any
- [ ] `deploy-canary.sh`
- [ ] Smoke `:8081`
- [ ] Write epitaph v1.4
