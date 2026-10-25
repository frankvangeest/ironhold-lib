# Feature: Backend abstraction (`ironhold_backend`) with a Nakama adapter

_Status: Draft_
_Planned at: `cd90a12` (2026-10-25)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Backend contract in core (Actions, events, request/event types) + in-memory fake backend; nothing networked | Queued | — |
| v2 | Nakama adapter, native: sign-in, cloud saves, leaderboards; generated server-module config | Queued | — |
| v3 | Nakama adapter: lobby list (public-read storage), matchmaker, `FindMatch` | Queued | — |
| v4 | Nakama relay as a `NetTransport` (match join, data, presence, reconnect) behind the same trait | Queued | — |
| v5 | Browser (wasm) adapter on `https`/`wss` and the stage 2 verification | Queued | — |

## What

A game author gets online features without writing code: a project's RON declares which backend features it uses
(`backend.ron`), and RON actions such as `SignIn`, `SaveToCloud`, `SubmitScore`, `ListLobbies`, `FindMatch` talk to whatever backend
the engine was built with, answering with pipeline events (`backend.signed_in`, `backend.score_submitted`, `lobby.listed`, ...).
The first and probably only real adapter is Nakama. With no backend configured or reachable, a project still starts and plays; the
online actions answer `backend.unavailable` instead of failing.

## Why

The Nakama spike (`planning/investigations/nakama_backend_viability.md`, section 8c) concluded: **go for Nakama as the meta
backend**, conditional go for its relay for snapshot-style gameplay. Building accounts, storage, leaderboards, lobbies and
matchmaking ourselves is the larger and riskier job; the investigation also said to put them behind one engine-side abstraction so
the adapter (and the relay transport) stay swappable and `ironhold_core` stays platform-agnostic. The spike already measured the
parts that decide the design: the realtime protocol layer is about 660 lines, REST calls cost 8-22 ms, a slow reader is dropped
after about 3 s, the usable message size is about 2.9 KB (JSON) or 4 KB (protobuf).

It unblocks: cloud saves (`planning/features/save_load_game_state.md` is local only today), Form 2 lobbies in
`planning/features/networking_multiplayer.md`, and any shipped game wanting a leaderboard.

## Approach

### Layers (and where code lives)

```
ironhold_core        Actions + events + BackendRequest/BackendEvent messages + a BackendPlugin hook (no sockets, no HTTP, no async runtime)
ironhold_backend     the Backend trait, shared types, the in-memory FakeBackend (used by tests and as the "no network" dev backend)
ironhold_nakama      Nakama adapter: REST + realtime socket (native: tokio-tungstenite, wasm: ewebsock/ehttp), protobuf envelope
ironhold_native/web  runners add the adapter plugin (the only place that knows which backend is built in)
```

- **Core never calls the network.** The action executor turns an online `Action` into a `BackendRequest` message and returns. The
  adapter plugin's systems consume requests, run them on its own task pool/`fetch`, and send `BackendEvent` messages back, which a
  core system turns into pipeline events (`GameEvent`) for the FSM, like any other capability. This keeps the Message → Interpreter
  → Action → Executor pipeline unchanged and deterministic code free of I/O.
- **Backend trait (async-free at the core boundary):** request/response pairs identified by a `request_id`; the trait is what the
  adapter implements and what `FakeBackend` implements, so the whole RON-to-event path is testable without a server.
- **The relay is a separate seam (`NetTransport`),** not part of the meta trait: gameplay sync (Lightyear, replicon, matchbox, ...)
  is chosen elsewhere (`networking_multiplayer.md`) and only needs "send bytes to a member, receive bytes with presence events".
  The Nakama relay implements it in v4; Matchbox can implement it too.

### One Nakama, several games

A single Nakama server has one user table, one server key and one set of config values; nothing separates games by itself. So the
engine namespaces everything by a `game_id` from `backend.ron`:

- storage collections `"{game_id}.saves"`, leaderboard ids `"{game_id}.{name}"`, match labels and matchmaker properties
  (`properties.game:{game_id}` in every query), lobby objects in `"{game_id}.lobbies"`;
- device ids are `"{game_id}:{device_uuid}"` by default, so a device has a separate account per game (one login across games is a
  deliberate opt-in later, not the default);
- one Nakama instance is fine while every game is ours; a game from someone else gets its own Nakama + Postgres (cheap in the homelab).
- The server key is shipped inside any web build; it is a client identifier, not a secret (the spike's key file stays outside the repo
  only because the deployed server is our own).

### Designer-facing RON (sketch, to refine in v1)

```ron
// assets/projects/<name>/backend.ron
BackendConfig(
    game_id: "ironhold_demo",
    endpoint: "https://nakama.example.org",   // optional; absent = offline project
    features: [CloudSave, Leaderboard(name: "best_time", order: Ascending, operator: Best), Lobbies, Matchmaker],
)
// state_machine.ron: actions
SignIn
SaveToCloud(slot: "slot1")
LoadFromCloud(slot: "slot1")
SubmitScore(board: "best_time", score_var: "run_time_ms")
ListLobbies, CreateLobby(name: "my game", max_players: 4), JoinLobby(id: "..."), FindMatch(min: 2, max: 4)
// events: backend.signed_in, backend.unavailable, backend.error:{code}, cloud.saved:{slot}, cloud.loaded:{slot},
//         score.submitted, lobby.listed:{json}, matchmaker.matched:{json}
```

Leaderboards can only be created by **server code** (spike finding), so the engine ships **one generic, data-driven Nakama module**
(Lua or TypeScript, decided in v2) and `ironhold backend config <project>` (a CLI command, validated like the rest of the CLI)
generates its config from `backend.ron`, so designers only edit RON and a deployer copies one file. Leaderboard creation by an
allow-list in that config, never by a client request.

### Constraints carried over from the spike

- Session tokens last 60 s: refresh before every reconnect (401 otherwise); an open socket survives expiry. The adapter owns refresh.
- A slow reader is dropped (queue 16, write timeout): treat a disconnect as normal and reconnect; the relay seam must say so.
- Default realtime envelope **protobuf**: latency identical to JSON, raises the payload ceiling from 2.9 to about 4 KB, costs about
  34 KB of wasm (negligible next to the engine). JSON stays available for debugging.
- Stored JSON is normalized by the server: saves must not depend on byte-exact text.
- Matchmaker result arrives about one server interval (default 15 s) after the ticket; the UI needs a "searching" state.
- The backend is optional: no endpoint, no network, or a failed sign-in must never block start-up or the offline game.

## Tasks

### v1 — contract and fake
- [ ] `backend.ron` schema (`BackendConfig`, `BackendFeature`), validation in `ironhold validate`, docs in `docs/20_data_formats.md`
- [ ] Online `Action` variants and `backend.*`/`cloud.*`/`score.*`/`lobby.*`/`matchmaker.*` events (schema + executor + query output)
- [ ] `ironhold_backend` crate: trait, request/event types, `FakeBackend` (in-memory, deterministic, can inject latency and failures)
- [ ] Core glue system (events back into the pipeline), `backend.unavailable` when no adapter is registered
- [ ] Tests with `FakeBackend`: each action to its event, offline project, error and timeout paths
- [ ] A demo project that uses cloud save and a leaderboard against the fake (via `/new-project`)

### v2 — Nakama adapter, native
- [ ] `ironhold_nakama`: REST client, session refresh, `game_id` namespacing; protocol layer reused from tag `spike/nakama_backend`
- [ ] The generic server module and `ironhold backend config`
- [ ] Contract tests against a real Nakama container (local script, not a hosted CI); pinned server version
- [ ] Sign-in, cloud save/load (optimistic version, conflict event), leaderboard submit/list

### v3 — lobbies and matchmaking
- [ ] Public-read lobby objects, matchmaker add/cancel/matched, token join

### v4 — relay transport
- [ ] `NetTransport` trait; Nakama relay implementation (protobuf), presence events, reconnect/rejoin, slow-reader handling

### v5 — browser and stage 2
- [ ] wasm transports in the adapter, `wss://` from an `https` page, mixed-content and CORS checks, tunnel latency and idle behaviour
      (the open criteria of the spike result)

## Open questions

- Crate layout: separate `ironhold_backend` + `ironhold_nakama`, or one crate with a Nakama feature? Separate is the proposal (the
  trait crate has no network dependencies); confirm with the architecture review.
- Where does the async/IO live in a Bevy app on both targets (tokio threads natively, `spawn_local`/`fetch` in wasm)? Proposal:
  adapter-internal, with channels to Bevy systems, never in `ironhold_core`.
- Account model beyond device ids (email, Steam, linking device accounts) and its privacy duties (GDPR deletion/export) before a
  public deployment.
- Lua or TypeScript for the generic server module (Lua is what the spike used and needs no build step).
- How the engine finds the server key and endpoint for a shipped web build (RON is public; per-environment override for dev).
- Are online state variables (best score, signed-in user) exposed as `GameVariables`, and which?
- Does v4 wait for the sync-library decision in `networking_multiplayer.md`? Proposal: only the seam, no sync code.
- Save conflicts: last-write-wins with a visible event, or a merge hook?

## Acceptance criteria

- Given a project without `backend.ron`, when it starts, then no backend code runs and online actions emit `backend.unavailable`.
- Given the `FakeBackend`, when a state machine fires `SaveToCloud` then `LoadFromCloud`, then `cloud.saved` and `cloud.loaded` arrive and the data round-trips.
- Given two projects with different `game_id` on one Nakama, when both store, list leaderboards and matchmake, then neither sees the other's data or tickets.
- Given the Nakama server is stopped, when the player starts the game, then start-up is not delayed and gameplay works offline.
- Given a dropped connection, when it recovers, then the adapter refreshes the session and rejoins without a designer-authored action.
- `ironhold_core` gains no network, async-runtime or platform-specific dependency (checked in review).
