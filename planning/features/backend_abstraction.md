# Feature: Backend abstraction (core contract + Nakama adapter)

_Status: In Progress (v1 and v2 Done, v3a Ready and split into v3a-1/v3a-2 by plan-review round 3 on 2026-11-07, v3b and v4 Queued; the public demo `backend_demo_online` with the optional `server_key` shipped through `planning/features/done/public_backend_demo.md`, 2026-11-03). Plan-review round 1 done 2026-10-25; all seven decisions answered 2026-10-27, see "Decisions (answered)"._
_Planned at: `cd90a12` (2026-10-25)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Contract in core (Actions, events, engine-written variables, poll trait, glue system) + `FakeBackend`; sign-in and leaderboard only; nothing networked | Done | 8745ef9 (2026-10-27) |
| v2 | `ironhold_nakama` adapter, **native and wasm32 on the LAN**: sign-in, leaderboards, server module with `game_id` enforcement; cloud save once local save exists | Done | e97da79 (2026-10-28) |
| v3a-1 | Lobbies, no server: core actions/events/variables, core-driven heartbeat, `FakeBackend`, validate checks, `backend_demo_lobby` on `fake://`, designer docs | Done | 891cf8b (2026-11-07) |
| v3a-2 | Lobbies, server side: six `ironhold_lobby_*` RPCs behind `game_rpc`, storage guard, features gate, contract tests, adapter, staging playtest | Ready (needs v3a-1) | — |
| v3b | Realtime socket and matchmaker (`FindMatch`, `CancelMatch`), "transport preview": fake and contract test only, not on the public demo | Queued | — |
| v4 | Stage 2: `wss://` from an `https` page, tunnel latency and idle sockets, half-open drop | Queued | — |
| later | Relay transport: only after the sync library is chosen (see "Relay seam") | Icebox | — |

## What

A game author gets online features without writing code: a project's `backend.ron` (found through `backend_path` in
`.project.ron`) declares which online features the project uses, and RON actions (`SignIn`, `SubmitScore`, `FetchLeaderboard`, ...)
talk to whatever backend the engine was built with. Results come back as pipeline events plus **engine-written variables** a
designer can bind to Labels (`Label.bind`). With no backend configured or reachable the project starts and plays normally; online
actions answer with a clear event instead of failing. The first and probably only real adapter is Nakama.

## Why

The Nakama spike (`planning/investigations/nakama_backend_viability.md`, section 8c) concluded: go for Nakama as the meta backend.
Building accounts, storage, leaderboards, lobbies and matchmaking ourselves is the larger, riskier job; the investigation also said
to put them behind one abstraction so the adapter stays swappable and `ironhold_core` stays platform-agnostic. Spike facts that shape
the design: REST 8-22 ms; session token 60 s (401 on expiry at connect); a slow reader is dropped after about 3 s; usable message
size about 2.9 KB (JSON) or 4 KB (protobuf); matchmaker result about one server interval (default 15 s) after the ticket;
leaderboards can only be created by server code.

## Plan-review round 1 (2026-10-25), what changed

System-architect and UX review both returned "needs more design work"; no new pipeline primitive is needed. Changes made here:
event payloads removed (the interpreter matches event names exactly, so JSON or error codes in an event name can never be bound);
the poll-trait model chosen; two crates instead of three; `backend_path`/`schema_version` added; failure events per operation;
async and determinism rules written down; wasm moved into v2; the relay trait dropped; server-side `game_id` enforcement added;
dependencies on the unshipped local save plan made explicit.

## Plan-review round 2 (2026-11-05), what changed

System-architect and UX review of v3 both returned "needs more design work" (the section was three bullets). All eight resulting decisions were
accepted by Frank the same day (see "Decisions (answered 2026-11-05)"): v3 is split into **v3a** (lobbies over REST and server-side RPCs, no
realtime socket) and **v3b** (socket and matchmaker, "transport preview"); lobbies are server-owned objects behind generic Lua RPCs instead of
public-read objects a client writes; a waiting-room vocabulary, a start signal and fixed failure reasons were added; the matchmaker gets its own
deadline. v3a is Ready; its prerequisites are tracked as backlog items (see v3a).

## Approach

### Contract in core, adapter outside

```
ironhold_core     owns the contract: BackendRequest / BackendEvent, the `Backend` poll trait, the glue system, FakeBackend. No network, no
                  async runtime, no platform code.
ironhold_nakama   the Nakama adapter (REST + realtime socket, protobuf envelope; native and wasm32 transports). Depends on core types only.
ironhold_native/web   runners insert the adapter (the only place that knows which backend is built in).
```

(Proposed: two crates. A separate trait crate would have no consumer: core's executor already builds the requests.)

**One adapter model, a poll trait owned by core:**

```rust
trait Backend { fn submit(&mut self, req: BackendRequest); fn poll(&mut self, out: &mut Vec<BackendEvent>); }
```

- The executor arm for an online `Action` pushes a `BackendRequest` (with a `request_id` from a core counter, issued in executor
  order) onto a **resource outbox** (`VecDeque`), not a Bevy `Message` (message buffers drop after two flips if a reader is gated and
  never flip while paused). The outbox joins `SceneStateParams` because `action_executor_system` is at Bevy's 16-parameter limit.
- **One glue system in a new core-owned slot** (`GameStreamSet::Backend`, after `DelayedEvents`) drains the outbox into
  `Backend::submit`, calls `poll`, sorts each frame's batch by `request_id`, and emits `GameEvent`s and variable writes. No
  adapter-registered system orders itself against the unstable core sets (`schedule_order_tests` would fail).
- The adapter keeps its I/O behind channels inside `poll()`. **I/O never uses Bevy's task pools** (single-threaded on wasm; blocking
  sockets would starve compute tasks on native).
- **wasm `!Send`:** the backend lives in a `NonSend` resource (the glue system takes `NonSendMut`); JS handles stay inside
  `spawn_local` tasks and only `Send` channels cross into the adapter.
- **Native I/O:** decided in the v2 spike-to-crate step (task below): a dedicated thread owning the socket (blocking tungstenite) or
  a current-thread tokio runtime on its own thread; not `ewebsock` natively (10 ms poll, no `TCP_NODELAY`), and the Windows
  tokio-timer lateness found by the spike must be addressed if timers are used.
- `FakeBackend` is deterministic: its latency counts polls/frames, never wall-clock; it can inject failures and timeouts.
  Selected by `endpoint: "fake://"` in `backend.ron` (and by tests).

### Determinism and async rules

- Backend responses are **per-client external inputs**. They must never change replicated or lockstep state on one peer only
  (e.g. a cloud load restoring variables mid-session); for replay they are recorded like input. A subsection goes into
  `docs/dev/40_determinism_and_networking.md`.
- Timeouts use `Time<Real>` (virtual time is frozen while paused; a sign-in started from the pause menu must still time out).
- Responses arrive seconds later, possibly after a state change or `LoadScene`. **Online replies are not cleared by `LoadScene`
  and arrive while paused**; bindings for them belong in `global_on` (same trap as `EmitEventAfterDelay`). Documented in
  `docs/30_runtime_events_and_logic.md`, including same-frame order.
- A duplicate request while one of the same kind is in flight is ignored with `backend.busy` (documented); retry = fire the action again.
- If `gameplay_fixed_tick_pipeline.md` moves the interpreter into `FixedUpdate`, the glue system moves with it (dependency).

### Events and variables (no payloads in event names)

Event names follow the existing `<category>.<verb>:<id>` rule with ids the designer can write; **no JSON and no error codes in names**.
Failure events exist **per operation** and carry that operation's id. Data goes to engine-written, reserved variables (like
`audio_volume_percent`).

| Area | Actions | Events | Engine-written variables (reserved `backend.` prefix, excluded from `SaveState`) |
|---|---|---|---|
| Account | `SignIn`, `SignOut` | `backend.signed_in`, `backend.signed_out`, `backend.sign_in_failed`, `backend.offline` (nothing configured), `backend.unreachable` (server down), `backend.busy`, `backend.connection_lost`, `backend.reconnected` | `backend.status` (`offline`/`connecting`/`online`), `backend.display_name`, `backend.last_error` (text, for a Label) |
| Leaderboard | `SubmitScore(leaderboard:, score_var:)`, `FetchLeaderboard(leaderboard:, count:)` | `score.submitted:{leaderboard}`, `score.submit_failed:{leaderboard}`, `leaderboard.listed:{leaderboard}`, `leaderboard.list_failed:{leaderboard}` | `leaderboard.<name>.count`, `leaderboard.<name>.<n>.name`, `.<n>.score`, `.my_rank`, `.my_best` (n = 1..count, fixed max 10) |
| Cloud save (after local save) | `SaveToCloud(slot:)`, `LoadFromCloud(slot:)`, `DeleteCloudSave(slot:)` | `cloud.written:{slot}`, `cloud.loaded:{slot}`, `cloud.empty:{slot}`, `cloud.save_failed:{slot}`, `cloud.load_failed:{slot}`, `cloud.conflict:{slot}` | — |
| Lobbies (v3a) | `ListLobbies`, `CreateLobby(name:, max_players:)` (name from `lobby_names`), `JoinLobby(row:)`, `LeaveLobby`, `StartLobby` (host only) | `lobby.listed`, `lobby.created`, `lobby.joined`, `lobby.left`, `lobby.started`, `lobby.member_joined`, `lobby.member_left`, `lobby.closed` (+ `:<reason>`), `lobby.list_failed`, `lobby.create_failed`, `lobby.join_failed`, `lobby.start_failed` (each + `:<reason>`), `lobby.connection_trouble`, `lobby.row_filled:<n>`, `lobby.row_empty:<n>`, `lobby.list_empty`, `lobby.member_row_filled:<n>`, `lobby.member_row_empty:<n>` | reserved `lobby.` prefix, excluded from `SaveState`: `lobby.count`, `lobby.<n>.name`, `.players` ("2/4"), `.host`, `.id` (internal) (n = 1..10); `lobby.current.name`, `.players`, `.is_host` ("yes"/"no", display only), `.id` (internal); `lobby.member.<n>.name` (n = 1..8, host first); `lobby.last_error` (detail line, see v3a) |
| Matchmaker (v3b) | `FindMatch(min:, max:, timeout_secs:)`, `CancelMatch` | `matchmaker.searching`, `matchmaker.matched`, `matchmaker.cancelled`, `matchmaker.timed_out`, `matchmaker.failed` | `matchmaker.status` |

- **Lists reach the UI as indexed variables plus row actions** (`JoinLobby(row: 3)`): a designer builds a list from N Labels and N
  Buttons they already know; no list widget and no text-input widget is needed. (Proposed; alternative is engine-owned panels like
  `ShopPanel`. Decision 1.)
- Unit variants are fine (`SignIn`, `ListLobbies`, like `Quit`); everything with an argument uses named fields.
- Cloud-save names mirror `save_load_game_state.md` (`SaveGame(slot:)`, `save.written:{slot}`): `cloud.written`/`cloud.loaded`/`cloud.load_failed`.
- Lobby actions and events in `networking_multiplayer.md` Form 2 are **superseded by this plan** (that file gets a cross-reference; one
  set of lobby actions in `actions.rs`).

### `backend.ron` (found through `backend_path`)

```ron
// assets/projects/<name>/backend.ron   (referenced by `backend_path: Some("backend.ron")` in .project.ron; no convention-path fallback)
BackendConfig(
    schema_version: 1,
    game_id: "ironhold_demo",                    // [a-z0-9_]{1,32}; defaults to project_id when omitted
    endpoint: Some("https://nakama.example.org"), // None = offline project; "fake://" = in-memory test backend
    features: [CloudSave, Lobbies, Matchmaker],
    leaderboards: [
        Leaderboard(name: "best_time", ranking: LowerIsBetter, keep: Best),   // designer terms, mapped to Nakama's sort/operator
    ],
)
```

- `ProjectConfig` gains `backend_path: Option<String>`; a failed `backend.ron` load logs an `error!` and the project runs offline
  (never a loading hang). `ProjectConfig` is `deny_unknown_fields`, so an older engine rejects a newer `.project.ron`; noted under schema evolution.
- `score_var` is a `GameVariables` text that must parse as an integer; a failure gives `score.submit_failed:{leaderboard}` and a
  warning, never a silent 0.
- The shipped demo needs something to submit: it uses a `score` variable (the engine has no run timer; "best time" is not offered
  until one exists).

### Several games on one Nakama

- `game_id` prefixes storage collections (`{game_id}.saves`, `.lobbies`), leaderboard ids, match labels and the matchmaker query;
  device ids are `{game_id}:{device_uuid}` (separate account per game by default).
- **Separator safety:** `game_id`, leaderboard names and slots are limited to `[a-z0-9_]` (length caps); no `.`, `:` or spaces, so
  `"a" + "b.c"` and `"a.b" + "c"` can never collide. Enforced in the loader and `ironhold validate`; `game_id` must also be unique
  across the repo's projects (`--strict`). Nakama's device-id charset/length for the `:` is checked in v2.
- **Client side, namespacing is structural:** newtypes (`NsCollection`, `NsLeaderboardId`, `NsMatchName`) only a `GameNs` can build,
  and the adapter's REST functions accept nothing else, so a forgotten prefix is a compile error.
- **Server side, it is enforced (v2):** the generic server module sets a `game_id` session variable at authentication and adds
  before-hooks that reject storage, leaderboard and matchmaker calls whose prefix or `+properties.game:` term does not match it.
  Client-side prefixing only stops honest bugs; anyone with the shared server key could otherwise read another game's public lobbies
  or write to its leaderboard. (If this is deferred, the plan must say a single trusted-games server is accepted; Decision 6.)
- Matchmaker `game` property is a required term; v3 tests that ids with underscores match exactly.
- One instance is fine while every game is ours; a third party's game gets its own Nakama + Postgres.
- The server key ships in a web build and is a client identifier, not a secret; the **http key (server-to-server) never appears in
  `backend.ron` or generated config**.

### Server module and deployer

Leaderboards are created only by server code, so the engine ships **one generic Lua module** (no build step, used by the spike) whose
config is generated from `backend.ron`: the generator is a pure function in `ironhold_nakama` behind a no-network feature, called by
the CLI command `ironhold backend config <project>` (the CLI does not depend on network crates). Documented as a **server-admin
step**: adding, renaming or removing a leaderboard needs a regenerated config and a redeploy; renaming orphans old scores. A submit
to a leaderboard the server does not know maps to `score.submit_failed:{leaderboard}` with a log line naming the missing redeploy.
Per-environment overrides of `endpoint` (web: URL parameter; native: env var/argument) are a v2 task.

### Relay seam (deferred)

No `NetTransport` trait now: the sync library (Lightyear / replicon / matchbox) is still undecided and brings its own IO model. Two
things are kept instead: matchmaker and lobby results carry a **match handle** as data (match id/token, presences) toward the later
transport; and the adapter's connection manager is designed to be **shared** (matchmaker and relay use the same realtime socket and session). The
constraints for whoever builds it: payload ceiling about 4 KB (protobuf) or 2.9 KB (JSON), reliable-ordered over TCP, slow-reader
drop after about 3 s, reconnect expected, session refresh before every reconnect. Realtime default is the protobuf envelope (same
latency, higher ceiling, +34 KB wasm).

### Dependencies on other plans

- `save_load_game_state.md` (still Draft): cloud save reuses its `SaveState` (one save format); `backend.*` variables are excluded
  from saves. Until it ships, v1/v2 cover sign-in and leaderboards only.
- `gameplay_fixed_tick_pipeline.md`: glue-system placement (above). `networking_multiplayer.md`: lobby vocabulary (above).
- Bevy 0.19 upgrade: only the glue/plugin code touches Bevy.

## Tasks

### v1 — contract and fake
- [x] `backend_path` in `ProjectConfig`; `BackendConfig` schema with `schema_version`; load error handling; `ironhold validate` checks:
      id charset/length (`game_id`, leaderboard, slot), no duplicates, undeclared leaderboard/feature used by an action (error),
      online action without `backend.ron` (warning), `endpoint` scheme (https; http only for localhost/LAN with a warning),
      `FindMatch` bounds, `--strict`: (deferred, see notes) bindings on result events no action can produce, `score_var` never written, unique `game_id` repo-wide
- [x] Online `Action` variants and events/variables above (schema, executor arm, CLI `query`, `cargo check -p ironhold_cli`)
- [x] `Backend` poll trait, `BackendRequest`/`BackendEvent`, outbox resource in `SceneStateParams`, glue system + `GameStreamSet::Backend`
      (`schedule_order_tests`), engine-written variable system (one writer, `GameplaySet::PostExecute`), `Time<Real>` timeouts
- [x] `FakeBackend` (frame-counted latency, injected failures, selected by `fake://`)
- [x] Tests: every action to its event and variables with the fake, offline project, failure per operation, busy, replies after `LoadScene` and
      while paused, `global_on` binding, determinism (batch sorted by `request_id`)
- [x] Demo project via `/new-project` (one screen, no 3D): sign-in status, a `score` counter + Submit, a top-5 leaderboard of Labels;
      labelled "offline test backend" when `fake://`; states which backend the hosted web build uses (the fake, labelled)
- [x] Default UI snippets (answered follow-up to decision 1, 2026-10-27): the demo project ships ready-made, copy-and-edit blocks as separate files
      in a `ui/` folder (`leaderboard.ui.ron` top-5 rows, `signin.ui.ron` status + button), documented as paste-in RON. Plain files for now
      (the leaderboard name is baked into the variable names; the docs say to find-and-replace it). When the "UI fragments" backlog item lands,
      the same files become parameterised fragments. Not an engine feature in this plan.
- [x] Docs: `docs/20_data_formats.md` (`backend.ron`, `backend_path`, file naming table, actions, engine-written variables),
      `docs/30_runtime_events_and_logic.md` (online events, same-frame order, searching/timeout state-machine example, save/fail
      pattern), `docs/15_authoring_tools.md` (validate rows, `ironhold backend config` as a server-admin step), `docs/STATUS.md`,
      README line (online is optional), `docs/dev/40_determinism_and_networking.md` (backend responses are per-client inputs)

**Implementation notes (v1, 2026-10-27; review round on `feature/backend_abstraction`):**
- The leaderboard action is named `FetchLeaderboard` (renamed from `ShowLeaderboard` on 2026-10-27): it only fetches rows into variables and shows nothing by itself.
- Variables are written in three places (`dispatch_backend_action`: `connecting`/`last_error`; the glue system: replies; the loader), in `Emit`/`Execute`
  rather than one `PostExecute` writer: a reply's variables are then visible to the interpreter in the frame its event arrives.
- Added beyond the plan: `backend.ready` (and `backend.offline` once loading ends without a backend) so an automatic sign-in has an event to bind
  (a sign-in fired earlier answers `backend.busy`); status `signed_out` (working backend, nobody signed in) separate from `offline` (no usable backend);
  the reason a project is offline goes to `backend.last_error`; `fake://?latency=N&fail=...` so failure screens can be rehearsed;
  optional `request_timeout_secs`; `reserved_variable_write` strict warning; sign-in and sign-out share one in-flight slot; submitting while still
  signing in answers that operation's own failure event.
- Deferred (backlog): the `--strict` check for bindings on result events no action can produce; a validate warning for a `Label` bound to
  `leaderboard.<x>.` with `<x>` undeclared; operation ids on `backend.busy`/`backend.offline`; a deadline for a `ConnectionLost` that never
  reconnects (adapter's job: it must send `Reconnected` or `SignedOut`); tests for a reply surviving `LoadScene` and the `fake://` load path through a
  real `backend.ron` asset (covered by the `backend_demo` playtest).
- **For v2 (decide before the adapter crate starts):** the adapter cannot learn its configuration today (`Backend` has only `submit`/`poll`, and
  `backend.ron` loads after the runner installs the adapter). Proposed: the runner registers an adapter factory in `AppOptions`
  (`FnOnce(&ResolvedBackendConfig) -> Box<dyn Backend>`, called once `backend.ron` resolves; `ResolvedBackendConfig` = endpoint, effective `game_id`,
  leaderboards). A `fake://` endpoint keeps selecting the built-in fake even if an adapter exists. The adapter must drop a server session for a
  request the engine already timed out, and must not allocate or cross the JS boundary every frame when nothing arrived.

### v2 — Nakama adapter

**Design decisions (2026-10-27, before the adapter crate):**
- **Hand-off:** the runner puts an adapter factory in `AppOptions` (`BackendFactory`, called once when `backend.ron` resolves with a non-`fake://`
  endpoint, given `ResolvedBackendConfig` = endpoint, effective `game_id`, leaderboards). No factory = offline with the existing reason text.
- **I/O model, narrowed:** v2 is request/response only (sign-in, submit, list), so both targets use `ehttp` callbacks that push into an
  `mpsc` channel drained by `poll()`; no thread of our own, no tokio, same code on native and wasm32, nothing `!Send` crosses the channel.
  The realtime socket (matchmaker, reconnect, relay) arrives in v3, where the thread-vs-tokio question from the Approach is decided; the
  `ConnectionLost`/`Reconnected` replies stay unused until then.
- **Slices:** (a) core factory hand-off + test; (b) `ironhold_nakama` REST client (sign-in with device id, session refresh, submit, list, `GameNs`
  newtypes) native, tested against the LAN Nakama; (c) Lua module + `ironhold backend config` + `game_id` hooks + adversarial test;
  (d) wasm32 build and browser check; (e) endpoint override, account deletion path, docs.
- [x] `ironhold_nakama`: REST client + session refresh + `GameNs` newtypes; protocol layer from tag `spike/nakama_backend`; Nakama device-id charset check
- [x] Decide native I/O model: v2 is request/response only, so `ehttp` callbacks into a channel on both targets (see Design decisions); the realtime socket question moves to v3
- [x] Generic Lua module with `game_id` session variable and before-hooks; `ironhold backend config`; pinned server version; contract tests against
      a local Nakama container (script, not hosted CI); adversarial test: a client for game A sending game B's ids is rejected
      (`crates/ironhold_nakama/tests/contract/run.sh`; also covers spoofed session vars at sign-in and refresh, unknown games, other sign-in methods)
- [x] Endpoint override per environment (`--backend`, `?backend=`); sign-in, `SignOut`, leaderboard submit/list
- [ ] Account deletion/export path (before any public deployment)
- [ ] **Blocked on `save_load_game_state.md` shipping:** cloud save/load reusing `SaveState`, with the conflict event (do not start with a separate format)

**Implementation notes (v2, 2026-10-28; review round on `feature/nakama_adapter`):**
- `Backend` gained `cancel(request_id)` (called when the engine times a request out) so a refresh-and-retry chain stops; `SignOut` answers every request in
  flight with a failure and forgets it, so a late sign-in/refresh/list can never bring the old session or the old player's board back; one shared session
  renewal serves all calls that got a 401 (refresh token first, the device id if that is refused).
- The web page address can only select `fake://` (a link must never choose which server receives a player's device id); a real endpoint comes from `backend.ron`.
  Native `--backend` accepts any endpoint.
- The runners always supply a factory; with no key or no device-id storage it fails with the reason on screen (`backend.last_error`), and a factory's own error
  text is passed through (written for designers, never contains the key).
- The Lua module is deny-by-default (matches, chat, groups, friends, tournaments, account linking, realtime) and also guards the declared board list and
  strips a client's per-write operator. Verified by `crates/ironhold_nakama/tests/contract/run.sh` against a local container (not the homelab yet).
- Deferred: see the backlog item "Backend v2 follow-ups".

### v3a — lobbies over REST (Ready, split into v3a-1 and v3a-2 on 2026-11-07)

Plan-review round 3 (2026-11-07, system-architect and ux-gamedesigner-reviewer; Frank accepted every recommendation, decisions 16-27 below) split v3a
like v1/v2 were split: **v3a-1** is everything that needs no server (core, `FakeBackend`, validate, the demo on `fake://`, designer docs); **v3a-2** is the
server side (Lua RPCs, contract tests, adapter, staging playtest). v3a-1 can be built and playtested without Docker or the homelab.

**Prerequisites.** Done: the `backend/` folder (local Nakama; the module stayed in `crates/ironhold_nakama/server/`); the homelab staging Nakama
(2026-11-07, reached with a native `--backend`, a built web `pkg/` cannot be re-pointed); the guarded `game_rpc` registration in `ironhold.lua`
("reopen the closed server APIs", 2026-11-07: Nakama 3.41.0 has no before-hook for REST RPC calls, so the guard lives at registration; it passes the
session's game id to the function and cannot protect another module's RPCs, so `ironhold.lua` must be the only module installed); the homelab Cloudflare rate
rule (2026-11-07: sign-up and `/v2/rpc/` share one counter, 30 requests per 10 s per IP, Block). Open, **before lobbies are enabled on any public project**
(including `backend_demo_online`): "Bind the device id to the game". Lobbies are not turned on in `backend_demo_online`; `backend_demo_lobby` carries them.

#### Rules both halves share (decisions 16-27)

**Who drives what (decisions 16, 17).** Core drives the heartbeat, not the adapter: the `Backend` trait has no clock and `Instant` panics on wasm32. While in a
lobby, the glue system issues a `LobbyHeartbeat` request every 10 s (plus a little jitter) on `Time<Real>`, as a normal request with its own dedupe key and a
real request id; the existing timeout and `cancel` machinery applies and `FakeBackend` stays deterministic. The reply is the whole lobby. **Core diffs it**
against the previous one and emits `lobby.member_joined`, `lobby.member_left`, `lobby.started` and `lobby.closed` in one place, so the fake gets them for free.
Request id 0 stays reserved for `ConnectionLost`/`Reconnected`, which v3a does not emit (decision 25). Every lobby reply carries the lobby id and a membership
generation; core drops a reply that does not match the current membership, so a late heartbeat can never bring `lobby.current.*` back after `lobby.left`,
`lobby.closed` or `SignOut`. The heartbeat's dedupe key differs from create/join/leave/start, so `LeaveLobby` never answers `backend.busy` because a heartbeat
is in flight. Core keeps the lobby-id snapshot behind `JoinLobby(row:)` itself and never reads `lobby.<n>.id` back from `GameVariables` (a designer can overwrite it).

**Timing (decision 18).** Heartbeat every 10 s with jitter; a member and a lobby expire after **60 s** without one. Both are fixed constants shared by the engine
and the generated Lua, not designer-tunable (two sides that drift would drop players). Why: the shared Cloudflare counter allows 30 requests per 10 s per IP and
a full 8-player lobby at 5 s would use 16 of them; a hidden browser tab stops Bevy's `Update` (and so the heartbeat), and 60 s survives a short alt-tab (a player
who stays away longer comes back to `lobby.closed:removed`). The engine enforces the 5 s minimum between `ListLobbies` (a call inside that window is answered
from the last snapshot). A native 429 maps to the fixed reason `rate_limited` with no retry; on the web a Cloudflare block looks like a network error. Before any
public lobby, test through the Cloudflare path with N clients (staging is on the LAN and never meets the limit).

**Lifecycle (decision 20).** A started lobby is no longer listed and refuses joins (`started`); members stop heartbeating once they have seen
`lobby.started`; the server object expires on its own; `lobby.current.*` survives `lobby.started` until `LeaveLobby`/`SignOut`. After a page reload the user is
the same but the client has no lobby state: `CreateLobby` and `JoinLobby` **replace** the old membership on the server, and `LeaveLobby` without local state still
calls the server's leave ("leave whatever I am in"). `SignOut` calls leave first, then the fire-and-forget logout. `LoadScene` does not leave.

**Failures (decision 21).** The server answers a business outcome as `{"ok":false,"reason":"full"}` (HTTP 200); `error()` is only for auth problems and bugs, so the
module's error masking can never hide a domain reason. The engine fires the generic failure event and **then** the reason form, so the specific message wins:
`lobby.join_failed` then `lobby.join_failed:full`; the same for `list`, `create`, `start`, and `lobby.closed` then `lobby.closed:host_left`. `lobby.last_error` stays as
a detail line (like `backend.last_error`). Fixed reasons: `full`, `gone`, `list_out_of_date`, `no_lobby`, `already_in_lobby`, `limit_reached`, `not_host`,
`not_signed_in`, `not_in_lobby`, `started`, `too_few_players` (a host needs at least 2 members to start), `not_enabled`, `rate_limited`, `busy`, `server`; `lobby.closed`
carries `host_left`, `removed`, `server` only, and those two never appear as failure reasons (core narrows each family; anything else an adapter reports becomes `server`).
`list_out_of_date` was dropped (a server cannot tell it from `gone`). A lobby action issued while its own kind is in flight answers `lobby.<op>_failed:busy`, not `backend.busy`.

**Rows and the empty state (decision 24).** State-machine rules cannot test a variable, so a designer cannot hide a row from `lobby.count`. After every list write
the engine fires `lobby.row_filled:<n>` / `lobby.row_empty:<n>` for n = 1..10 and `lobby.list_empty` when there is nothing; for the waiting room
`lobby.member_row_filled:<n>` / `lobby.member_row_empty:<n>` for n = 1..8. A designer writes one `SetUiVisible` rule per row Group. (The general "hide a bound Label
or Group when empty" UI feature stays its own backlog item; it would let a later version drop these events.)

**Waiting room (decisions 9, 25).** `lobby.member.<n>.name`: the host is member 1, the player's own name is in the list, members are ordered by server join order and
compacted when one leaves; `lobby.member_joined`/`member_left` fire only for other players. `lobby.<n>.host` (the host's display name) is exposed so rows can be
told apart. All `lobby.member.<n>.*` and `lobby.current.*` are cleared together by `lobby.left`/`lobby.closed`; `lobby.<n>.*` by `SignOut` and when a list comes back
empty. Variables are written only when the value changed (a heartbeat reply must not rewrite about 20 `GameVariables` every few seconds). Host gating: show Start on
`lobby.created`, hide it on `lobby.joined`; `lobby.current.is_host` is for display only. `lobby.<n>.id` / `lobby.current.id` are documented as engine-internal, not for the screen.

**Connection (decision 25).** The first failed heartbeat fires `lobby.connection_trouble` (a designer shows "Reconnecting..."); a success afterwards fires nothing; past the
expiry window the engine drops its lobby state and fires `lobby.closed:server`. v3a does **not** synthesize `backend.connection_lost`/`reconnected` and does not touch
`backend.status`: the REST adapter never sends `Reconnected`, so a synthesized loss would leave `backend.status` on `connecting` for good. Coming back never rejoins.
`fake://?fail=` gains `join_full`, `gone`, `start`, `heartbeat` to rehearse each of these.

**Names and limits (decision 23).** `CreateLobby(name:)` takes one of the names the project **declares** in `backend.ron` (`lobby_names: ["Demo lobby", ...]`, an additive field with a
serde default, `schema_version` stays 1), 1-32 printable ASCII characters each; the generated `ironhold_games.lua` carries the list and the server refuses any other name
(a key holder could otherwise send arbitrary text). One lobby per user, at most 50 per game (soft under concurrent creates), `max_players` 2..8.

**Reserved prefixes.** One `RESERVED_VARIABLE_PREFIXES` const in `schema/backend.rs` (`backend.`, `leaderboard.`, `lobby.`; `matchmaker.` for v3b), used by the runtime sign-out
clearing, the `reserved_variable_write` validate check and the later `SaveState` exclusion (today the prefixes are hard-coded in `runtime/backend.rs` and
`ironhold_cli/src/commands/backend_checks.rs`, and `SaveState` has no exclusion yet).

#### v3a-1 — core, `FakeBackend`, validate, demo on `fake://`, docs (no server)

- **Core.** New `Action`s (`ListLobbies`, `CreateLobby`, `JoinLobby(row:)`, `LeaveLobby`, `StartLobby`) and `RequestKind`s (including `LobbyHeartbeat`) with dedupe keys and
  failure mapping; the glue system's heartbeat timer, the diff, the stale-reply drop, the row events, the changed-only variable writes; `is_online_action` learns the lobby
  actions. The match handle (lobby id plus a server-issued token) stays inside the adapter, never in `GameVariables`.
- **`FakeBackend`.** Simulates lobbies including a few other members who join, leave, and one that starts; `?fail=` options as above; deterministic.
- **Validate.** Errors: `row` outside 1..10; `max_players` outside 2..8; a lobby action whose feature (`Lobbies`) is not declared (**new**: no feature-declared check exists
  today, `backend_checks.rs` only checks leaderboards); a `CreateLobby` name that is not in `lobby_names`; a declared name that is empty, over 32 characters or not printable ASCII
  (so no em-dash). `--strict`: `JoinLobby` with no `ListLobbies` anywhere; a Label bound to `lobby.11.*`; a binding on a `lobby.*` event or variable the engine cannot produce (a
  typo such as `lobby.joind` or `lobby.current.nmae`; a closed vocabulary, so cheap); `reserved_variable_write` widened to `lobby.`.
- **Demo.** New project `backend_demo_lobby` (`/new-project`, `fake://`): list (5 rows, a comment on extending to 10, like `leaderboard.ui.ron`), create, join, leave, start, a waiting room
  from `lobby.current.*` and `lobby.member.<n>.name`, rows shown or hidden from the row events, friendly messages from the reason events, "Reconnecting..." from `lobby.connection_trouble`.
  Default UI snippet `lobby.ui.ron` in its `ui/` folder. **Honest wording** (the fake finds simulated members, not people): card and on-screen text say **"Practice lobby with
  pretend players (test backend). No real players, no shared play yet."**; after Start: **"This is where a match would begin. Online play is not built yet."** Remove
  `multiplayer` from the `backend_demo` card's `data-keywords` in `index.html`. The "finds real players" wording is for a later live variant only.
- **Docs.** `docs/20_data_formats.md` (actions, events, reasons, variables, the snapshot and row-event rules, limits, `lobby_names`, `fake://?fail=` options in the `endpoint` row, and the
  `features` row that today says "nothing reads them yet"), `docs/30_runtime_events_and_logic.md` (action list; empty list, full lobby, server down; browse - waiting room - started as states; why
  a refresh loop faster than 5 s is answered from the snapshot), `docs/15_authoring_tools.md` (one row per new validate check), `docs/STATUS.md`, README row, `index.html` card.
- [x] Core: actions, request kinds, heartbeat timer, diff, stale-reply drop, row events, changed-only writes, shared reserved-prefix const, tests
- [x] `FakeBackend` lobbies and `?fail=` options
- [x] Validate checks (errors and `--strict`) and the schema field `lobby_names`, with CLI tests
- [x] `backend_demo_lobby` project, `lobby.ui.ron`, baseline (own commit), card, `backend_demo` keyword fix
- [x] Docs as listed; the RPC request and reply field lists (below) written into this plan before v3a-2 starts

#### v3a-2 — Lua RPCs, contract tests, adapter, staging playtest (needs v3a-1)

**Storage model (decisions 8, 19).** A lobby is **server-owned** storage (system user; no client permission) in collections named from `lobby_collection(game_id)` (one helper in
`ironhold.lua`; no RPC builds a collection name by hand): `{game_id}.lobbies` (the lobby: members, `max_players`, `started`, `host`, a version), `{game_id}.lobby_hb` (one row per
member, written only by that member's heartbeat, so heartbeats never contend with joins) and `{game_id}.lobby_of` (a per-user index, written **together with the lobby in one
multi-object `storage_write`** so both change or neither). The lobby object is rewritten only on create, join, leave, start and an expiry sweep, with a version check; a join that
loses the race for the last slot re-reads, re-checks capacity, retries up to 3 times, then answers `full`. A listing reads the lobbies with `nk.storage_list` and the **system user id
explicitly** (never nil or empty, which would list client-owned objects), drops expired lobbies and members, and deletes an expired lobby only with a versioned delete so a sweep
cannot remove one a heartbeat just extended. No module-level state (Lua VMs are pooled). The display name comes from `context.username`, never from the payload.

**Storage guard (corrects an earlier plan line).** `ironhold.lua`'s storage before-hook only checks the `{game_id}.` prefix, so today a client could read and write `{game}.lobbies`.
v3a-2 adds an explicit deny of the three lobby collections for every client storage call (read, write, delete, list), with a contract case for each.

**Gating (decision 22).** `ironhold backend config` emits `features = { lobbies = true }` per game (from `backend.ron`'s `features`) and `lobby_names` into `ironhold_games.lua`;
a lobby RPC from a game without it answers `not_enabled`. "Not on a public project" is then a server fact, not only a client habit.

**Adapter and trait contract (from the v3a-1 reviews).** (1) On `SignOut` the adapter sends `LeaveLobby` first and the logout only after the leave has completed: today `NakamaBackend` sends every request as its own concurrent fetch and `cancel_all` drops chains in flight, so the leave and the logout race and the leave often loses (a joinable ghost lobby for up to 60 s). Keep an in-flight `LeaveLobby` out of `cancel_all` and send the logout from its completion; the `Backend` trait doc already states the rule. (2) Replies carry the lobby's `version`; the Lua module increments it on every write of the lobby object. (3) `ironhold_lobby_start` takes the lobby id and answers `not_in_lobby` when the caller's lobby differs. (4) A `LeaveLobby` request means "leave whatever I am in" (idempotent), also with no lobby id. (5) A 429 maps to `rate_limited`, and a Cloudflare block that looks like a network error on the web maps to `server`.

**RPC shapes.** Six RPCs registered through `game_rpc`, ids from a table (the id pattern in `game_rpc` is only a self-check): `ironhold_lobby_create`, `_join`, `_list`, `_leave`,
`_heartbeat`, `_start`. Requests are small JSON objects decoded by one `decode_payload(payload, max_bytes)` that answers `{ok=false, reason="server"}` for malformed or oversized input;
a REST RPC body is a JSON-encoded string (or use `?unwrap`). Replies: `{ok=true, lobby={id, gen, name, players, max, started, host, members={{name=...}, ...}}}` or `{ok=false, reason=...}`.
The exact request and reply field lists are written into this section before the first Lua line.

- **Contract tests** (`tests/contract/run.sh`, adversarial, one per cross-game call): another game's list/join/heartbeat/leave/start; a name outside the declared list; a non-ASCII or oversized name;
  a join of a full lobby; **two simultaneous joins of the last slot** (exactly one wins); a second lobby by one user; the **reload** case (create after losing local state replaces the old
  membership); a started lobby refusing joins; expiry (members and lobby); a direct client read, write and list of each lobby collection; a game without `lobbies`.
- **Adapter** (`ironhold_nakama`): lobby client over the six RPCs, maps the fixed reasons (and 429 to `rate_limited`), keeps the match handle; no heartbeat loop in the adapter (core drives it).
- [ ] Module: `lobby_collection`, `decode_payload`, six RPCs, storage guard, features gate, `lobby_names`; `ironhold backend config` output
- [ ] Contract tests as listed
- [ ] Adapter lobby client
- [ ] Dev playtest against the staging Nakama (native `--backend`), then a Cloudflare-path test with N clients before any public project

### v3b — realtime socket and matchmaker ("transport preview", Queued, depends on v3a)

- [ ] Per-kind request deadline: `FindMatch` uses its own `timeout_secs` and is exempt from the global `request_timeout_secs` (15 s equals Nakama's
      15 s matchmaker interval, so the global timeout would fire before any result); `matchmaker.timed_out` is the engine's answer and calls
      `cancel()`, which sends `MatchmakerRemove`
- [ ] Realtime socket in `ironhold_nakama` behind `client`: native = a dedicated thread with blocking `tungstenite` (reusing the existing ureq/rustls
      stack, no tokio); wasm32 = `web_sys::WebSocket` inside the `NonSend` adapter, closures pushing into the existing channel. **JSON envelope, not
      protobuf** (matchmaker messages are tiny; protobuf adds about 34 KB to the wasm). Opened lazily on `FindMatch`, closed on matched or cancelled;
      a fresh session token first (tokens last 60 s); `ConnectionLost`/`Reconnected` only while a socket is open; **no reconnect-and-resume in v3**:
      Nakama drops the ticket with the session, so a drop answers `matchmaker.failed` (and cancels any search so the state machine can exit)
- [ ] Lua: reopen `MatchmakerAdd` with a guard that **rewrites** the ticket (sets `string_properties.game` to the session's game and prepends
      `+properties.game:{gid}` to the query), and `MatchmakerRemove`; keep `MatchJoin`/`MatchCreate`/`Party*` closed; adversarial contract
      cases (cross-game ticket, forged `game` property)
- [ ] `FindMatch`, `CancelMatch`, `matchmaker.searching/matched/cancelled/timed_out/failed`, `matchmaker.status`; validate: `FindMatch` with `min > max`
- [ ] Demo: extend `backend_demo_lobby` with a Find match button, labelled "transport preview"; ships behind the fake and the contract test only,
      **not on the public demo** until a relay transport exists (players would be matched into nothing). Docs mark it **"matching only, no shared gameplay yet"**

### v4 — stage 2
- [ ] `wss://` from an `https` page, mixed content/CORS, tunnel latency and idle behaviour, half-open drop (the open spike criteria)

## Decisions (answered 2026-10-27)

1. **Lists to the UI:** indexed engine-written variables + `JoinLobby(row:)`; no engine-owned panels. Panels may later be a convenience layer on top.
2. **Lobby vocabulary:** this plan supersedes Form 2's lobby actions/events in `networking_multiplayer.md`.
3. **Cloud save:** waits for `save_load_game_state.md` and reuses `SaveState`; v1/v2 = sign-in + leaderboards.
4. **Crates:** two (contract + `FakeBackend` in core, `ironhold_nakama`).
5. **Relay trait:** deferred until the sync library is chosen.
6. **`game_id` enforcement:** server-side in v2.
7. **Native I/O model:** decided in v2 (task above).

### Decisions (answered 2026-11-05, plan-review round 2; Frank accepted all eight recommendations)

Split: v3a lobbies over REST (Ready), v3b socket and matchmaker ("transport preview"). Details live in the v3a and v3b sections above.

8. **Lobby storage:** server-owned objects behind generic `ironhold_lobby_*` Lua RPCs with expiry and heartbeat; per-user and per-game caps; no direct client access.
9. **Waiting room:** `lobby.current.*`, `lobby.member.<n>.name`, and `lobby.member_joined`/`member_left`/`closed` events.
10. **Start signal:** host-only `StartLobby`, `lobby.started`; the match handle stays in the adapter.
11. **Leaving:** `SignOut` leaves, `LoadScene` does not, a closed tab is covered by expiry; a dropped connection ends the lobby state before `backend.connection_lost`.
12. **Matchmaker timeout:** `FindMatch` has its own deadline, exempt from the global request timeout.
13. **Realtime socket:** native dedicated thread with blocking `tungstenite` (no tokio); wasm32 `web_sys::WebSocket`; opened only during a search; JSON; no reconnect in v3.
14. **Lua module:** reopen only an allowlist of `ironhold_lobby_*` RPCs (v3a) and a ticket-rewriting matchmaker guard (v3b); adversarial contract tests for each.
15. **Demo:** new `backend_demo_lobby` on `fake://` first, labelled "finds and joins players; nobody can play together yet"; `backend_demo_online` is unchanged. (Wording for the fake replaced by decision 26.)

Also adopted from the two reviews: fixed failure reasons in `lobby.last_error` instead of error codes in event names, a visible failure for `JoinLobby` on an
empty or out-of-range row, designer-driven refresh with a documented minimum of about 5 s, and the validate checks listed in v3a.

### Decisions (answered 2026-11-07, plan-review round 3; Frank accepted every recommendation)

Round 3 re-reviewed v3a after the `game_rpc` guard, the shared Cloudflare limit and the first real look at the code. Verdict was "needs more design work"; these decisions close it. v3a is split into v3a-1 (no server) and v3a-2 (server).

16. **Heartbeat owner:** core, on `Time<Real>`, as a normal request with its own dedupe key and a real request id; core diffs the returned lobby into `member_joined`/`member_left`/`started`/`closed`. Request id 0 stays for `ConnectionLost`/`Reconnected` only.
17. **Stale replies:** every lobby reply carries the lobby id and a membership generation; core drops a mismatch; the heartbeat's dedupe key differs from the other lobby actions.
18. **Timing:** heartbeat 10 s with jitter, expiry 60 s, both fixed constants shared by engine and Lua; the engine answers a `ListLobbies` inside 5 s from the last snapshot; native 429 maps to `rate_limited`; a Cloudflare-path test with N clients before any public lobby.
19. **Storage layout:** lobby object (versioned, written only on create/join/leave/start/sweep), one heartbeat row per member, a per-user index written together with the lobby in one multi-object write; retry the last-slot race up to 3 times, then `full`; system user id explicit on every list; versioned deletes; explicit client deny of the three lobby collections (the earlier plan line claiming the module already did this was wrong).
20. **Lifecycle:** a started lobby is not listed and refuses joins; members stop heartbeating after `lobby.started`; `lobby.current.*` survives until leave; `CreateLobby`/`JoinLobby` replace an old membership (page reload); `LeaveLobby` without local state still calls the server.
21. **Failures:** business outcomes are `{ok=false, reason}` (HTTP 200), errors only for auth and bugs; the engine fires `lobby.<op>_failed` then `lobby.<op>_failed:<reason>` (and `lobby.closed` then `lobby.closed:<reason>`); `lobby.last_error` stays as a detail line; the reason list above is the fixed set; `busy` is `lobby.<op>_failed:busy`.
22. **Server gating:** `ironhold_games.lua` carries `features = { lobbies = true }` per game and the lobby RPCs answer `not_enabled` without it.
23. **Lobby names:** a declared list `lobby_names` in `backend.ron` (additive, `schema_version` stays 1), checked by validate and by the server.
24. **Rows and empty state:** per-row events `lobby.row_filled:<n>`/`lobby.row_empty:<n>` (1..10), `lobby.list_empty`, and `lobby.member_row_filled:<n>`/`lobby.member_row_empty:<n>` (1..8); the general hide-when-empty UI feature stays a separate backlog item.
25. **Connection:** `lobby.connection_trouble` on the first failed heartbeat, `lobby.closed:server` after the expiry window; v3a does not synthesize `backend.connection_lost`/`reconnected` and does not touch `backend.status`; `fake://?fail=heartbeat` rehearses it.
26. **Demo wording on `fake://`:** "Practice lobby with pretend players (test backend). No real players, no shared play yet."; after Start "This is where a match would begin. Online play is not built yet."; `multiplayer` leaves the `backend_demo` card keywords. (Replaces the wording of decision 15 for the fake.)
27. **Phase split:** v3a-1 (core, `FakeBackend`, validate, demo, docs) then v3a-2 (Lua RPCs, contract tests, adapter, staging playtest).

Follow-ups from the v3a-1 review cycle (2026-11-07; the five reviewers' findings were all taken, none contradicts decisions 16-27):

28. **`LobbyInfo.version` and `StartLobby { lobby_id }` are part of the contract now.** The server's lobby object carries a version that grows with every write; core drops a lobby answer older than the one it already has, and a lobby never goes from started back to not started (a slow heartbeat answer must not undo a start). `StartLobby` carries the lobby id core believes it is in; the server refuses with `not_in_lobby` when its own view differs (for example after a reload replaced the membership).
29. **A heartbeat gets at most one heartbeat interval (10 s) to answer**, whatever `request_timeout_secs` says, so the trouble signal never arrives after the server has already expired the member. Expiry counts from the last good answer; a started lobby never expires on the client (members stop checking in, so nothing could keep it alive).
30. **Lists follow the session, memberships follow a generation.** A list answer is dropped only when the session ended; leaving or losing a lobby never swallows it. A sign-out forgets the cached list, and a `ListLobbies` while a sign-out is in flight answers `not_signed_in`.
31. **Sign-out leaves the lobby, also while a create or join is still waiting** (the engine does not know yet that it is in one, the server may). The adapter must therefore send `LeaveLobby` BEFORE the logout and must not cancel it (see v3a-2). A session the server ends by itself closes the lobby with `lobby.closed:server`.
32. **Members are told apart by name.** `LobbyInfo.members` are names (Nakama usernames are unique, so the host check, the self-filter and the join/leave diff work by name). If names ever stop being unique, `LobbyInfo` needs member ids first. An answer that no longer lists the player means the server removed them: `lobby.closed:removed`.
33. **The `lobby.` prefix is the engine's** (events and variables); `ironhold validate --strict` treats any other name under it as a typo. The fake gives a joiner a visible start (Ada's lobby starts on check-in 4) and a host who walks away (Grace's, check-in 6), and create/join replace an old membership the way the server will.

### Form 2 coverage check (done 2026-10-27)

| Form 2 item (`networking_multiplayer.md`) | Where it lives now |
|---|---|
| `CreateLobby(name:, max_players:)`, `JoinLobby`, `ListLobbies` | This plan, v3 (`JoinLobby(row:)` instead of `lobby_id:`) |
| `multiplayer.lobby_*` events with JSON payload | `lobby.*` events plus `lobby.<n>.*` variables |
| `LobbyList` UI component | Not built; Labels and Buttons bound to the variables (decision 1) |
| Lobby metadata held by the relay vs the host | Resolved here: lobby objects live in the meta backend (Nakama), not the relay |
| `max_players` enforcement | Resolved 2026-11-05: enforced on the server inside `ironhold_lobby_join` (v3a); a full lobby answers `lobby.join_failed` with reason `full`; host simulation only checks |
| Relay choice (Matchbox vs managed), NAT handshake, direct/relay fallback, internet latency, rollback | Stays in `networking_multiplayer.md` (relay seam deferred) |

Smaller defaults taken (say if you disagree): fake selected by `endpoint: "fake://"`; Lua server module; `game_id` defaults to `project_id`;
last-write-wins plus `cloud.conflict:{slot}`; sign-in is an explicit `SignIn` action, online actions before sign-in finishes answer
`backend.busy`/`backend.offline` instead of queueing; online replies survive `LoadScene` and arrive while paused.

## Acceptance criteria

- Given a project without `backend.ron`, when it starts, then no backend code runs and online actions emit `backend.offline`.
- Given `FakeBackend`, when a state machine fires `SubmitScore` then `FetchLeaderboard`, then `score.submitted:{leaderboard}` and
  `leaderboard.listed:{leaderboard}` arrive and the `leaderboard.<name>.<n>.*` variables hold the rows.
- A designer can show a top-5 leaderboard and (v3) a lobby list using only Labels and Buttons from RON.
- A failed submit or save emits that operation's failure event (`score.submit_failed:{leaderboard}`, `cloud.save_failed:{slot}`), never one generic error.
- `ironhold validate` rejects an undeclared leaderboard, an action whose feature is not declared, and an invalid or duplicate `game_id`.
- Given two projects with different `game_id` on one Nakama, neither sees the other's data or tickets; **a client authenticated for game A that
  sends game B's ids is rejected by the server**.
- Given the server is stopped, start-up is not delayed and gameplay works offline; given a dropped connection, the adapter refreshes the
  session and reconnects without a designer-authored action.
- `ironhold_core` gains no network, async-runtime or platform-specific dependency; backend responses never alter replicated state on one peer only.
