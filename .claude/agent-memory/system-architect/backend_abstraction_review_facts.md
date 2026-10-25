---
name: backend-abstraction-review-facts
description: Plan-review facts for backend_abstraction.md (2026-10-25) - FSM exact-match events can't carry payloads, wasm socket !Send, game_id separator ambiguity, poll-trait model, hidden deps on save_load and fixed-tick pipeline
metadata:
  type: project
---

Plan-review of `planning/features/backend_abstraction.md` at `f302479` (2026-10-25), verdict: Needs more design work.

- **The FSM matches event names by exact string equality** (`fsm_interpreter.rs` `binding.event == *event_name`, transitions `t.on == *event_name`). Any event shaped `x.listed:{json}` / `backend.error:{code}` is unconsumable from RON. Lists (lobbies, leaderboard rows) need a data channel (bound widget or GameVariables), not an event payload. `networking_multiplayer.md` Form 2 has the same flaw (`multiplayer.lobby_listed:{json_payload}`) and uses different action/event names (`lobby_id:`, `multiplayer.*`) - one vocabulary must win.
- **Async external work precedent in core:** only `terrain.rs` (`AsyncComputeTaskPool` + `poll_once` on a `Task` component). No network precedent. core has `futures-lite`, no tokio.
- Recommended shape: a **poll-based, async-free trait** owned by core (`submit(req)` / `poll() -> Vec<BackendEvent>`), driven by ONE core glue system in a core-owned `GameStreamSet` slot; responses sorted by monotonically issued `request_id` before becoming `GameEvent`s. Adapters must not register systems against `GameplaySet` (schedule.rs says those sets are unstable outside core).
- **wasm:** `web_sys::WebSocket`/JsValue are `!Send`, so a `Box<dyn Backend + Send + Sync>` Resource fails on wasm; use a NonSend resource or keep JS handles inside `spawn_local` tasks behind Send channels.
- `game_id` + `.` separator is ambiguous (`a` + `b.c` == `a.b` + `c`); constrain the charset. Isolation is client-side only unless the server module enforces it (session vars carrying game_id + before-hooks).
- Hidden deps: cloud save needs `SaveState` from the unshipped `save_load_game_state.md`; the glue system's schedule must follow `gameplay_fixed_tick_pipeline.md` if the interpreter moves to FixedUpdate. Paused `Time<Virtual>` -> request timeouts must use `Time<Real>`; message buffers don't flip while paused (see [[real-pause-review-facts]]).
- Backend responses are per-client external inputs: they must never feed lockstep sim state un-replicated; record them in a replay stream like input.

**Why:** these were the load-bearing gaps; recurring if lobby/multiplayer plans are revised.
**How to apply:** any review of backend, lobby, cloud-save or Form 2 networking plans. See [[nakama-spike-facts]], [[determinism-networking]].
