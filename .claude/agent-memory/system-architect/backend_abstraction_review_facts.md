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

Code review of P2 (`feature/public_backend_demo`, uncommitted on 4ea6142, 2026-10-30): plan implemented as written
(`RunnerOptions` + `DeviceUuidSource` closure + `resolve_server_key` in ironhold_nakama; core only sets
`ResolvedBackendConfig::server_key = None` when `BackendEndpointOverride` is present). Residual facts:
- The factory cannot tell "file had no key" from "file key dropped by override", so the missing-key hint wrongly says
  "set server_key in backend.ron" under `--backend` (also when the override equals the file's endpoint).
- Runner keys (`?nakama_key=`, env) are never checked with `is_valid_server_key`; only the file key is (via `problems()`).
- The device uuid is created before `NakamaBackend::new` validates endpoint/game_id (safe only because `problems()` ran first).

Code review of P3 (`feature/public_backend_demo`, 8f7fdf5..4a7e030, 2026-11-03), facts that outlive it:
- `backend.display_name` = Nakama `session.username` (unique, auto-generated; "Player" if empty) -> deletion-by-quoted-name
  works. A committed public `server_key` is fine; first commit used a `PLACEHOLDER_SET_AT_GO_LIVE` value, no earlier real key.
- Key rotation is coupled to a full `/release` (main only advances via integration), so server-side rotation and the Pages
  deploy must be done together or the demo is broken in between; stale tabs then see "the server rejected the sign-in".
- `backend_demo_online` baseline is deterministic only because nothing auto-signs-in (no request at load). Adding
  `backend.ready -> SignIn` there would make test_web hit the live server.
- `backend_demo_online` ui/ snippets and assets/prefabs are byte-identical copies of `backend_demo`'s: hand-synced drift risk.

Plan review of v3 lobbies/matchmaker (9bc30b0, 2026-11-05), verdict Needs more design work:
- No v3 Action/RequestKind exists yet (only `BackendFeature::Lobbies/Matchmaker` in schema/backend.rs); v3 is 3 bullets.
- Public-read storage objects are owner-write only: joiners cannot bump "2/4", max_players is unenforceable, and a vanished
  host leaves the object forever. Needs a server RPC (server-owned objects, OCC version) or an authoritative labelled match.
  (Later found: a `register_req_before` on `RpcFunc` is accepted but NEVER runs in Nakama 3.41.0 - see below.)
- Engine `request_timeout_secs` default 15 s == Nakama matchmaker interval 15 s: FindMatch would time out. Needs a per-kind deadline.
- Engine-written vars clear only `leaderboard.` (backend.rs ~467); `lobby.`/`matchmaker.` are not under the reserved `backend.` prefix.
- MatchmakerAdd guard should REWRITE query/properties (prepend `+properties.game:{gid}`), not validate them.

Code review of `feature/lobby_rpc_allowlist` (931f207..53887d3, 2026-11-07), facts that outlive it:
- Nakama 3.41.0 has no working before-hook for REST RPCs; the only guard is wrapping the function at registration (`game_rpc`
  in ironhold.lua: id must match `^ironhold_lobby_[a-z_]+$`, needs `context.vars.game_id` in `games`, passes game_id to fn).
  The id allowlist is a self-lint (same author controls both sides); the session wrapper is the actual security boundary.
- Realtime `Rpc` stays in the `register_rt_before` closed list, so lobby RPCs are REST-only until v3b reopens it.
- Server-side `nk.storage_*` inside an RPC bypasses `collections_guard` (req_before hooks only see client APIs): game namespacing
  inside lobby RPCs is convention unless a helper builds the collection name.
- Nakama loads each module via `require(name)` per Lua VM, so a module required by another (ironhold_games precedent, now
  ironhold via test_rpcs.lua) executes once per VM; modules re-run in every pooled VM (leaderboard_create logs repeat - pre-existing).
- The CLI embeds ironhold.lua at build time: `ironhold backend config --out` from a stale tools/bin binary ships the OLD module.
- docs/20 server-admin text says "copy both" but not "and no other module" - the operator rule lives only in a Lua comment.

Plan re-review of v3a (f8c83c8, 2026-11-07), verdict Needs more design work (one short round, no new primitive):
- The `Backend` trait has no clock (`poll(&mut self, out)`), and `std::time::Instant` panics on wasm32-unknown-unknown: an
  adapter-driven heartbeat needs a time source. Recommended: core issues heartbeat as a normal `RequestKind` on `Time<Real>`
  from the glue system (own dedupe key, real request id, timeout/cancel apply, Fake stays deterministic); id 0 then stays
  for ConnectionLost/Reconnected only. `Vec::sort_by_key` is already stable, so "id-0 tie-break" is a non-issue.
- REST adapter never emits ConnectionLost/Reconnected; synthesizing `backend.connection_lost` in v3a leaves status stuck
  "connecting" (hub.reconnecting waits for a Reconnected nothing sends).
- Hidden web tabs stop requestAnimationFrame -> Bevy Update stops -> no heartbeats -> 30 s expiry drops alt-tabbed players.
- `owns()` prefix check ACCEPTS client storage ops on `{game}.lobbies`; "clients never touch the collection" needs an explicit deny.
- Generated `ironhold_games.lua` carries no `features`: lobby RPCs answer every hosted game (incl. backend_demo_online).
- `game_rpc` masks non-table errors: an OCC version-conflict from `nk.storage_write` becomes "the request failed" unless the
  RPC fn pcalls/retries it itself.
- Reserved prefixes are string literals in 2 places (runtime retain `leaderboard.`, CLI `backend.`/`leaderboard.`): no shared const.
- No generic "action's feature not declared" check exists in backend_checks.rs; v3a must add it (v1 task text implied it existed).
- Cloudflare: one counter 30 req/10 s/IP for sign-up + /v2/rpc; staging is reached over LAN, so dev playtests never exercise it.

Code review of v3a-1 (`feature/backend_v3a_1`, 7ce2f5d..a06cdfc, 2026-11-07), facts that outlive it:
- Core's "SignOut enqueues LeaveLobby first, adapters process in order" is NOT guaranteed by the `Backend` trait: the Nakama adapter
  fires each request as its own concurrent ehttp fetch, and its SignOut does `cancel_all` (kills the Leave chain, no 401 refresh) +
  fire-and-forget logout in the same submit loop. v3a-2 must defer logout until an in-flight LeaveLobby finishes (or state the contract).
- `FakeBackend::reply_for` mutates state at SUBMIT time and never expires members; with Create/Join refusing `already_in_lobby` when
  `mine` is set it contradicts decision 20 (create/join REPLACE the old membership). Any core-side forget (heartbeat-expiry close,
  SignOut with Create/Join in flight, Create/Join timeout) leaves the fake "in a lobby" forever: `?fail=heartbeat` demo dead-ends.
- `LobbyStarted` (host's own Start reply) does not clear `LobbyTracker::trouble`, and `lobby_tick` checks expiry BEFORE the started
  early-return: one earlier failed heartbeat -> `lobby.closed:server` 60 s after a successful start.
- `LobbyInfo` has no server version/gen (plan reply shape has `gen`): a heartbeat sent before StartLobby whose reply lands a frame
  after `LobbyStarted` un-starts the lobby, resumes heartbeats, and fires `lobby.started` twice (Nakama only; fake is synchronous).
- `GameVariables` has no change-detection consumer today (labels poll by value), so the `put()` changed-only writes are forward-compat only.
- Runtime sign-out clearing still uses literals, not `RESERVED_VARIABLE_PREFIXES` (it can't use the whole const: `backend.` would wipe
  the status it just set). `fixed_reason` is not op-specific: an adapter `host_left` on a join failure emits an event validate rejects.

Plan review of v3a-2 `backend_lobbies_server.md` (01fded1, 2026-11-08), verdict Needs more design work (one round):
- Core ALREADY enqueues `LeaveLobby` before `SignOut` (dispatch, when in a lobby or a create/join/leave is in flight): an adapter that
  originates its own leave duplicates it. The adapter's real job: hold the leave until an in-flight Create/Join has finished (concurrent
  fetches let the leave hit the server first = ghost), then logout.
- The adapter has no clock, so "cap the deferred logout at the request timeout" is unimplementable in the adapter; Leave and SignOut start
  the same frame with equal timeouts, and a timed-out SignOut maps to `Unreachable` (`backend.unreachable`, no var cleanup, no
  `backend.signed_out`). Needs a tiny core change (SignOut failure -> SignedOut) or reply SignedOut at once and hold the next SignIn.
- `UpdateAccount` is NOT in ironhold.lua's closed list: a key holder can rename itself; lobby/leaderboard names are not "server-made".
- `is_valid_lobby_name` allows `"` and `\`: the games.lua generator must escape (Lua injection into server config otherwise).
- `the_contract_fixture_is_what_the_generator_writes` pins fixture == generator output, so a "fixture-only" field must either be
  generator-emittable (a production path) or live in a separate test-only module (test_rpcs.lua precedent).
- Listing the whole `{game}.lobby_hb` collection grows with every user who ever played (rows never deleted); batch-read host keys instead.
- `nk.time()` is documented as milliseconds (plan says seconds); math.random in pooled VMs is a collision risk, use nk.uuid_v4 + `"*"`.
- Every `lobby_of` write must be versioned too (a sweep clearing another user's index races that user's own create/join).

**Why:** these were the load-bearing gaps; recurring if lobby/multiplayer plans are revised.
**How to apply:** any review of backend, lobby, cloud-save or Form 2 networking plans. See [[nakama-spike-facts]], [[determinism-networking]].
