# Feature: Bind the device id to the game

_Status: Draft (not plan-reviewed yet)_
_Planned at: `81c71d6` (2026-11-07)_

## What

Nothing changes for the designer: no RON field, no action, no event. What changes is the credential the engine sends to Nakama at sign-in.
Today it is `{game_id}:{uuid}` where `uuid` is one random value per browser profile (web: `localStorage` key `ironhold.device_id`) or per machine
(native: `~/.ironhold/device_id`), shared by every game on that origin or machine. After this feature the credential is derived from the game and the
endpoint, so one game's id is worthless on another game, and a leaked id from one game cannot be replayed on the others.

## Why

- **The gap.** The server decides which game a session belongs to from the id's prefix alone (`before_authenticate_device` in
  `crates/ironhold_nakama/server/ironhold.lua`). The rest of the id is the same `uuid` for every game. Anyone who learns it can send
  `other_game:{uuid}` and get that player's account in the other game. Found by the `public_backend_demo` review (2026-11-03) and logged in the
  multi-tenant investigation.
- **Why now.** Lobbies (v3a-2) raise what a stolen id is worth: with a leaderboard an attacker can post scores; with lobbies they can also join, host and
  hold slots as the player, through the public Cloudflare path. Cloud save raises it again.
- **Why cheap now.** No cloud save exists, and the public demo holds only leaderboard scores, so changing the id format orphans very little. Every later
  phase makes the migration more expensive.
- **Privacy side effect.** Today the same `uuid` appears in every game's account, so a server operator can link a player across games. A derived id breaks that link.

## What this does NOT fix (say it in the docs)

A script running on the same origin can read `localStorage`, so it can read the stored value and derive any game's id from it. No engine-side change
fixes that: it needs separate origins per game (a deployment choice, not an engine feature). This feature closes two narrower holes:

1. **A leaked wire id.** Ids reach Nakama's database and logs, and a hosting operator of one game on a shared server sees them. Today that id can be replayed as another game by
   swapping the prefix. After: the wire id is a one-way hash, so it is useless for any other game.
2. **A redirected endpoint.** The stored value must never be enough to sign in on a server the attacker runs and then reuse on the real one. The endpoint
   is part of the hash input, so an id made for server A is not the id for server B.

## Approach

**Derivation (client, in `ironhold_nakama/src/ns.rs`, behind `GameNs`):**

```
wire_id = "{game_id}:" + hex( sha256( "ironhold-device-v1" 0x00 game_id 0x00 endpoint 0x00 uuid ) )[..32]
```

- `endpoint` is the normalized base URL (scheme, host, port, no path, lower case); the same string the client already sends requests to.
- 32 hex characters keep the id at 32 + 1 + game_id (at most 32), well inside Nakama's 10-128 byte limit.
- The prefix stays readable because the Lua hook and the guard (`context.vars.game_id`) rely on it.
- `GameNs::device_id(&str)` keeps its signature and stays the only way to build a wire id, so the compile-time "cannot forget the prefix" property carries over.
- The raw `uuid` is never sent and never appears in a log line. The existing `uuid` validation (`[A-Za-z0-9-]`) stays for the stored value.
- `sha2` is a small pure-Rust crate and builds for wasm32. wasm-perf-reviewer to confirm the size cost (expected: a few KB). Do not add a second hash
  crate; if one is already in the tree, use that.

**Storage:** unchanged (one `uuid` per profile). Per-game storage keys would not add protection against same-origin scripts (they can read every key), so
they are not worth the extra state. The derived wire id already gives each game its own account.

**Server (`ironhold.lua`, second step, after the migration window):** `before_authenticate_device` additionally requires the part after `:` to be exactly
32 lowercase hex characters, which refuses every legacy-format id. This is a format check only; the server cannot verify the hash because it never sees the inputs. Its value is that
an old leaked `{game_id}:{uuid}` can no longer be replayed once the cutover is done.

**Migration of existing accounts.** The wire id changes, so each existing player gets a new, empty account in each game they played, and loses the old
leaderboard row. Nakama's `LinkDevice` is denied by design (a second device id could join two games) and there is no rename. Options, for Frank to decide:

| Option | What happens | Cost |
|---|---|---|
| **A. Clean break** (recommended) | New ids from the next release; old accounts and their scores stay on the server, unused. Server format check goes in the same release. | Public demo players restart their leaderboard row. No code. |
| B. Grace period | The client signs in with the new id; for N weeks the server also accepts legacy ids. The client never sends the legacy id again, so this protects nobody and only delays the cutover. | Pointless without an account-link mechanism. Not recommended. |
| C. Server-side relink | A new RPC maps an old id to the new one after proof of the old uuid. | An authenticated pre-session RPC and a new attack surface. Not worth it for leaderboard scores. |

**Order of work:** A is cheapest only if it lands before cloud save and before any lobby is enabled on a public project, which is exactly the
backlog's stated gate.

## Tasks

- [ ] Plan-review (system-architect for the derivation and the server check, wasm-perf-reviewer for the dependency); Frank decides the migration option
- [ ] `ns.rs`: `device_id` derives the hashed id from game_id, endpoint and uuid; `GameNs` gets the endpoint (or the call site passes it); unit tests: different game, different endpoint or different uuid gives a different id; same inputs give the same id; length and character set; the raw uuid never appears in the output
- [ ] `client.rs`: pass the normalized endpoint into the derivation; check that no error message or log line contains the uuid or the wire id
- [ ] Runners (`ironhold_web`, `ironhold_native`): no storage change; update the comments that describe what is stored and sent
- [ ] `ironhold.lua`: the 32-hex format check in `before_authenticate_device` (after the release that ships the client change); contract test: a legacy-format id is refused with "unknown game" or a dedicated message, a derived id is accepted; the existing `game_id`-from-prefix tests keep passing
- [ ] Contract test with two games on one server: the same `uuid` gives two different accounts, and an id derived for game A with the prefix swapped to game B signs in as a different, empty account (no cross-game data visible)
- [ ] Native and web agree: one golden-vector test with a fixed `uuid`, game and endpoint, run in both the native and the wasm test targets, so the two runners can never drift apart
- [ ] Docs: `docs/20_data_formats.md` backend section (what is stored, what is sent, the "same-origin script" limit in one honest sentence), `docs/STATUS.md`, `ironhold_nakama` CLAUDE.md
- [ ] Remove "Bind the device id to the game" as the gate on lobbies in `backend_demo_online`; record the staging-first rollout

## Open questions

- Migration option A, B or C (recommended: A).
- Does `ironhold_games.lua` ever need to run on a second endpoint for the same game (staging and public)? The endpoint is in the hash, so staging and public accounts are
  separate by construction. Confirm that is wanted (it probably is: a staging account should not exist on the public server).
- Is a localhost or LAN endpoint spelled the same way every time (`localhost` versus `127.0.0.1`)? Decide whether the normalization maps them, or whether they are different accounts on purpose.

## Acceptance criteria

- Given two games on one server and one browser profile, when the player signs in to both, then the server sees two unrelated account ids.
- Given an id captured from game A's traffic, when it is sent with game B's prefix, then it opens no account that holds game A data and the server logs nothing about the raw uuid.
- Given the same game and endpoint, when the player reloads the page, then the same account is used (the id is stable).
- Given a legacy `{game_id}:{uuid}` id after the cutover release, when it signs in, then the server refuses it.
- Given the native and the web runner with the same stored uuid, game and endpoint, then both send the identical wire id.
