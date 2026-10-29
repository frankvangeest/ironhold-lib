# Feature: Public backend demo (GitHub Pages talking to the homelab Nakama)

_Status: Draft (awaiting plan-review and the decisions under "Open questions")_
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

## Approach

### P1 - Homelab stage 2 (lives in the homelab repo, `planning/features/nakama-deployment.md`)
Not duplicated here; this plan only states what the engine side needs from it:
- hostname (proposed `play.frankcreates.com`), valid TLS (Cloudflare Tunnel -> Traefik `letsencrypt`), routing **only** the
  client API to port 7350; the console keeps `nakama.home`, LAN-only
- the `DOCKER-USER` / ufw-docker bug fixed first, so the console is not LAN-wide exposed while the API is public
- Cloudflare WAF rate-limit rule (decide numbers: e.g. N requests/min/IP on `/v2/account/authenticate/*`)
- a smoke check from outside the LAN (mobile data): `curl https://<host>/healthcheck` and the authenticate preflight
- **secrets:** rotate the Nakama secrets printed into the 2026-10-28 transcript before the hostname goes live (see
  Open question 2: by hand now vs wait for the vault)
- decide which game id(s) the public server hosts; redeploy `ironhold_games.lua` for exactly those; remove
  `spike_leaderboard.lua`; delete the smoke-test account

### P2 - `server_key` in `backend.ron` (engine, schema change -> full code workflow)
- New optional `server_key: String` on `BackendConfig` (`#[serde(default)]`, backward compatible, no `schema_version` bump
  unless the reviewer disagrees). Validated like the other fields (non-empty, sane length, no whitespace).
- Resolution order for the Nakama adapter: runner-supplied key (`?nakama_key=` / `NAKAMA_SERVER_KEY` / key file) wins,
  then `backend.ron`'s `server_key`, else the existing "no Nakama server key" error. Explicit override wins so a developer
  can point a published project at their own server without editing the file.
- `ironhold validate` accepts it, `query`/CLI output unchanged unless it lists backend fields, docs updated
  (`docs/20_data_formats.md` table and the "Running against a Nakama server" section, which currently says the page address
  supplies it), a test for each precedence case in `ironhold_nakama` / `ironhold_core`.
- Doc must say plainly: it is public, it ships in the build, rotating it means rebuilding clients.

### P3 - The online demo project
Decision needed (Open question 1). Recommendation: **a new project `backend_demo_online`** (via `/new-project`) that copies
`backend_demo`'s scene and logic, with `game_id: "backend_demo_online"`, `endpoint: "https://<public host>"` and the
`server_key`. Reasons: `test_web.py` and the baselines stay offline and deterministic (no flaky network in the suite);
the fake demo keeps documenting the failure paths; the online one is tiny. Cost: one more project to register (README row,
`index.html` card, `validate_projects.rs`, `test_web.py` `PROJECTS` - the latter must exclude it from network-dependent
steps or accept `backend.offline` as its steady state, to be decided in the plan-review) and a baseline.
- The module's `ironhold_games.lua` is regenerated for **both** demo games (`ironhold backend config` over every hosted
  project; a game left out can no longer sign in) and redeployed to the homelab.
- Scene shows the online state honestly: `backend.offline` / `server unreachable` must read as "the demo server is
  offline right now" with a retry, because the homelab will sometimes be off. (Check `backend_demo`'s existing labels cover it.)
- Short note on the page about what is stored: an anonymous device id and the score under a server-chosen random name;
  cleared browser data starts a new account. (Privacy, see Open question 3.)

### P4 - Release and live check
Normal `/release` on `integration` (full tests, `cargo clean`, WASM release build, `pkg/` in its own commit, release
playtest, fast-forward `main`). After Pages deploys, the real acceptance test runs **from the Pages origin on a device off
the LAN** (see acceptance criteria). Nothing here is skippable: Pages serves `main`.

## Tasks
- [ ] P1 (homelab repo): tunnel + Traefik API-only router, DOCKER-USER fix, rate-limit rule, secret rotation, module cleanup
- [ ] P1: external smoke check (`/healthcheck`, authenticate preflight and a device-auth round trip from mobile data)
- [ ] P2: `server_key` field in `backend.ron` schema, validate, adapter precedence, docs, tests, `cargo check -p ironhold_cli`
- [ ] P2: review cycle (alignment, system-architect, debug-detective; ux for the doc change)
- [ ] P3: `/new-project backend_demo_online`, endpoint and key, regenerate and redeploy `ironhold_games.lua` for both games
- [ ] P3: offline/unreachable wording check, short "what is stored" note, `index.html` card, README row, baseline (own commit)
- [ ] P4: `/release`; after deploy run the live acceptance checklist from the Pages origin
- [ ] Update `docs/STATUS.md` (hosted build no longer fake-only) and the backend_abstraction plan status
- [ ] Tests

## Decisions (Frank, 2026-10-29)
- **Hostname:** `play.frankcreates.com`.
- **Availability:** the demo may be offline; honest UI message, no automatic fallback to `fake://`.
- **Secrets:** the homelab vault is live but not every service uses it yet (migration is in progress, in the homelab repo,
  in a separate session). Gate for P1: Nakama's secrets are vault-managed and the ones printed into the 2026-10-28
  transcript are rotated before the hostname goes live. This plan does not change the homelab repo; it only waits on it.

## Open questions (recommendations pending Frank's confirmation)
1. **Separate online project (recommended) or switch `backend_demo` itself?** Separate keeps the browser suite and baselines
   offline and deterministic and keeps the `fake://` failure-path demo; costs one more registered project.
2. **Privacy/deletion:** recommended: documented manual deletion on request plus a "what is stored" note for the demo; the
   in-game delete/export action stays a hard requirement for the hosting platform (relax the v2 follow-up from "before any
   public deployment" to "before collecting anything beyond anonymous scores").
3. **Abuse limits:** recommended: Cloudflare WAF rate-limit rule only; scores are unverified (say so in the demo note);
   module-side limits belong to the multi-tenant investigation.

## Acceptance criteria
- Given the Pages build of `backend_demo_online` opened on a phone on mobile data, when the visitor presses sign in, then
  the leaderboard screen shows a name and no error; submitting a score appears on the leaderboard and survives a reload
  (same browser) and shows up in a second browser as a different player.
- Given the homelab Nakama is stopped, when the page loads, then the demo shows the offline message and no console errors
  other than the failed request, and recovers when the server returns.
- Given the public hostname, when requesting the console path or any REST call the module denies, then it is refused
  (console unreachable from outside, denied APIs answer an error).
- Given a `backend.ron` with `server_key` and no `?nakama_key=`, when the page loads, then sign-in works; given both,
  the page-address value wins; given neither, the existing "no Nakama server key" error appears.
- `cargo test` suites and `ironhold validate --strict` pass for both demo projects; the fake demo and the browser suite are
  unchanged.
- The Nakama secrets from the 2026-10-28 transcript are no longer valid before the hostname is announced.
