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


Code review of v1 (`feature/backend_abstraction`, 1455f9c, 2026-10-27), facts that outlive it:
- The shipped `Backend` trait has no configure step and `AppOptions`/`start_app_with` has no adapter hook, while `backend.ron`
  loads asynchronously AFTER the runner would install the adapter: a real adapter cannot learn endpoint/game_id. Fix shape: a
  runner-registered factory `FnOnce(&ResolvedBackendConfig) -> Box<dyn Backend>` called from `choose_backend`.
- `BackendAvailability::Loading` answers `backend.busy` and no load-complete event exists, so `scene.ready -> SignIn` races
  the backend.ron fetch (web vs native diverge).
- In-flight dedupe is per kind, so SignOut during Connecting enqueues; a late SignedIn reply can land after SignedOut.
- `game_id` defaulting to `project_id` is never charset-checked (runtime or CLI), and the runtime never computes the effective id.

Code review of v2 (`feature/nakama_adapter`, a601837, 2026-10-28), facts that outlive it:
- Engine `SignIn` while hub status is Online answers `backend.signed_in` without asking the adapter, so any adapter-side
  session loss (refresh failed) that only emits a per-op failure leaves the player stuck "Online" with every call failing.
  An adapter that loses its session must drive the hub to SignedOut (needs a reply variant or reuse of an unsolicited one).
- The `Backend` trait has no cancel: after the engine's 15 s `Time<Real>` timeout the adapter keeps its multi-stage chain
  (call -> 401 -> refresh -> retry -> rank) running, so a "timed out" submit can still land (Keep::Total double-counts on retry).
- ehttp 0.7 native spawns one OS thread per request (ureq 3 + rustls/ring); default timeout 30 s recv-body only; web fetch has none.
- Cargo resolver-2 feature unification: `-p ironhold_cli` alone has no ehttp, but a combined `-p cli -p native` / `--workspace`
  build links `client` into the CLI too.
- `?backend=` + localStorage device uuid = credential exfiltration vector (device id IS the account credential).
- Lua module guards only leaderboard/storage REST APIs; realtime socket (no `register_rt_before`), Link*, groups, tournaments,
  ListMatches etc. are open. Contract compose uses token_expiry 7200 s, so the refresh path is never hit against real Nakama.

Plan review of `public_backend_demo.md` (889f611, 2026-10-30), facts that outlive it:
- Runners build the `BackendFactory` BEFORE backend.ron loads and today decide key + device uuid eagerly (web/native
  `nakama.rs`), returning an error-only factory when the key is missing. Any data-side key (`server_key` in backend.ron) needs
  the key resolved INSIDE the factory closure from `ResolvedBackendConfig`, and device-uuid creation moved into the closure too,
  or every page load of every project writes a persistent id to localStorage.
- Precedence "runner key wins" lets a crafted `?nakama_key=` link or a stale `NAKAMA_SERVER_KEY`/`~/.nakama_server_key` break
  sign-in ("the server rejected the sign-in"); recommended "key travels with its endpoint": file key only for the file endpoint,
  dropped when `--backend`/`?backend=` overrides it.
- A real adapter never reports `backend.offline` for a down server: the factory makes no request, so availability is Ready and
  the failure only shows on SignIn (`backend.unreachable`, "server unreachable"). Cloudflare 429/5xx pages lack CORS -> same.
- github.io user/project Pages share ONE origin, so `ironhold.device_id` (the account credential) is readable by every repo's page.

**Why:** these were the load-bearing gaps; recurring if lobby/multiplayer plans are revised.
**How to apply:** any review of backend, lobby, cloud-save or Form 2 networking plans. See [[nakama-spike-facts]], [[determinism-networking]].
