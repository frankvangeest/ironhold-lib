# Feature: Public backend demo (GitHub Pages talking to the homelab Nakama)

_Status: In Progress (plan-review 2026-10-30, findings folded in; one design change for Frank to veto, see "Plan-review outcome")_
_Planned at: `5683aed` (2026-10-28)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| P1 | Homelab stage 2: public `https://` hostname for Nakama's client API (homelab repo) | Queued | - |
| P2 | Engine: optional `server_key` in `backend.ron` (+ validate, docs, tests) | Queued | - |
| P3 | Online demo project pointed at the public endpoint | Queued | - |
| P4 | `/release` batch, then the live check from the Pages origin | Queued | - |

P1 and P2 are independent and can run in parallel; P3 needs both; P4 needs P3.

## What
The hosted demo gets a second, **online** variant of `backend_demo`: a visitor to the GitHub Pages site can sign in
(anonymous device account), submit a score and see a real leaderboard stored on the Nakama server in the homelab. The
existing `fake://` demo stays as it is (offline, deterministic, used by the browser test suite). Nothing changes for a
designer except one optional `backend.ron` field (`server_key`) so a published page works from a plain link.

## Why
Backend v2 is merged and was playtested against the homelab from localhost, but nobody outside the LAN can run it:
Pages is `https`, the server is a plain `http://` LAN address. This phase proves the whole chain a future hosted game will
use (Pages build -> public TLS hostname -> Nakama module -> Postgres) and surfaces the operational gaps (rate limits, key
handling, rotation, privacy) while the stakes are one demo leaderboard.

## Findings that shape the plan
- **Mixed content / LAN address:** a Pages page cannot call `http://192.168.178.106:7350`. The engine already says so in
  `docs/20_data_formats.md` (hosted builds need an `https://` endpoint).
- **CORS is fine (verified 2026-10-28):** a preflight from `Origin: https://frankvangeest.github.io` with
  `authorization,content-type` returns `200`, `Access-Control-Allow-Origin: *`, `Access-Control-Allow-Headers:
  Authorization,Content-Type`. Re-check once through the tunnel (Cloudflare must not strip it).
- **The web page cannot choose a real endpoint:** `?backend=` accepts only `fake://` by design, so the real URL must be
  committed in a `backend.ron` that ships in the build.
- **Only plain HTTP is needed today.** The adapter is request/response (`ehttp`); no WebSocket is opened until v3, so the
  tunnel only has to carry HTTPS. (v3 will add the idle-socket and 100 s Cloudflare timeout checks.)
- **The server key is public by design** (it ships in every web build) but today only arrives via `?nakama_key=`. The
  v2 follow-up list already marks the `backend.ron` field as required before any public deployment.
- **Our Lua module is already deny-by-default** (only the engine's REST calls are allowed, leaderboard/storage ids are
  prefix-guarded, operator forced to 0), which is what makes exposing the client API acceptable. The console (:7351) is a
  different port/router and must stay unrouted.
- **Nakama has no built-in rate limiting.** Anyone can create accounts and submit scores. Mitigation has to come from
  Cloudflare (WAF rate-limit rule on the hostname) and/or our module.

## Plan-review outcome (2026-10-30)
system-architect: needs revision (all plan-text fixes, folded in below; no WASM or binary-size risk, no new dependency).
ux-gamedesigner-reviewer: P2 ready once the doc items were added, P3 not ready (offline wording, designer path). Folded in.
Changes that matter:
- **Design change for Frank to veto:** the key precedence is now "the file's `server_key` goes with the file's own endpoint"
  instead of "runner key always wins" (see P2). Reason: on web a runner key can never be paired with another server
  (`?backend=` is `fake://` only), so runner-wins only allowed a crafted link
  `play.html?project=backend_demo_online&nakama_key=x` to break sign-in for whoever clicks it, and a stale native
  `NAKAMA_SERVER_KEY` / key file from the LAN playtests would override the correct key after rotation.
- "Server down" is not `backend.offline` (that means "no usable backend configured"): a down server answers
  `backend.unreachable` / `backend.sign_in_failed`, and only after the visitor presses Sign in. Criteria rewritten.
- v2 deliberately kept the key out of `backend.ron` (it is deployment data, not game data). This phase reverses that for
  published games, because a hosted page has no other way to supply it; the reason must be written into the docs.

## Approach

### P1 - Homelab stage 2 (lives in the homelab repo, `planning/features/nakama-deployment.md`)
Not duplicated here; this plan only states what the engine side needs from it:
- hostname `play.frankcreates.com` (Frank, 2026-10-29), valid TLS (Cloudflare Tunnel -> Traefik `letsencrypt`), routing
  **only** the client API to port 7350, endpoint without a path (the adapter only trims a trailing `/`); the console keeps
  `nakama.home`, LAN-only
- the `DOCKER-USER` / ufw-docker bug fixed first, so the console is not LAN-wide exposed while the API is public
- Cloudflare WAF rate-limit rule on `/v2/account/authenticate/*`; the action must be **Block (429)**, not a managed/JS
  challenge (`fetch` cannot solve one); no redirects on `/v2/*` (a CORS preflight does not follow them); no cache rule on `/v2/*`
- **secrets gate:** the homelab vault is live; Nakama's secrets must be vault-managed and the ones printed into the
  2026-10-28 transcript rotated before the hostname goes live (done in the homelab repo, separate session)
- the public server hosts exactly the demo games (`backend_demo_online`, and `backend_demo` since both are regenerated
  into one `ironhold_games.lua`; **note `backend_demo`'s namespace is then publicly writable with the public key**, accepted
  for a demo); remove `spike_leaderboard.lua`; delete the smoke-test account
- external smoke check from mobile data: `/healthcheck`, the authenticate preflight and a device-auth round trip; re-verify
  CORS through the tunnel (verified on the LAN 2026-10-28: `Access-Control-Allow-Origin: *`, `Authorization,Content-Type`)

### P2 - `server_key` in `backend.ron` (engine, schema change -> full code workflow)
**Field.** Optional `server_key: String` on `BackendConfig` (`schema/backend.rs`, `#[serde(default)]`, no `schema_version`
bump: it is optional and `pkg/` ships together with the assets; `deny_unknown_fields` only affects a new file on an old
engine). Doc name: "Nakama's `socket.server_key`; ignored by `fake://`". Placed next to `endpoint` in the docs table.

**Validation (`ironhold validate`):** non-empty, length cap, printable ASCII, no whitespace, **no `:`** (the key is sent as
Basic auth `key:`). `--strict` warnings: `server_key` next to a `fake://` or missing endpoint (dead data); `"defaultkey"`
on an `https://` endpoint. The docs must say it is the *public* `socket.server_key`, and that `runtime.http_key` and the
console credentials are secrets that must never be pasted here. Endpoint checks need no new work (the CLI already refuses
plain `http` to a non-LAN host, `backend_checks.rs`).

**Precedence: the file's key goes with the file's own endpoint.**
1. endpoint comes from `backend.ron` (no `BackendEndpointOverride`): `backend.ron`'s `server_key` wins, then the runner key
   (`?nakama_key=`, `NAKAMA_SERVER_KEY`, key file), else the existing "no Nakama server key" error;
2. `BackendEndpointOverride` active (native `--backend http://other`): the file's key is dropped (it must not be sent to a
   different server), the runner key applies, else the error.
One pure function in `ironhold_nakama` owns this rule so both runners share it and the tests have a home.

**Where it is resolved (the runners decide too early today).** The runners build the factory before `backend.ron` has
loaded and return an error-only factory when no key is set (`ironhold_web/src/nakama.rs:31-33`,
`ironhold_native/src/nakama.rs:44-46`); `backend.ron` is read later (`runtime/backend.rs` -> `build_adapter`). So:
(a) add `server_key: Option<String>` to `ResolvedBackendConfig` (filled in `build_adapter`; update the struct literals in
`ironhold_nakama/tests/{live_lan,contract,adapter_flow}.rs`); (b) runners always return a real factory and hand it the
optional runner key; (c) the key is resolved inside the factory with the pure function; (d) **device-id creation moves into
the factory too**: today the web runner touches `localStorage` only when `?nakama_key=` is present, and dropping that gate
without moving it would make every page load of every project (including offline ones) write a persistent
`ironhold.device_id` - a privacy regression that contradicts the "what is stored" note.
`Debug` on `ResolvedBackendConfig`/`BackendConfig` can print the key; it is public, so accepted on purpose (comment it).
Optional: `ironhold backend config` warns when the projects named together disagree on endpoint or `server_key`.

**Docs/comments that go stale and must change:** `docs/20_data_formats.md` (field table at the `backend.ron` section; "Your
backend.ron does not change"; the Web cell, the `play.html?...nakama_key=` example, the "without the key" sentence, the
"no Nakama server key" troubleshooting row, which also gets the stale-`?nakama_key=`-link / stale-env-key cause and a
"rotating" paragraph), `docs/15_authoring_tools.md` (the `backend_config` row and the two `--strict` checks),
`docs/STATUS.md` (hosted build no longer fake-only), `assets/projects/backend_demo/backend.ron:5`, comments in
`ironhold_nakama/src/client.rs:22-23`, `ironhold_web/src/nakama.rs:1-3`, `ironhold_web/src/lib.rs:43`, and
`crates/ironhold_core/src/runtime/CLAUDE.md` ("Online backend": key-resolution and lazy device-id rules). Re-cache
`tools/bin/ironhold` afterwards (the stale binary rejects the new field).
**Rotation wording:** "`server_key` is public: it ships with every build and anyone can read it; it identifies the server,
it does not protect it. To rotate: change the key on the server, update `server_key` in every hosted project's
`backend.ron`, republish. Players with an open page or an old copy fail with `the server rejected the sign-in` until they
reload or update." (On web the RON files are served next to the build; verify that when writing the doc.) Say also that on
web a runner key can only supply a missing key, never change the server.

### P3 - The online demo project `backend_demo_online`
Separate project (decided), created with `/new-project`: copies `backend_demo`'s scene and logic, `game_id:
"backend_demo_online"`, `endpoint: "https://play.frankcreates.com"`, `server_key: "<public key>"`.
- **Explicit sign-in only.** Auto sign-in on `backend.ready` stays off (as in `backend_demo`, `logic/state_machine.ron:16`).
  This keeps `test_web.py` deterministic with no carve-out: nothing is requested at load, so the baseline shows
  `signed_out`, no failed fetch trips the console-error gate, and CI runs create no server accounts. It is a normal
  `PROJECTS` entry.
- **Friendly status in RON, no engine change.** Bind `backend.unreachable` and `backend.sign_in_failed` in `global_on` to
  `SetVariable("demo_msg", "The demo server is offline right now. Press Sign in to try again.")`, clear it on
  `backend.signed_in`, show it in its own Label; keep the raw `last_error` Label as a smaller detail line. (Writing
  `backend.*` variables is rejected by validate, so the message goes through a game variable.) Plain ASCII, labels
  <= ~58 characters at 640 px (labels do not wrap). Add this as a reusable recipe to `docs/30_runtime_events_and_logic.md`
  "Online events".
- **Behind Cloudflare the failure text differs:** a stopped tunnel/Nakama or a rate limit answers an HTTP 502/530/429 page
  (adapter: `SignInFailed`, "server answered HTTP 5xx"; without CORS headers the browser may instead report a transport
  error, i.e. "server unreachable"). Observe the real `last_error` and event for: tunnel down, Nakama down, rate-limited,
  from the Pages origin, and add the rows to the docs/20 troubleshooting table.
- **Visitor notes in two places:** the `index.html` card (before the click) and the in-scene note (replacing
  `fake_note`, 3-4 labels <= 58 chars): an anonymous device id is kept in this browser (and is readable by every page on
  `frankvangeest.github.io`; log this for the hosting investigation: per-game origin or custom domain); the score is saved
  under a random name the server picks; clearing site data or another browser starts a new account and old scores stay
  behind; **scores are not verified** and the board may be reset; the demo server is sometimes offline; to request
  deletion quote the name shown after "Signed in as:" via a contact link (e.g. a GitHub issue).
- **Wording.** Card: "Online Backend Demo (live server): sign in anonymously, submit a score, see a real shared
  leaderboard. Scores are not verified; your account lives in this browser (clearing site data starts a new one). The demo
  server is sometimes offline." README row: "`backend_demo_online` | The same demo against a real Nakama server over https
  (`endpoint` + `server_key` in `backend.ron`); only works with the public demo server it points at." Reword the existing
  `backend_demo` card so the two read as a pair (offline test backend vs live server).
- **`backend_demo_online/backend.ron` header comment:** "This endpoint and key only accept `game_id:
  "backend_demo_online"`. For your own game you need your own Nakama server, or ask its admin to add your game
  (`ironhold backend config`)." A copy that renames `game_id` fails with "the server rejected the sign-in"; one that does
  not rename it writes into the demo leaderboard.
- **Designer path:** add a short "Publishing an online game" checklist to docs/20 under "Running against a Nakama server":
  (1) an `https://` server running the Ironhold module; (2) its admin regenerates `ironhold_games.lua` with your project
  among all the others; (3) set `endpoint` and `server_key`; (4) host the web build together with your `assets/` folder;
  (5) `ironhold validate --strict` shows no `backend_plain_http`. State plainly that the homelab server is not a shared
  hosting service (that is the multi-tenant investigation).
- Regenerate and redeploy `ironhold_games.lua` for both demo games (a game left out can no longer sign in).

### P4 - Release and live check
Normal `/release` on `integration` (full tests, `cargo clean`, WASM release build, `pkg/` in its own commit, release
playtest, fast-forward `main`). The real acceptance test runs **from the Pages origin on a device off the LAN**. Nothing
here is skippable: Pages serves `main`.

## Tasks
- [ ] P1 (homelab repo): tunnel + API-only Traefik router, DOCKER-USER fix, Block-429 rate-limit rule, vault-managed and rotated secrets, module cleanup, external smoke check
- [ ] P2: `server_key` field, validation + two `--strict` checks, pure precedence function in `ironhold_nakama`, `ResolvedBackendConfig` field, runners return a real factory and own lazy device-id creation, tests per precedence case (file, runner, override, none) and a test that nothing is written to `localStorage` without a sign-in attempt where testable
- [ ] P2: docs and comments listed above; `cargo check -p ironhold_cli`; re-cache `tools/bin/ironhold`
- [ ] P2: review cycle (alignment, system-architect, debug-detective; ux for the docs; wasm-perf skipped, no per-frame work)
- [ ] P3: `/new-project backend_demo_online` (explicit sign-in, friendly-status recipe, notes, header comment), regenerate and redeploy `ironhold_games.lua` for both games
- [ ] P3: card, README row, reword the `backend_demo` card, `docs/30` recipe, "Publishing an online game" checklist, baseline (own commit)
- [ ] P3/P4: record the real `last_error` for tunnel down / Nakama down / rate-limited from the Pages origin and add troubleshooting rows
- [ ] P4: `/release`, then the live acceptance checklist from the Pages origin
- [ ] Update `docs/STATUS.md` and the backend_abstraction plan status; log the "device id readable by every Pages site on the origin" finding in the multi-tenant investigation item
- [ ] Tests

## Decisions (Frank, 2026-10-29)
- **Separate online project** `backend_demo_online`; the `fake://` demo and the browser suite stay offline.
- **Privacy:** documented manual deletion on request plus a "what is stored" note; the in-game delete/export action becomes a
  hard requirement for the hosting platform (relax the v2 follow-up from "before any public deployment" to "before
  collecting anything beyond anonymous scores").
- **Abuse:** Cloudflare WAF rate-limit rule only; scores are unverified (said in the notes); module-side limits belong to the
  multi-tenant investigation.
- **Hostname:** `play.frankcreates.com`.
- **Availability:** the demo may be offline; honest UI message, no automatic fallback to `fake://`.
- **Secrets:** the homelab vault is live but not every service uses it yet (migration in the homelab repo, a separate
  session). Gate for P1: Nakama's secrets vault-managed and the leaked ones rotated before the hostname goes live.

## Open questions
- None blocking. Frank may veto the precedence change described under "Plan-review outcome".

## Acceptance criteria
- Given the Pages build of `backend_demo_online` on a phone on mobile data, when the visitor presses Sign in, then the
  screen shows a name and no error; a submitted score appears on the leaderboard, survives a reload (same browser) and shows
  up in a second browser as a different player.
- Given the homelab Nakama (or the tunnel) is stopped, when the visitor presses Sign in, then the friendly "demo server is
  offline" message appears (via `backend.unreachable` or `backend.sign_in_failed`), nothing was requested at page load, and
  pressing Sign in again after the server is back works.
- Given a rate-limited visitor, then the message is still sensible (the observed `last_error` is documented).
- Given the public hostname, when requesting the console or a REST call the module denies, then it is refused.
- Key precedence: file endpoint + `server_key` + no runner key -> sign-in works; file endpoint + both -> the file key is
  used (a stale `?nakama_key=` cannot break the demo); `BackendEndpointOverride` active -> the runner key is used and the
  file key is never sent; neither -> the existing "no Nakama server key" error.
- Loading an offline project (no real endpoint) writes nothing to `localStorage`.
- `cargo test` suites and `ironhold validate --strict` pass for both demo projects; `test_web.py` runs `backend_demo_online`
  with no network traffic and no console errors; the fake demo and the browser suite are otherwise unchanged.
- The Nakama secrets from the 2026-10-28 transcript are no longer valid before the hostname is announced.
