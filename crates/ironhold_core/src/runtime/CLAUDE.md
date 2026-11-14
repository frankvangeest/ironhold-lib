# runtime/ — boot and update systems outside `scene_manager/`

Rules for `input.rs`, `actions.rs`, `messages.rs`, `model_spawner.rs`, `material_factory.rs` and `mod.rs`. Loaded
when you touch a file in this folder (`scene_manager/` has its own `CLAUDE.md`); the crate-wide rules are in
`../CLAUDE.md`.

## Topic references

- Gamepad binding, claiming and hot join, in full: `docs/dev/gamepad-routing.md` (the two rules in this file are the
  part that must never be violated).

<!-- b:1530.rule -->
## Gamepad binding (`input.rs`)

`InputMap.gamepad_index` is only a **one-time seed**. The source of truth every gamepad consumer reads is
`BoundGamepad(pub Option<Entity>)` (`capabilities/player.rs`): `None` is "pending", `Some(entity)` is locked to that
gamepad entity for the player's lifetime. Gamepad and keyboard are read additively (`||`), never exclusively.

- **`gamepad_bind_system` (`FixedUpdate`, `.before(input_translator_system)`) must never bind a player to an `Entity`
  another player already holds, even across frames.** It keeps a `claimed: HashSet<Entity>` seeded from every
  already-bound player **and from every undrained `is_hot_join` spawn's captured `bound_gamepad`** (a hot-joined
  player can wait in `PendingEntitySpawns` for a frame or more), grown as the pass binds new ones. Without the
  `claimed` check a pad that connects later with a lower `Entity::index()` shifts the sorted list and a pending
  player's seed resolves to a pad another player already owns.
- Pending players are visited in **ascending `PlayerIndex` order**, so which player wins a duplicated seed is
  deterministic, not an accident of query order. A displaced player stays pending (no auto-rebind this session) and
  gets one `warn!` after `GAMEPAD_DIAGNOSTIC_WARN_SECS` (3.0).
- `unclaimed_gamepad_trigger_system` (`Update`) must also reserve the pad a still-pending live player's seed is about
  to resolve to: `FixedUpdate` can tick zero times in a frame, so on the frame a pad first appears
  `gamepad_bind_system` may not have run and the pad would look unclaimed for one frame.

Consumers of the pad (`input_translator_system` here, and the gamepad readers in `capabilities/`) take `Option<&BoundGamepad>`,
**never a required `&BoundGamepad`** (a required one silently drops any test-built player missing it from the whole query tuple,
not just gamepad logic), and look the pad up with `bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok())`.

<!-- b:1606.rule -->
## Gamepad-triggered hot join (`input.rs`)

`unclaimed_gamepad_trigger_system` (`EmitSet::Ui`, before the interpreters) checks the global unclaimed-gamepad bindings
only against pads **not** claimed by a live player's `BoundGamepad`, by an undrained `is_hot_join` entry's captured
`bound_gamepad`, or by a pending player's seed resolving this frame. On a `just_pressed` match it emits the usual
`UiEvent::ButtonPressed` and writes the pad's `Entity` into `PendingJoinGamepad(Option<Entity>)`.

- **At most one pad is captured per frame** (lowest `Entity::index()` wins), and the resource is **reset to `None`
  unconditionally at the top of every run**: otherwise a non-join gamepad trigger (a pause button) leaves a stale pad
  identity for a later keyboard-triggered join to inherit.
- `Action::JoinPlayer`'s executor arm `.take()`s `PendingJoinGamepad` and writes it straight into
  `PlayerConfig.bound_gamepad`, with no round trip through `inputs.gamepad_index`: re-resolving an index to an
  `Entity` a frame later is a window in which pad churn binds the wrong device. A keyboard-triggered join sees
  `None` and is unaffected.

## Online backend (`backend.rs`)

- The contract lives here (`Backend` poll trait, `BackendRequest`/`BackendEvent`, `FakeBackend`); adapters live in their own crate and
  are installed by the runner into the `NonSend` `BackendSlot`. No network, async runtime or platform code in core.
- The online `Action`s only push onto `BackendHub`'s outbox (a resource, never a `Message`: a message buffer drops entries while a
  reader is gated or paused). `backend_glue_system` (`GameStreamSet::Backend`) is the one place that submits, polls, sorts the
  batch by `request_id`, times requests out on `Time<Real>` and writes events plus the `backend.*`/`leaderboard.*` variables.
- **Lobbies (v3a-1).** Core drives the heartbeat: `lobby_tick` (in the glue system) queues a `LobbyHeartbeat` every `LOBBY_HEARTBEAT_SECS` on
  `Time<Real>` (a normal request with its own dedupe key; no clock in the `Backend` trait, and `Instant` panics on wasm32), and
  `apply_lobby_state` diffs each reply into `lobby.member_joined`/`member_left`/`started` (the fake gets them for free). Every request
  stores `LobbyTracker::gen`; a lobby reply from an older generation is dropped (the membership ended or the session did), so a late
  heartbeat can never bring `lobby.current.*` back. `drop_lobby` is the only way a membership ends (bumps `gen`, clears `lobby.current.*`
  and `lobby.member.*`). Failure reasons are the fixed `LOBBY_REASONS` (anything else from an adapter becomes `server`) and fire as the
  generic event then the `:<reason>` form. `JoinLobby(row:)` resolves against core's own row snapshot, never the variables. The names of the
  engine's lobby events and variables are a closed vocabulary (`is_lobby_event`/`is_lobby_variable` in `schema/backend.rs`) that
  `ironhold validate --strict` checks designers' bindings against: add a new one there too (a test in `tests/lobby_tests.rs` fails when the
  engine emits a name the vocabulary does not know). Variables are written only when changed (`put`).
- Event names carry only designer-writable ids, never JSON or error text (the interpreter matches names exactly); reasons go to
  `backend.last_error`. Every failure is per operation (`score.submit_failed:{lb}`), never one generic error.
- The runner hands the engine a `BackendFactory` through `AppOptions` (`backend_factory`, plus `backend_endpoint` to override `backend.ron`'s endpoint);
  `choose_backend` calls it once, only for a non-`fake://` endpoint with nothing installed in the slot. The server key belongs to the file's
  own endpoint: `ResolvedBackendConfig::server_key` is `backend.ron`'s `server_key`, and `None` when a runner override replaced the endpoint; the
  runner's key (`?nakama_key=`, env, key file) only fills in (`ironhold_nakama::resolve_server_key`). The runners resolve the key and the device
  secret inside the factory (which runs when `backend.ron` resolves, only for a real endpoint with a key), never at startup, so an offline
  project never creates or stores a device secret. The server only ever sees `{game_id}:` plus a hash of the game and that secret
  (`ironhold_nakama::ns`), and the web runner's `?backend=` filter (`is_fake_endpoint`) is load-bearing for keeping it from a link-chosen server. The Nakama adapter (`crates/ironhold_nakama`)
  is request/response over `ehttp` and channels, with no thread or runtime of its own of ours (ehttp on native spawns one OS thread per request,
  so a request is not free); its pure server-config generator is behind `default-features = false` so `-p ironhold_cli` alone never links
  an HTTP stack (a workspace-wide build unifies the feature on). `Backend::cancel` is how a timed-out request stops its retry chain; an
  adapter must forget a cancelled request and drop its late answer. The Nakama adapter answers `SignOut` with `SignedOut` at once, but a
  lobby create, join or leave still in flight finishes with the old session (kept in its `Teardown`, never in `self.session`): a leave waits
  for an unanswered create or join, the logout goes out last, a `SignIn` is held until then, and every step also ends on `cancel` (the
  teardown's own session renewal has no engine id, so it never holds anything up). A leave names the lobby of the adapter's last answered create or join (`lobby_id`, fixed per request in `leave_ids` so a 401 retry says the same), so the server ignores a late leave after a re-join; a create or join start resets it to unknown (`{}` = leave whatever I am in), because its answer may never arrive.
- Never use Bevy task pools for adapter I/O. `FakeBackend` latency counts polls, never wall-clock time.
