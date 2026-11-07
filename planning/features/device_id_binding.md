# Feature: Bind the device id to the game

_Status: Draft, revised after plan-review round 1 (2026-11-07); Frank chose migration A and dropping the endpoint from the hash (2026-11-07); waits for the re-review_
_Planned at: `81c71d6` (2026-11-07)_

## What

No new RON field, action or event. What changes is the credential the engine sends to Nakama at sign-in. Today it is `{game_id}:{uuid}` where `uuid` is one
random value per browser profile (web: `localStorage` key `ironhold.device_id`) or per machine (native: `~/.ironhold/device_id`), shared by every game on
that origin or machine. After this feature the wire id is a one-way hash of the game and a stored secret, so a wire id seen by one game says nothing
usable about any other game.

## Why

- **The gap.** The server picks the game from the id's prefix alone (`before_authenticate_device` in `crates/ironhold_nakama/server/ironhold.lua`); the rest
  of the id is the same `uuid` for every game. Whoever sees `a:{uuid}` (a shared HAR or devtools capture, a sniffed http staging hop, a future per-game
  admin view or export in the multi-tenant plan) can send `b:{uuid}` and be that player in game b. Found by the `public_backend_demo` review (2026-11-03).
- **Why now.** Lobbies (v3a-2) raise what a stolen id is worth: with a leaderboard an attacker can post scores; with lobbies they can also join, host and hold
  slots as the player, through the public Cloudflare path. Cloud save raises it again.
- **Why cheap now.** No cloud save exists and the public demo holds only leaderboard rows. Every later phase makes the migration more expensive.
- **Privacy side effect.** Today the same `uuid` is in every game's account, so a server operator can link a player across games. A derived id breaks that link.

## What this protects against, and what it does not (state this in the docs)

All hash inputs are known to the client, so there is no secret in the algorithm. The protection is exactly this: **someone who holds one game's wire id,
but not the stored secret, cannot derive or guess the id for another game.** Things that carry the wire id and are covered: a player's HAR/devtools capture,
a sniffer on a plain-http staging path, a future per-game export. Checked in the reviews: the Nakama JWT carries only user id, username and vars;
leaderboard rows carry owner id and username; `GetUsers` is closed; nothing in `client.rs`, `ns.rs` or core logs the id.

Not covered, and the docs must say so in plain words:

1. **A page on the same web address.** All games published under one address (for example everything under `yourname.github.io`, other repositories'
   pages included) share one browser storage. Any page there, or any script you add such as analytics, can read the stored secret and sign in as that
   player in every one of your games. To keep games apart, give each game its own address (a custom domain or subdomain per game).
2. **A leak of the whole shared server** (database dump, backup, Nakama console, Cloudflare). That store holds every game's wire id at once. Per-game
   tenant views must never expose the raw database.

The earlier draft also claimed to stop "a redirected endpoint". That hole is already closed: the web runner refuses any `?backend=` that is not `fake://`
(`crates/ironhold_web/src/lib.rs`, a backend_abstraction v2 decision), and a native `--backend` override needs local access to `~/.ironhold/device_id` anyway.
**That filter is now load-bearing for credential safety**; pin it with a test and say so in its comment.

## Approach

**Hash inputs: game and secret only. The endpoint is NOT an input** (decided by both reviewers independently; it orphaned every account on any hostname change,
split one Nakama reached by LAN address and tunnel into two accounts, and bought nothing because the redirect hole was already closed). Staging and public are
separate databases, so they need no hash input. This also removes the need for an `account_scope` field.

```
secret  = stored random value (see "Rotation" below)
wire_id = "{game_id}:" + hex( blake3::derive_key("ironhold device id v1", game_id 0x00 secret) )[..32]
```

- 32 hex characters (128 bits) keeps the id well inside Nakama's 10-128 byte limit. The readable `{game_id}:` prefix stays: the Lua hook and the guard
  (`context.vars.game_id`) rely on it.
- `GameNs::device_id(&str)` keeps its signature and stays the only way to build a wire id, so "cannot forget the prefix" remains a compile error. The secret's own
  charset and minimum length are validated in `ns.rs` instead of the composed length.
- **blake3, not sha2.** `Cargo.lock` has no `sha2`; `blake3 1.8.x` is already in both the wasm32 and the native tree (via bevy), so adding it as a direct
  dependency of `ironhold_nakama` costs no binary size. (Only if migration option D below is chosen must the server compute the hash, and then sha256 is needed.)
- The golden vector (fixed game and secret -> fixed wire id) in a unit test is the compatibility contract. Comment it: changing it orphans every account.
  One native test is enough because the same pure function runs on both targets; no wasm test target is needed.
- The raw secret is never sent, logged or put in an error message.

**Rotation (part of the clean break).** The raw uuids of existing players stay in Nakama's device table, so anyone with a database dump could derive every
new wire id from them. So the client also **rotates the stored value**: a new storage key (`ironhold.device_secret`; native file `~/.ironhold/device_secret`),
and the old `ironhold.device_id` / `device_id` are deleted on first run. Players get new accounts anyway, so this costs nothing extra.

**Server (`ironhold.lua`).** `before_authenticate_device` additionally requires the part after `:` to be exactly 32 lowercase hex characters. Lua has no `{32}`
quantifier and `%x` accepts A-F, so: `local gid, rest = string.match(id, "^([a-z0-9_]+):(.*)$")`, then `#rest == 32 and not string.find(rest, "[^0-9a-f]")`. Check
the game first, then the format, so an unknown game still answers "unknown game". The format failure has its **own reject text** (for example "this game version is out of
date, reload the page"), because the people who hit it are visitors with an old cached tab or an old native build, and the "unknown game" row in `docs/20`
sends designers to the wrong fix. It reaches the game as `backend.sign_in_failed`; add a `last_error` row to `docs/20`. The format check is only a format
check (the server never sees the inputs); its value is that a leaked legacy `{game_id}:{uuid}` cannot be replayed once the legacy accounts are gone.

**Cutover order (one release, `/release` step).** (1) Pages deploy with the new client; (2) the Lua module with the format check goes to staging and then public on pmve;
(3) the legacy purge or rewrite (below). The server check must go live **after** the Pages deploy, otherwise every current client breaks; a lag of about a day is
harmless. Deploying the module and the data step are homelab-repo operations and need Frank's confirmation on the day.

## Migration of existing accounts

| Option | What happens | Cost |
|---|---|---|
| **A. Clean break + rotation + purge** (recommended) | New ids and a new stored secret from the release. An admin step deletes the legacy accounts (which also removes the raw uuids from the server) and wipes the `best` leaderboard of `backend_demo_online`. Do not rename the board (it would change every `leaderboard.best.*` bind and the docs/30 snippets). | Public demo players restart their row. No relink code. |
| D. Server-side one-shot rewrite | An admin script rewrites each `g:{uuid}` device row to `g:{hash}` between the deploy and the format check. Keeps scores, removes the raw uuids, no client fallback, no new attack surface. Needs a hash the server can compute (sha256), so `sha2` for the client. | Worth it only if the demo's rows matter. A bigger admin step and a new dependency. |
| B. Grace period | Server accepts legacy ids for N weeks. The client never sends the legacy id again, so this protects nobody. | Rejected. |
| C. Dual sign-in plus `LinkDevice` relink | Rejected: during the window, anyone holding a leaked legacy id can link an id *they* chose into the victim's account and keep it after cutover; it also reopens `LinkDevice`, closed so one account never sits in two games. | Rejected. |

Player-facing part of A: the `backend_demo_online` card in `index.html` says "clearing site data starts a new account, so note your name first"; after the cutover
that advice no longer helps. Add one dated line to the card for a few weeks ("Accounts and scores were reset on <date> after a security update"); the in-scene
`note_3` ("the board may be reset") already covers the screen, so no RON change. Pending deletion requests become moot once the legacy accounts are deleted.

## Tasks

- [x] Frank decided: migration A, no endpoint in the hash (2026-11-07); re-review, then set the status to Ready
- [ ] `ns.rs`: `device_id` derives the hashed id with `blake3::derive_key`; unit tests: other game gives another id; other secret gives another id; same inputs give the same id; length 32 hex plus prefix; the raw secret never appears in the output; the golden vector with the "changing this orphans every account" comment; secret charset and minimum length validated on their own. Add `blake3` to `crates/ironhold_nakama/Cargo.toml`
- [ ] Runners (`ironhold_web`, `ironhold_native`): new storage key and file, delete the old ones on first run, update the comments about what is stored and sent. Private window or blocked storage: unchanged (the secret is read or created before any hash; the existing failure path and its message stay true)
- [ ] Test that pins the fake-only `?backend=` filter in `ironhold_web` (it is now load-bearing); comment in the filter says why
- [ ] `ironhold.lua`: the format check with its own reject text; update the header comment (lines 6-7 say `{game_id}:{uuid}`)
- [ ] `tests/contract.rs`: `raw_session(&format!("alpha:{}", unique(..)))` at lines 127, 158, 192, 193, 206 builds ids the new check refuses; add a helper `fresh_device_id(game)` producing `{game}:{32 hex}`. New cases: a legacy-format id is refused with the new message; an unknown game with a legacy format still answers "unknown game"; two games, same secret, give two unrelated accounts. `adapter_flow.rs` mock ignores the body, so assert the id format in the refusal-then-reauth test (re-auth reuses `self.device_id`, so it is covered automatically); `live_lan.rs` needs the new module on staging and gets a new account there
- [ ] Docs: `docs/20_data_formats.md` ~5115 and ~5125 (the line "Each game gets its own account (`{game_id}:{device id}`)" becomes false: rewrite as "what is kept, what is sent"; the same-web-address paragraph in plain words; a step in "Publishing an online game"; a `last_error` row for the out-of-date refusal; one sentence that staging and public show two different random names), `crates/ironhold_core/src/runtime/CLAUDE.md` ("Online backend"), the `NakamaOptions` doc comment (`client.rs`), `backend_abstraction.md` ~145, `docs/STATUS.md`. There is no `crates/ironhold_nakama/CLAUDE.md`
- [ ] `planning/backlog.md` line about the multi-tenant investigation (~218): it offers "the per-game device id binding" as an alternative to per-game origins; correct it, since this feature is not a substitute for per-game addresses
- [ ] `index.html` `backend_demo_online` card: the dated reset line (and optionally "an anonymous account code" instead of "an anonymous device id")
- [ ] Re-cache `tools/bin/ironhold` before `ironhold backend config --out` (the CLI embeds `ironhold.lua` at build time)
- [ ] Release-day homelab operations (Frank confirms each): deploy the module to staging then public, purge the legacy accounts (or run the rewrite under option D), wipe the `best` board

## Decisions (answered 2026-11-07)

1. **Migration A** (clean break, rotation, purge of legacy accounts, wipe of the `best` board). D was not chosen.
2. **The endpoint is not a hash input.**

## Acceptance criteria

- Given two games on one server and one browser profile, when the player signs in to both, then the server sees two unrelated account ids.
- Given a wire id captured from game A's traffic, when it is sent with game B's prefix, then it opens no account holding game A data.
- Given the same game, when the player reloads the page, then the same account is used (the id is stable).
- Given a legacy `{game_id}:{uuid}` id after the cutover, when it signs in, then the server refuses it with the "out of date, reload" message, and that message appears as `backend.sign_in_failed`.
- Given the native and the web runner with the same stored secret and game, then both send the identical wire id (golden vector).
- Given a database dump after the purge, then it contains no raw uuid and no legacy device row.
