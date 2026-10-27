# Feature: Backend abstraction (core contract + Nakama adapter)

_Status: In Progress (v1 Done, v2-v4 Queued). Plan-review round 1 done 2026-10-25; all seven decisions answered 2026-10-27, see "Decisions (answered)"._
_Planned at: `cd90a12` (2026-10-25)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Contract in core (Actions, events, engine-written variables, poll trait, glue system) + `FakeBackend`; sign-in and leaderboard only; nothing networked | Done | 8745ef9 (2026-10-27) |
| v2 | `ironhold_nakama` adapter, **native and wasm32 on the LAN**: sign-in, leaderboards, server module with `game_id` enforcement; cloud save once local save exists | Queued | — |
| v3 | Lobby list (public-read storage), then matchmaker | Queued | — |
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
| Lobbies (v3) | `ListLobbies`, `CreateLobby(name:, max_players:)`, `JoinLobby(row:)`, `LeaveLobby` | `lobby.listed`, `lobby.created`, `lobby.joined`, `lobby.left`, `lobby.list_failed`, `lobby.join_failed` | `lobby.count`, `lobby.<n>.name`, `lobby.<n>.players` ("2/4"), n = 1..10 |
| Matchmaker (v3) | `FindMatch(min:, max:, timeout_secs:)`, `CancelMatch` | `matchmaker.searching`, `matchmaker.matched`, `matchmaker.cancelled`, `matchmaker.timed_out`, `matchmaker.failed` | `matchmaker.status` |

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

### v3 — lobbies and matchmaker
- [ ] `JoinLobby(row:)` resolves against the snapshot taken when the list was written (rows shift on refresh); also expose `lobby.<n>.id`
- [ ] Default UI snippet `lobby.ui.ron` (rows with a Join button each) in the demo project's `ui/` folder, same rules as v1
- [ ] Public-read lobby objects with `JoinLobby(row:)`; then matchmaker (`FindMatch`, `CancelMatch`, timeout). Docs mark these **"matching only, no shared
      gameplay yet"** until a relay transport exists (designers get a match handle and nothing to play).

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

### Form 2 coverage check (done 2026-10-27)

| Form 2 item (`networking_multiplayer.md`) | Where it lives now |
|---|---|
| `CreateLobby(name:, max_players:)`, `JoinLobby`, `ListLobbies` | This plan, v3 (`JoinLobby(row:)` instead of `lobby_id:`) |
| `multiplayer.lobby_*` events with JSON payload | `lobby.*` events plus `lobby.<n>.*` variables |
| `LobbyList` UI component | Not built; Labels and Buttons bound to the variables (decision 1) |
| Lobby metadata held by the relay vs the host | Resolved here: lobby objects live in the meta backend (Nakama), not the relay |
| `max_players` enforcement | Open: enforce in the lobby object (v3), host simulation only checks |
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
