# Feature: Backend v3a-2 - lobbies on the Nakama server

_Status: Draft (2026-11-08), decisions answered, plan-review round 1 pending_
_Planned at: `3c788d5` (2026-11-08)_

Second half of v3a in `planning/features/backend_abstraction.md` (decisions 8, 16-33 there stay the rules; this file only adds what v3a-2 needs to build:
the exact RPC shapes, the storage algorithms, the adapter changes, the test list and the gaps found while reading the code on 2026-11-08).
v3a-1 (core, `FakeBackend`, validate, `backend_demo_lobby` on `fake://`, docs) is merged and done.

## What

No new RON field, action or event for designers: everything they can write already exists from v3a-1. What this adds is the real server behind
`ListLobbies`, `CreateLobby`, `JoinLobby`, `LeaveLobby`, `StartLobby` and the heartbeat: six Lua RPCs in `ironhold.lua`, the generated per-game config
(`features`, `lobby_names`), and the lobby client in the Nakama adapter. After it, `backend_demo_lobby` run with a native `--backend` pointing at the
staging Nakama finds real players on other clients instead of pretend ones.

## Why

v3a-1 proved the designer surface against a fake. Nothing shows that the server algorithms (a race for the last slot, expiry, a reload that replaces a
membership, sign-out ordering) hold on a real Nakama until this exists. It is the gate for any multiplayer lobby, and for v3b (realtime) which reuses
the same lobby objects. The device-id binding gate is done (merged 2026-11-08); its release-day cutover (Pages deploy, module deploy, legacy purge) is
still open and is a precondition for any **public** lobby, not for building or staging this.

## Findings from reading the code (2026-11-08)

1. **The staging deploy now hits the device-id check.** `deploy-nakama-modules.sh` `probe()` signs in with a 20-hex id, which the new module refuses, so every
   module deploy rolls back. The homelab probe fix (32 lowercase hex) is therefore a prerequisite of deploying *this* work to staging, not only of release day.
2. **No match token is needed.** The plan says the adapter keeps "the lobby id plus a server-issued token". The session already identifies the member
   (`context.user_id`) and the per-user `lobby_of` index says which lobby that is; a token would only duplicate that. Drop the token: the adapter keeps
   nothing but what core sends (`lobby_id` in the request).
3. **`nk.storage_delete` cannot join a `storage_write` transaction.** "Written together so both change or neither" works for writes only, so a leave
   writes the lobby object and a **tombstone index row** (`{lobby = ""}`) in one `storage_write`; it never deletes the index row.
4. **A heartbeat reply needs two outcomes the plan did not spell out:** the lobby ended because the host left (`lobby.closed:host_left`) versus the player was
   removed (expired, replaced; `lobby.closed:removed`). The server therefore keeps a **closed marker** (below) instead of deleting a lobby the instant its host goes.
5. **`storage_list` with the system user is the only safe list**, and a listing that has to check every member's liveness would read up to 50 lobbies x 8 heartbeat
   rows. The design below lists the heartbeat collection once per game instead.
6. **`SignOut` ordering is a real adapter rewrite** (decision 31): `NakamaBackend::submit` sends every request as its own concurrent fetch and `SignOut` calls
   `cancel_all`, which today drops an in-flight leave. See "Adapter".
7. **REST RPC bodies are JSON-encoded strings** (a JSON string containing JSON) and the reply is `{"payload":"<json string>"}`; `?unwrap` behavior with a user session is not
   certain on 3.41.0. Build and test the string-encoded form; try `?unwrap` only as an optimization in a contract case, never as a dependency.

## Constants (shared by engine and Lua, not tunable)

`HEARTBEAT_SECS = 10`, `EXPIRY_SECS = 60`, `MAX_LOBBIES_PER_GAME = 50`, `MIN_PLAYERS = 2`, `MAX_PLAYERS = 8` (2..8 per lobby), `LIST_LIMIT = 10` rows,
`MAX_NAME = 32`, `PAYLOAD_MAX_BYTES = 512`. The engine side already has them in `schema/backend.rs`; the Lua side defines them once at the top of the lobby
section with a comment naming the Rust const each mirrors, and a contract test asserts the two agree (the test reads `LOBBY_EXPIRY_SECS` and
`LOBBY_HEARTBEAT_SECS` from `ironhold_core`).

## Storage (server-owned, system user, no client permission)

| Collection | Key | Value | Written by |
|---|---|---|---|
| `{game}.lobbies` | `lobby_id` (16 random hex) | `{v, name, max, host, members = {{uid, name}, ...} (join order), started, closed = nil\|"host_left"\|"server", created, ended}` | create, join, leave, start, sweep |
| `{game}.lobby_hb` | `user_id` | `{lobby, at}` (`at` = `nk.time()` seconds, the **server** clock, never the client's) | that member's heartbeat only |
| `{game}.lobby_of` | `user_id` | `{lobby = "<id>" \| ""}` | the same `storage_write` as the lobby object |

Rules: collection names come only from `lobby_collection(game_id, kind)`; the display name is `context.username`; `v` increments on every write of the lobby object and is what
replies call `version` (Nakama's own object version string is used only as the compare-and-swap token). Every `storage_list` passes the system user id
`00000000-0000-0000-0000-000000000000` explicitly. No module-level state (Lua VMs are pooled).

**Closed marker.** When the host leaves or expires, the lobby object is rewritten with `closed = "host_left"` (versioned) instead of deleted; members find out on their next
heartbeat. A lobby is physically deleted (versioned delete) by a sweep once it has been closed or started for more than `EXPIRY_SECS`, or has no live member.

## RPCs (all through `game_rpc`; every handler starts with `lobbies_enabled(game_id)`, else `{ok=false, reason="not_enabled"}`)

Common: payload decoded by `decode_payload(payload, PAYLOAD_MAX_BYTES)`; anything malformed, oversized or of the wrong type gives `{ok=false, reason="server"}` (HTTP 200).
Reply on success: `{ok=true, lobby = {id, version, name, max, started, host, players, members = {"Ada", "Grace"}}}` (`members` are names, host first, includes the caller); the list reply
is `{ok=true, rows = {{id, name, players, max, host}, ...}}`. Failure: `{ok=false, reason=<fixed reason>}`. `error()` is for "no game session" and bugs only.

| RPC | Request | Does | Reasons |
|---|---|---|---|
| `ironhold_lobby_list` | `{}` | list the game's lobbies that are not started, not closed, and whose host's heartbeat is live (full lobbies are listed; the join answers `full`), newest first, at most `LIST_LIMIT`. Lazy-deletes at most 5 dead lobbies per call (versioned) | `not_enabled`, `server` |
| `ironhold_lobby_create` | `{name, max_players}` | `name` must be in the game's declared `lobby_names` and 1-32 printable ASCII; `max_players` 2..8; refuses past `MAX_LOBBIES_PER_GAME` (soft); first removes the caller from any lobby they are in (below); writes lobby + `lobby_of` + own `lobby_hb` | `not_enabled`, `limit_reached`, `server` |
| `ironhold_lobby_join` | `{lobby_id}` | reads the target and checks it first (exists, not closed, not started, not full) **before** leaving the old lobby; then leaves the old one, then CAS-joins; on a lost race re-reads, re-checks, retries up to 3 times, then `full` | `not_enabled`, `gone`, `full`, `started`, `server` |
| `ironhold_lobby_leave` | `{}` or `{lobby_id}` | idempotent "leave whatever I am in" (`lobby_of`); a host leaving marks the lobby `closed="host_left"`; last member out deletes it; always answers `{ok=true}` | `not_enabled`, `server` |
| `ironhold_lobby_heartbeat` | `{lobby_id}` | writes the caller's `lobby_hb`, reads the lobby, **sweeps** members whose heartbeat is older than `EXPIRY_SECS` (versioned rewrite; a dead host closes the lobby), returns the lobby | `host_left` / `removed` (the adapter turns these two into `LobbyClosed`), `not_enabled`, `server` |
| `ironhold_lobby_start` | `{lobby_id}` | caller must be the host (`not_host`), the lobby the caller's own (`not_in_lobby` when `lobby_of` differs), at least `MIN_PLAYERS` members (`too_few_players`), not already started; sets `started=true`, bumps `v` | `not_enabled`, `not_host`, `not_in_lobby`, `too_few_players`, `started`, `server` |

Absent lobby for a join answers `gone`; for a heartbeat or start it answers `removed` / `not_in_lobby` (the caller no longer has one).

**Replace on create/join** is "leave old (one atomic write of the old lobby and the index), then enter new". If the new entry then fails on a race, the player is in no lobby; the next heartbeat
of the old lobby answers `removed`, which core reports as `lobby.closed:removed` - honest and recoverable. The target is checked **before** the old one is left, so this only happens on a lost race.

## Gating, guard and config

- **`ironhold backend config`** emits per game `features = { lobbies = true }` (when `backend.ron` declares the `Lobbies` feature) and `lobby_names = {...}` into `ironhold_games.lua`
  (`server_config.rs`: `GameServerConfig` gains `lobbies: bool` and `lobby_names: &[String]`; names re-validated when generated, they end up inside Lua string literals). A game's absence of
  `lobbies` means every lobby RPC answers `not_enabled`.
- **Storage guard.** The existing storage before-hook only checks the `{game}.` prefix; it gains an explicit deny of `{game}.lobbies`, `{game}.lobby_hb`, `{game}.lobby_of` for read, write, delete and list (clients never touch lobby state; only the RPCs, which run as the server).
- **`game_rpc` allowlist** already accepts `ironhold_lobby_[a-z_]+`; the six ids come from one table.

## Adapter (`ironhold_nakama`)

- `protocol.rs`: one `rpc(base, token, id, json)` builder (string-encoded body) and one `parse_lobby_reply` for `{"payload": "..."}`; maps `{ok=false, reason}` to the fixed reasons (anything unknown becomes `server`), HTTP 429 to `rate_limited`, a transport failure to `server`.
- `client.rs`: replace the "lobbies are not supported yet" arms in `call()` and `handle()` with the six RPCs; heartbeat `host_left`/`removed` replies become `BackendReply::LobbyClosed`, every other heartbeat failure `LobbyHeartbeatFailed`. No heartbeat loop and no clock in the adapter.
- **Sign-out ordering (decision 31).** `SignOut` sends `LeaveLobby` first if a lobby request has ever succeeded or one is in flight (the engine may not know yet), keeps that request out of `cancel_all`, and sends the logout (and the `SignedOut` reply) from its completion, with a short cap (the request timeout) so a dead server does not trap the player in "signing out". Everything else in flight is still cancelled.
- No match token, no new state beyond a `signing_out: Option<u64>` marker.

## Tests

- **Lua is only exercised by the Docker contract test** (nothing runs it natively), so the contract test is the real gate; it runs twice at the end (after the last code change and after the review fixes).
- Contract fixture: `tests/contract/modules/ironhold_games.lua` gains `lobbies = true` and `lobby_names` for `alpha`, none for `beta` (`not_enabled`), and a third game `gamma` for cross-game cases.
- Contract cases (adversarial, one per cross-game call and per rule): another game's lobby id in join/heartbeat/leave/start; a name outside the declared list, a non-ASCII and an over-long name, `max_players` 1 and 9; join of a full lobby; **two simultaneous joins of the last slot (exactly one wins, run in two threads, repeated)**; a second lobby by one user; the **reload** case (create after losing local state replaces the old membership); a started lobby refuses a join; a heartbeat after the host left answers `host_left`; expiry of a member and of a lobby (the test fixture shortens `EXPIRY_SECS` through a fixture-only override, never in the real module: a `ironhold_games.lua` field `lobby_expiry_secs` that the module reads, default 60, documented as test-only); `list` hides started, closed and dead lobbies; a direct client read, write, delete and list of each of the three collections is refused; `not_enabled`; payload fuzz (non-JSON, an array, 100 KB); the Lua constants equal the Rust ones.
- `adapter_flow.rs` (mock server, native): reply mapping for each RPC and reason, 429 to `rate_limited`, the sign-out ordering (the leave is on the wire before the logout, the logout still goes out when the leave fails or times out), and that a re-authentication in the middle of a lobby call retries it once.
- `live_lan.rs` (ignored, staging): create, join from a second adapter, heartbeat diff, start, leave.

## Tasks

- [ ] Homelab (prerequisite, Frank confirms): `deploy-nakama-modules.sh` `probe()` to a 32-lowercase-hex id and re-copy to pmve (the same task as in the device-id release-day list)
- [ ] `ironhold.lua`: lobby section (constants, `lobby_collection`, `decode_payload`, six RPCs, sweep, closed marker), storage guard, `features` gate, `lobby_expiry_secs` test override; header comment updated
- [ ] `server_config.rs` + `ironhold backend config`: `features`, `lobby_names`; generator tests; the contract fixture regenerated from it
- [ ] Contract tests as listed, including the two-thread last-slot race
- [ ] Adapter lobby client + sign-out ordering; `adapter_flow.rs` cases
- [ ] Docs: `docs/20_data_formats.md` (what the server stores, who can see a member's name: other lobby members and anyone listing; timing; the limits; "a lobby is gone after 60 s without its heartbeat"; the server-admin step to enable `lobbies` for a game), `docs/15_authoring_tools.md` if a new validate check appears, `docs/STATUS.md`, `crates/ironhold_core/src/runtime/CLAUDE.md` (adapter line), the `NakamaBackend` doc comment
- [ ] Dev playtest against the staging Nakama (native `--backend`, two clients on the LAN), checklist below
- [ ] Cloudflare-path test with N clients: **not part of this Done**, tracked as its own backlog item before the first public lobby project (decision for Frank below)

## Playtest (staging, two native clients)

Deploy to staging first (homelab module deploy, Frank confirms). Run `backend_demo_lobby` natively with `--backend <staging url>` twice (two windows, two different `HOME`s so each has its own device secret).
1. Both sign in; the list is empty. 2. Window A creates; window B refreshes and sees it with 1/N. 3. B joins; A sees `member_joined` within about 10 s, B sees A as host. 4. A starts; B sees started within about 10 s, Start is refused for B (`not_host`). 5. B leaves; A's member list shrinks. 6. A third window cannot join a full lobby (`full`). 7. Kill A without leaving; after about 60 s B gets `lobby.closed` with `host_left` and the list no longer shows it. 8. Sign out in the middle of a join (slow staging): no ghost lobby in the list.

## Decisions (answered 2026-11-08, Frank agreed with all three recommendations)

1. **The match token is dropped.** The session and the per-user `lobby_of` index identify the member; the adapter keeps nothing but what core sends.
2. **The Cloudflare-path test is its own backlog item**, "enable lobbies on the first public project" (device-id cutover, a `lobbies` flag for that game on the public server, N clients through `play.frankcreates.com`). It is not part of v3a-2's Done.
3. **`lobby_expiry_secs` is a test-only field** in the generated config (default 60, documented as test-only), so the expiry cases take seconds.

## Open risks for the reviewers

- Listing cost (one `lobby_hb` list per call, at most 50 lobbies x 8 rows, `storage_list` pages of 100): fine at demo scale, revisit before many games share a server.
- A host that loses its tab for more than 60 s closes its lobby for everyone (by design, decision 18).
- Names are visible to anyone who lists or joins; they are the server-made random names, but the docs must say so.
- `?unwrap` unverified; string-encoded bodies are the tested path.
