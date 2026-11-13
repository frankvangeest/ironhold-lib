# Feature: Backend v3a-2 - lobbies on the Nakama server

_Status: Done (2026-11-13). Ready 2026-11-08 after plan-review round 3 (UX round 2 Ready, architect round 3 Ready with text fixes, folded in); playtested on the LAN staging server with 5 players._
_Planned at: `3c788d5` (2026-11-08)_

Second half of v3a in `planning/features/backend_abstraction.md` (decisions 8, 16-33 there stay the rules; this file only adds what v3a-2 needs to build:
the exact RPC shapes, the storage algorithms, the adapter changes, the test list and the gaps found while reading the code on 2026-11-08).
v3a-1 (core, `FakeBackend`, validate, `backend_demo_lobby` on `fake://`, docs) is merged and done.

## What

No new RON field, action or event for designers: everything they can write already exists from v3a-1. What this adds is the real server behind
`ListLobbies`, `CreateLobby`, `JoinLobby`, `LeaveLobby`, `StartLobby` and the heartbeat: six Lua RPCs in `ironhold.lua`, the generated per-game config
(`features`, `lobby_names`), the lobby client in the Nakama adapter, and closing the one server route that lets a player choose their own display name.
After it, `backend_demo_lobby` run with a native `--backend` pointing at the staging Nakama finds real players on other clients instead of pretend ones.
**Core needs no change** (replies already match; the adapter builds `Lobby*Failed` replies itself, see Adapter).

## Why

v3a-1 proved the designer surface against a fake. Nothing shows that the server algorithms (a race for the last slot, expiry, a reload that replaces a
membership, sign-out ordering) hold on a real Nakama until this exists. It is the gate for any multiplayer lobby, and for v3b (realtime) which reuses
the same lobby objects. The device-id binding gate is done (merged 2026-11-08); its release-day cutover (Pages deploy, module deploy, legacy purge) is
still open and is a precondition for any **public** lobby, not for building or staging this.

## Findings from reading the code (2026-11-08, corrected after plan-review round 1)

1. **The staging deploy hits the device-id check.** `deploy-nakama-modules.sh` `probe()` signs in with a 20-hex id, which the new module refuses, so every
   module deploy rolls back. The homelab probe fix (32 lowercase hex) is a prerequisite of deploying *this* work to staging, not only of release day.
2. **No match token is needed.** The session identifies the member (`context.user_id`) and the per-user `lobby_of` index says which lobby that is.
3. **`nk.storage_delete` cannot join a `storage_write` transaction** (`nk.multi_update` may support storage deletes in 3.41; unverified, a contract case settles it). The
   design does not depend on it: a leave writes a **tombstone index row** (`{lobby = ""}`) in the same `storage_write` as the lobby object.
4. **A heartbeat reply needs two outcomes:** `host_left` (the lobby ended because the host left) versus `removed` (the player was swept or replaced). The server keeps a **closed marker**.
5. **List cost.** The heartbeat collection grows with every player ever and rows are only deleted on leave/sweep, so it is never listed. A list reads `{game}.lobbies`
   (up to 3 pages of 100, see Paging) and batch-reads only the host heartbeat keys of the lobbies on that page (one `storage_read`, at most 50 ids).
6. **`SignOut` is a real adapter rewrite** (decision 31, see Adapter). Core queues `LeaveLobby` before `SignOut` itself (`backend.rs:463-477`) in the same frame with the same timeout; a
   timed-out `SignOut` surfaces as `Unreachable`; the adapter has no clock (`Instant` panics on wasm32).
7. **REST RPC bodies are JSON-encoded strings** and the reply is `{"payload":"<json string>"}`; `?unwrap` is unverified. Build and test the string-encoded form only.
8. **`RequestKind::failure` always writes reason `"server"`** (`backend.rs:91-98`): the adapter must build `Lobby*Failed{reason}` replies directly.
9. **Display names are not server-made today.** `UpdateAccount` is not in the closed list (`ironhold.lua`, section 3), so any holder of the shared key can rename their account to any unique text,
   which then shows in lobby lists and the live leaderboards. This feature closes `UpdateAccount` (see Gating).
10. **Constants:** `LOBBY_EXPIRY_SECS`, `LOBBY_HEARTBEAT_SECS`, the row/player/name limits and the list minimum exist in `schema/backend.rs`; `MAX_LOBBIES_PER_GAME` and `PAYLOAD_MAX_BYTES` do not (they are server-only).
11. **Time units:** `nk.time()` is most likely milliseconds. Store and compare `math.floor(nk.time() / 1000)` everywhere; a contract case proves it.

## Constants

Shared with the engine (not tunable): `HEARTBEAT_SECS = 10`, `EXPIRY_SECS = 60`, `MIN_PLAYERS = 2`, `MAX_PLAYERS = 8`, `LIST_LIMIT = 10`, `MAX_NAME = 32` (all in `schema/backend.rs`; the Rust names are `LOBBY_HEARTBEAT_SECS`, `LOBBY_EXPIRY_SECS`, `MAX_LOBBY_ROWS`, `MAX_LOBBY_NAME_LEN`, `MIN_LOBBY_PLAYERS`, `MAX_LOBBY_PLAYERS`, and they are `f64`).
Server-only (Lua module locals, not in Rust): `MAX_LOBBIES_PER_GAME = 50`, `PAYLOAD_MAX_BYTES = 512`.
The Lua side defines them once at the top of the lobby section with a comment naming the Rust const each mirrors. A **Docker-free** unit test in `server_config.rs` reads `MODULE_LUA` and compares the **numeric values** (not the text, the Rust consts are `f64`) of
`HEARTBEAT_SECS` and `EXPIRY_SECS` (and the other shared values) with the Rust consts; the contract test also checks them.

## Storage (server-owned, system user, `permission_read = 0`, `permission_write = 0` on every object)

| Collection | Key | Value | Written by |
|---|---|---|---|
| `{game}.lobbies` | `lobby_id` (`nk.uuid_v4()` without dashes, 32 hex) | `{v, name, max, host, members = {{uid, name}, ...} (join order), started, closed = nil\|"host_left", created, ended}` | create, join, leave, start, sweep |
| `{game}.lobby_hb` | `user_id` | `{lobby, at}` (`at` = server seconds, never the client's clock) | that player's heartbeat, create, join (same write); deleted on leave and in the sweep (versioned) |
| `{game}.lobby_of` | `user_id` | `{lobby = "<id>" \| ""}` | the caller's own row only, in the same `storage_write` as the lobby object; after a leave, a best-effort delete of the caller's **own** row using the version the tombstone write returned, only while it is still `{lobby = ""}`; a conflict means a newer row exists, keep it, **never retry** (a missing row already means "in no lobby") |

Rules:
- Collection names come only from `lobby_collection(game_id, kind)`. These names share a namespace with future cloud-save slots (`{game}.{slot}`): **the names `lobbies`, `lobby_hb`, `lobby_of` are reserved** (a future slot validator rejects them).
- The display name is `context.username` (server-made, now that `UpdateAccount` is closed).
- `v` increments on every write of the lobby object; replies call it `version` (also prevents ABA). Nakama's own object version string is only the compare-and-swap token.
- **Every write is versioned**: a new lobby is written with version `"*"` (must not exist); every other write passes the version it read. `lobby_of` writes use `"*"` when the row was absent.
- **The sweep never writes another user's `lobby_of` row.** Readers treat a `lobby_of` that points at a lobby the user is not a member of, or that is closed, started or missing, as "in no lobby". "Leave old" clears only the **caller's own** row.
- **Liveness** of a member = `lobby_hb.lobby == this lobby id` **and** `now - at <= EXPIRY_SECS`. Every check compares both.
- **A `lobby_hb` row is deleted only if its `lobby` equals the lobby being swept, left or deleted, using the version read.** Otherwise a player who finished started lobby X and created Y would lose the row that now points at Y when X's retention delete runs, and Y would close with `host_left`.
- **Paging.** The list and the live-lobby count follow the `storage_list` cursor up to a hard bound of 3 pages (300 objects); dead lobbies (started or closed, kept 60 s; abandoned ones cleaned at most 5 per call) plus 50 live ones can exceed one page of 100. If the bound is hit, create first runs its lazy cleanup (up to 5 dead lobbies) and only then answers `limit_reached` (fail closed), so a flooded game heals; list returns the live lobbies from the pages it scanned (`storage_list` pages by key, a random id, so "newest first" only orders that partial set).
- Every `storage_list` passes the system user id `00000000-0000-0000-0000-000000000000`. No module-level state (Lua VMs are pooled).
- **Every `storage_write` / versioned delete is wrapped in `pcall` inside the RPC** and a failure means "re-read and retry", at most 3 times. **Exception: the cleanup delete of the caller's own `lobby_of` row never retries** (a retry would re-read a concurrent join's `{lobby = Y}` and delete it, leaving the player in Y with no index row). The sweep inside a heartbeat swallows its own conflict and still answers.
  If the third retry fails while the lobby is not full, the answer is `server`, never `full`. (`game_rpc` turns any non-table error into the generic "the request failed", so an unwrapped conflict would hide the cause.)

**Closed marker.** When the host leaves or is swept, the lobby is rewritten with `closed = "host_left"` (versioned) instead of deleted; members learn on their next heartbeat. A lobby is physically deleted
(versioned) by a sweep once it has been closed or started for more than `EXPIRY_SECS`, or has no live member; its members' `lobby_hb` rows are deleted with it (retention, below).

## RPCs (all through `game_rpc`; every handler starts with `lobbies_enabled(game_id)`, else `{ok=false, reason="not_enabled"}`)

Common: payload decoded by `decode_payload(payload, PAYLOAD_MAX_BYTES)`; malformed, oversized or wrong-typed input (a non-string `lobby_id`, a `lobby_id` that is not 32 lowercase hex, a non-integer `max_players`)
gives `{ok=false, reason="server"}` (HTTP 200). A refused name is also `server`, and the Lua logs the real cause (name, game) so an operator can diagnose it.
Success: `{ok=true, lobby = {id, version, name, max, started, host, players, members = {"Ada", "Grace"}}}` (`members` are names, host first, includes the caller); list: `{ok=true, rows = {{id, name, players, max, host}, ...}}`.
Failure: `{ok=false, reason=<fixed reason>}`. `error()` is for "no game session" and bugs only. HTTP 429 (it comes from the Cloudflare rate limit in front of the server; Nakama has no per-IP limiter) is `rate_limited` for any call, except on a heartbeat, where it is `LobbyHeartbeatFailed{rate_limited}` (core shows every failed heartbeat as `lobby.connection_trouble`).

| RPC | Request | Does | Reasons |
|---|---|---|---|
| `ironhold_lobby_list` | `{}` | list one page of `{game}.lobbies`; batch-read the hosts' `lobby_hb`; keep lobbies that are not started, not closed and whose **host** is live (full lobbies are listed, the join answers `full`), newest first, at most `LIST_LIMIT`. Lazy-deletes at most 5 dead lobbies per call (versioned) | `not_enabled`, `server` |
| `ironhold_lobby_create` | `{name, max_players}` | name in the game's declared `lobby_names` and 1-32 printable ASCII; `max_players` integer 2..8; counts **live** lobbies only (same lazy cleanup as list) and refuses at `MAX_LOBBIES_PER_GAME`; replaces any membership of the caller (below); one write of new lobby (`"*"`) + caller's `lobby_of` + caller's `lobby_hb` | `not_enabled`, `limit_reached`, `server` |
| `ironhold_lobby_join` | `{lobby_id}` | read and check the target first (exists, not closed, not started, not full). **Already a member: return the lobby unchanged** (a reload then join of the same lobby, including the host, changes nothing in the lobby object), but **still write the caller's `lobby_hb`** (versioned), so a reload of about 50 s does not get the caller swept before core's first beat. Otherwise replace as one write (below); on a lost race re-read, re-check, retry up to 3 times | `not_enabled`, `gone`, `full`, `started`, `server` |
| `ironhold_lobby_leave` | `{}` or `{lobby_id}` | idempotent "leave whatever I am in" via `lobby_of`; a host leaving closes the lobby (`host_left`); a closed lobby is deleted by a later list or create once it has been over for `EXPIRY_SECS`; for a started, closed or missing lobby only the caller's own index row is cleared; deletes the caller's `lobby_hb`; always `{ok=true}` | `not_enabled`, `server` |
| `ironhold_lobby_heartbeat` | `{lobby_id}` | requires `lobby_of == lobby_id` **and** membership; writes the caller's `lobby_hb` (never adds a membership); sweeps members whose liveness failed (versioned rewrite; a swept host closes the lobby); returns the lobby. A swept caller gets `removed` | `host_left` / `removed` (the adapter turns these two into `LobbyClosed`), `not_enabled`, `server` |
| `ironhold_lobby_start` | `{lobby_id}` | caller must be the host (`not_host`) and a member (`not_in_lobby`), at least `MIN_PLAYERS` members (`too_few_players`), not already started; sets `started=true`, bumps `v` | `not_enabled`, `not_host`, `not_in_lobby`, `too_few_players`, `started`, `server` |

`limit_reached` comes only from create. A missing lobby answers `gone` for a join, `removed` for a heartbeat and `not_in_lobby` for a start.

**Replace on create/join** is **one `storage_write` of three objects**, all version-checked: the old lobby minus the caller (or closed, if the caller was host), the new lobby plus the caller, and the caller's `lobby_of`
(plus the caller's `lobby_hb` for the new lobby). A lost race therefore leaves the player in the **old** lobby, never in none. The target is checked before the write. This only happens after a reload: while core holds a lobby it
refuses create/join with `already_in_lobby` itself, so a replaced old lobby's heartbeat never runs again; the other members of the old lobby learn from their own next heartbeat (`host_left` or a shorter member list). Lock ordering
of a multi-object write is unverified: the race test includes a two-thread cross-join (A moves X to Y while B moves Y to X) and must finish without a deadlock or a half-write.

## Gating, guard and config

- **`ironhold backend config`** emits per game `features = { lobbies = true }` (when `backend.ron` declares `Lobbies`) and `lobby_names = {...}` into `ironhold_games.lua`
  (`server_config.rs`: `GameServerConfig` gains `lobbies: bool` and `lobby_names: &[String]`). **Names are escaped when written into Lua string literals** (`\` and `"` are legal in a lobby name, `is_valid_lobby_name`):
  use Rust `{:?}`-style escaping (Lua-compatible for printable ASCII) and a generator test with both characters, plus a test that the output parses back to the same names. A game without `lobbies` answers `not_enabled` on every lobby RPC. The game id is written as a bracketed string key (`["3rd_game"] = {`): a bare Lua name cannot start with a digit or be a keyword (`end`).
  **The generator never emits a test-only field** (a generator test asserts it).
- **Storage guard.** The existing storage before-hook only checks the `{game}.` prefix; it gains an explicit deny of `{game}.lobbies`, `{game}.lobby_hb`, `{game}.lobby_of` for read, write, delete and list.
- **The sign-in `username` is dropped.** `AuthenticateDevice?create=true&username=X` sets the username of a new account, and `before_authenticate_device` only rewrites `vars`: the hook sets `payload.username = nil` so Nakama generates the name. Contract case: sign in with `?username=Ada`, get a generated name. Accounts created on staging before this change keep any name they chose; the release-day purge covers them.
- **`UpdateAccount` is closed** in section 3 of `ironhold.lua`, with a contract case. Any other route that can change a display name is checked and closed the same way (contract case per route). Without this, names in lists and the live leaderboards are chosen by the player, and a rename makes the session token's username differ from the account's until refresh, which breaks the by-name host/self checks (decision 32).
- **Test-only expiry override.** The contract test sets a short expiry through a separate module `tests/contract/modules/ironhold_test_overrides.lua` (same precedent as `test_rpcs.lua`), loaded with `pcall(require, ...)`, value clamped to 1..60, and a warning is logged at load when it is in effect. It is never in the real module's config or the generated file.
- **`game_rpc` allowlist** already accepts `ironhold_lobby_[a-z_]+`; the six ids come from one table.

## Adapter (`ironhold_nakama`)

- `protocol.rs`: one `rpc(base, token, id, json)` builder (string-encoded body) and one `parse_lobby_reply` for `{"payload": "..."}`; maps `{ok=false, reason}` to the fixed reasons (anything unknown becomes `server`), HTTP 429 to `rate_limited` (on a heartbeat core turns it into `lobby.connection_trouble`), a transport failure and any other non-2xx answer that is not 401 or 429 (for example the 403 `game_rpc` sends for "the request failed") to `server`.
- `client.rs`: replace the "lobbies are not supported yet" arms in `call()` and `handle()` with the six RPCs. The adapter **builds `Lobby*Failed{reason}` replies directly** (core's generic failure helper would write `server` every time). Heartbeat `host_left`/`removed` become `BackendReply::LobbyClosed`, every other heartbeat failure `LobbyHeartbeatFailed`. No heartbeat loop and no clock in the adapter (core drives it).
- **Sign-out teardown (decision 31; adapter-only, answered 2026-11-08).** Core already queues `LeaveLobby` before `SignOut` when it holds a lobby or a lobby request is in flight (`backend.rs:463-477`), in the same frame, so **the adapter never originates a leave** and the leave **reaches `submit` before `SignOut` does**.
  - **Submit-time rule:** a `LeaveLobby` that arrives while a create/join is in flight is held until that request finishes; one that arrives with none in flight is sent at once. `cancel_all` spares the leave and any create/join, **including requests parked in `fail_waiting` behind an in-flight 401 renewal** (today it would answer the leave and drop its chain, `client.rs:338-350`, and a parked join would be dropped, leaving a ghost member). A renewal that lands after `SignOut` updates the private teardown session, never `self.session`.
  - `SignOut` keeps answering `SignedOut` immediately (as today). The adapter keeps the **old session privately**; teardown is: wait for the in-flight create/join, then the core-issued leave (already sent or held), then the logout. The leave is sent with the old session (one 401 refresh allowed, never overwriting `self.session`).
  - **Every teardown step ends on (its reply arrives) OR (core cancels that request id)**: core's timeout calls `cancel(id)` on the create/join/leave, and web fetch has no timeout of its own, so a wait on the network alone could last minutes. A cancelled step is skipped, not waited for; the ghost risk it leaves is the accepted 60 s expiry.
  - The logout has no core request id, so nothing can cancel it. The held `SignIn` is therefore **released as soon as the logout is sent**, without waiting for its answer (the logout only invalidates the old tokens). Core cancelling the held `SignIn` on its timeout is the cap if an earlier step never ends.
  - Use a small teardown state (idle, waiting for create/join, leaving, logout sent, plus the held sign-in), not a `signing_out` marker. Update the `Backend` trait doc at `backend.rs:206-207` (what `SignOut` answers and when).
- No match token.

## Tests

- **Lua is only exercised by the Docker contract test** (nothing runs it natively), so it is the real gate; it runs twice at the end (after the last code change and after the review fixes).
- Contract fixture: `tests/contract/modules/ironhold_games.lua` gains `lobbies = true` and `lobby_names` for `alpha`, none for `beta` (`not_enabled`), and a third game `gamma` for cross-game cases.
- **Nakama 3.41.0 behaviours nothing has verified, one contract case each:** `nk.time()` units; the error `storage_write` raises on a version conflict; multi-object write lock ordering (two-thread cross-join); `storage_list` signature and page limit; `nk.multi_update` with storage deletes; `?unwrap` (optimization only).
- Contract cases (adversarial, one per cross-game call and per rule): another game's lobby id in join/heartbeat/leave/start; a name outside the declared list, a non-ASCII and an over-long name, a name with `"` and `\`; `max_players` 1, 9 and 2.5; join of a full lobby; **two simultaneous joins of the last slot (exactly one wins, two threads, repeated)**; the two-thread cross-join; joining the lobby you are already in (including the host); a second lobby by one user; the **reload** case (create and join after losing local state replace the old membership, and a lost race leaves the old one); a started lobby refuses a join; heartbeat after the host left answers `host_left`; a swept caller's heartbeat answers `removed`; expiry of a member and of a lobby (via the override module); **negative case: a member who heartbeats inside the window is not removed**; a fresh joiner is not swept before their first heartbeat, nor a reloaded member whose idempotent join refreshed the beat; **start X, create Y, let X's retention delete run, Y must stay listed** (a heartbeat row is deleted only for its own lobby); more than 100 objects (many dead lobbies plus live ones) still enforce the 50 cap and list live lobbies; sign-in with `?username=Ada` gets a generated name; **a leave and a join of Y by the same user run concurrently: Y's `lobby_of` survives**; `list` hides started, closed and dead lobbies and an abandoned lobby no longer counts toward `limit_reached`; a direct client read, write, delete and list of each of the three collections is refused; `UpdateAccount` and every other rename route refused; `not_enabled`; payload fuzz (non-JSON, an array, 100 KB, a non-string `lobby_id`); the Lua constants equal the Rust ones.
- `server_config.rs` unit tests (Docker-free): constants match `MODULE_LUA`; names with `"` and `\` escape and round-trip; the generator never emits a test-only field; the fixture still equals the generator output.
- `adapter_flow.rs` (mock server, native): reply mapping for each RPC and reason, a refused request builds `Lobby*Failed{reason}` with that reason (not `server`), 429 to `rate_limited` (a heartbeat 429 shows as `lobby.connection_trouble`), the teardown (sign-out during an in-flight join: the join finishes, the core-issued leave goes out with the old session, then the logout; the logout still goes out when the leave fails; a new sign-in during teardown is held and proceeds when the logout is sent; **a create that never answers: core cancels it, then the leave and the logout still go out and the held sign-in proceeds**; the leave is submitted before `SignOut` while a join is in flight and is held until the join ends; core cancelling the held sign-in leaves no stuck state; **sign out while a 401 renewal is in flight with a parked join**: the join and the leave still go out and the renewed token goes to the teardown session), and that a re-authentication in the middle of a lobby call retries it once.
- `live_lan.rs` (ignored, staging): create, join from a second adapter, heartbeat diff, start, leave.

## Tasks

- [x] Homelab (prerequisite, Frank confirmed): `deploy-nakama-modules.sh` `probe()` to a 32-lowercase-hex id and re-copy to pmve (done 2026-11-08, homelab commit `5b08a21`)
- [x] `ironhold.lua`: lobby section (constants, `lobby_collection`, `decode_payload`, six RPCs, sweep, closed marker, pcall-and-retry writes), storage guard, `features` gate, close `UpdateAccount` (and any other rename route), optional test-overrides module load; header comment updated
- [x] `server_config.rs` + `ironhold backend config`: `features`, `lobby_names` with escaping; reserved collection names; generator tests and the Docker-free constants test; the contract fixture regenerated from it
- [x] Contract tests as listed, including the two-thread last-slot race and cross-join, and the 3.41 behaviour cases
- [x] Adapter lobby client + teardown state machine; `adapter_flow.rs` cases; the `Backend` trait doc update
- [x] Docs (one pass, `docs/20_data_formats.md` unless named):
  - rewrite the reason-to-operation table (`docs/20` about lines 5191-5207) from the RPC table above: `limit_reached` only from create and **no longer for "one lobby per player"** (line 5203, a second lobby replaces the first after a reload), `rate_limited` from any 429, a 429 on a heartbeat becomes `connection_trouble`; the `not_enabled` row (5202) names both causes ("`Lobbies` is not in `features` (validate reports it), or the server admin has not regenerated and redeployed the config (validate cannot see this)"); the `host_left` row (5207) reads "the host left, lost contact for a minute, or rejoined from a reload"; the `removed` row names the hidden-tab cause;
  - line 5224 ("Creating or joining a lobby takes you out of the one you were in") becomes "only after a reload does a Create/Join replace a membership the game forgot"; line 5125 ("there is no way to choose one yet") becomes "players cannot choose a name";
  - rewrite the `already_in_lobby` text: after a reload Create/Join **replace** the old membership and the other members see `host_left`/`member_left`; `already_in_lobby` is core's own refusal while the engine knows it holds a lobby, so the reason row says "you are in one in this session";
  - a `not_enabled` sentence, and a small table **under Lobbies** (lobby failures never write `backend.last_error`, so the `last_error` table at 5161-5172 is the wrong place) with columns event / likely cause / fix: `lobby.create_failed:server` (a name or game missing from the server's generated config; the admin reads the Nakama log for the refused name) and `lobby.*_failed:not_enabled` (the admin regenerates and restarts);
  - **extend the existing "What is kept, what is sent" paragraph at `docs/20:5125`** (do not add a second paragraph with that heading): names, check-in times and retention ("a lobby record is removed shortly after it ends; a player's check-in row is deleted when they leave or are swept; their index row is deleted when they leave, and otherwise kept until their next lobby action"), who can see a name (other members and anyone listing);
  - timing for publishers: others see a join or start within about 10 s; a Refresh within 5 s may still show old rows; on the web a hidden tab stops checking in and a host away over 60 s closes the lobby; a returning member is told `removed`; the web build cannot reach staging;
  - admin steps: the exact `ironhold backend config <every project the staging server hosts, including backend_demo_online> assets/projects/backend_demo_lobby --out ...` (a game left out gets `unknown game`); adding, renaming or removing a `lobby_names` entry, or adding `Lobbies`, needs the admin step again (admin regenerates and restarts first, then publish the client); extend "Publishing an online game" step 2;
  - replace the callout "Today a lobby is a rehearsal. Only the in-memory `fake://` test backend serves lobbies" and `backend.ron:11` ("A real server needs only another endpoint here") and `STATUS.md:65`; `README.md:135` ("pretend players ..." becomes "on `fake://` by default (pretend players); natively with `--backend` against a lobby-enabled server it finds real players. No shared play yet."); `backend.ron:1-3` (add "run natively with `--backend` to meet real players");
  - re-check `docs/30_runtime_events_and_logic.md` (about lines 458 and 521-533) after the above;
  - `docs/15_authoring_tools.md` only if a new validate check appears; `crates/ironhold_core/src/runtime/CLAUDE.md` (adapter line); the `NakamaBackend` doc comment.
- [x] Demo `backend_demo_lobby` (text boxes hold about 55 characters, box width / (font size x 0.6)): `fake_note` "Lobby demo. On fake:// the players are pretend." and `fake_note_2` "No shared play yet."; keep the retry hint ("Backend unreachable. Press Sign in to retry."); a `lobby.list_failed:not_enabled` rule ("Lobbies are off for this game on the server."); the host-left message "The host left or lost connection, so the lobby closed."; `state_machine.ron:74-75`: the comment **and the rule** change (today it says "Leaving your old lobby. Try again." and fires `LeaveLobby`; `already_in_lobby` is now core's own refusal while it holds a lobby, so make it a plain message "You are already in a lobby. Leave it first." or remove it). Check `ironhold backend config` accepts a project whose `endpoint` is `fake://`.
- [x] Dev playtest against the staging Nakama (native `--backend`; Frank played with 5 players on the LAN, 2026-11-13, all as expected)
- [x] Backlog: item "enable lobbies on the first public project" (Cloudflare path, N clients through `play.frankcreates.com`, device-id cutover, the two limits below). **Not part of this Done.**

## Implementation notes (2026-11-08)

Built on `feature/backend_lobbies_server`. The code review (alignment, architect, debug-detective, UX) found, and the branch fixed: an invalid generated Lua key for a game id starting with a digit or a keyword; the teardown's own renewal holding up the logout (it has no engine id, so it now counts only while a request is parked behind it); a join of a started lobby moving a member's check-in; the create cap counting the caller's own lobby; a reason such as `host_left` leaking into a create/join/start failure; and many docs details. Deferred to the backlog: the adapter's leave carries no `lobby_id` (so the server's stale-leave protection never applies to real clients), and a single account can fill the scan bound with closed lobbies (see "Enable lobbies on the first public project").

## Playtest (staging, native clients)

Prerequisites: homelab `probe()` fix deployed; the staging module deployed with the generated config including `backend_demo_lobby` (Frank confirms every server action). Each window needs its **own device secret** and the server key. On Windows the native runner reads `USERPROFILE` before `HOME` (`crates/ironhold_native/src/nakama.rs:11-12`), and the demo `backend.ron` has no `server_key`, so per window (Git Bash, Windows-style paths because `USERPROFILE` is not converted):

```
USERPROFILE=C:/tmp/lobbyA HOME=C:/tmp/lobbyA NAKAMA_SERVER_KEY=<staging key> cargo run -p ironhold_native --release -- --project backend_demo_lobby --backend http://<staging>:7350
USERPROFILE=C:/tmp/lobbyB HOME=C:/tmp/lobbyB NAKAMA_SERVER_KEY=<staging key> cargo run -p ironhold_native --release -- --project backend_demo_lobby --backend http://<staging>:7350
```

Keep the window of a waiting player in front (the web build is not part of this test). Check 0: the two "Signed in as:" names **differ** (otherwise the secrets were shared; stop).

1. **Start alone.** A creates a lobby and presses Start while alone: `too_few_players`. Then B refreshes (wait 5 s first) and sees it with 1/N, B joins; B sees A's name first and A sees B join within about 10 s.
2. **Leave.** B leaves; A sees "A player left." within about 10 s.
3. **Host lost.** B rejoins; kill A without leaving; after 60-70 s B gets `host_left` and a Refresh no longer lists the lobby. Restart A with the same `USERPROFILE`.
4. **Start with two.** A creates a fresh lobby, B joins, A starts; B sees started within about 10 s. Both press "Back to the lobbies"; the started lobby is no longer in either list (no third client needed).
5. **Reload.** A creates, B joins; close A and reopen it (same `USERPROFILE`), Create: the old lobby is replaced and B gets `host_left` within about 10 s.
6. **Sign out.** B joins A's lobby, then B presses Sign out: A sees "A player left." and B's row disappears within about 10 s. The mid-join race is covered by `adapter_flow.rs`, not by hand (on a LAN a join answers in milliseconds).

`not_host` and `full` are covered by the contract tests, not by hand (the demo hides Start from non-hosts, and `full` needs more clients than the demo's `max_players: 4` allows without five windows).

## Decisions (answered 2026-11-08)

Round 0 (before plan-review):
1. **The match token is dropped.**
2. **The Cloudflare-path test is its own backlog item**, "enable lobbies on the first public project".
3. **`lobby_expiry_secs` is test-only** - revised below: it is not a generated-config field.

Round 1 (after plan-review, Frank agreed with every recommendation):
4. **Sign-out is adapter-only** (answered "agreed with recommended" after the pros and cons): `SignedOut` immediately, private teardown with the old session, a held `SignIn` as the cap, no core change.
5. **Test expiry override** lives in a separate test-only module (`ironhold_test_overrides.lua`), never in the generator or the real config.
6. **`UpdateAccount` is closed in this feature**, with a contract case.
7. **Shared-IP and cap-abuse limits are accepted for staging** and written into the Cloudflare-path backlog item and the docs: players behind one public IP share Cloudflare's 30 requests per 10 s (8 players use about 24 per 10 s before 401 refresh chains), and two IPs running about 50 heartbeating dummy hosts can hold a game's 50-lobby cap with free accounts.
8. **Demo wording** is neutral (true for the fake and the real backend).
9. **A refused name stays `server`**, the Lua logs the cause, a troubleshooting row is added; no new reason in `LOBBY_REASONS`.
10. **Per-player rows are deleted** on leave and in the sweep, and the docs say so. (`lobby_hb` rows are deleted only for their own lobby; the caller's own `lobby_of` row is deleted after the tombstone write.)

## Open risks for the reviewers (round 3)

- A host that loses its tab for more than 60 s closes its lobby for everyone (by design, decision 18).
- Names are visible to anyone who lists or joins; they are server-made now that `UpdateAccount` is closed.
- Multi-object `storage_write` lock ordering and the conflict error shape on 3.41.0 are unverified until the contract cases run.
- Retention deletes a started lobby 60 s after it starts; v3b, which reuses these objects, must revisit that.
- `?unwrap` unverified; string-encoded bodies are the tested path.
- When core times out a leave and its join in the same frame (same-frame submit, equal timeouts), the leave is skipped and the join may still land: the accepted 60 s ghost, no fix.
- The adapter teardown is the most intricate piece: the held `SignIn` can delay a quick re-sign-in by up to its own timeout on a dead server.
