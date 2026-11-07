---
name: backend-abstraction-pattern
description: Online backend v1+v2 (backend.ron/BackendHub/FakeBackend/BackendFactory/ironhold_nakama) — request/reply Action pattern, runner-supplied config split, and designer traps found in reviews 2026-10-27/28
metadata:
  type: project
---

Online actions are **request Actions**: executor arm -> `dispatch_backend_action` (runtime/backend.rs) pushes onto a resource outbox in
`BackendHub` (SceneStateParams), `backend_glue_system` in `GameStreamSet::Backend` emits GameEvent + writes `backend.*`/`leaderboard.*` vars.
No ActionQueue push from the backend — correct pattern. Refusals (busy/offline) are emitted by the executor itself (next-frame visible).

**Why:** v1 review of feature/backend_abstraction (2026-10-27); v2 review of feature/nakama_adapter (2026-10-28). v3 lobbies/matchmaker reuse the same shape.

**How to apply (checklist for v2/v3 additions):**
- New online Action touches: schema actions.rs (+ `is_presentation_only` exhaustive arm), executor arm, both substitution fns
  (action_substitution.rs), CLI `backend_checks::is_online_action`, docs/20 + docs/30 tables.
- v1 traps since FIXED: `backend.ready` event now emitted; `fake://?fail=..&latency=N` makes FakeFailures RON-reachable;
  `request_timeout_secs` field; project_id-fallback game_id charset now a validate error.
- v2 split (by design): `backend.ron` = endpoint/game_id/leaderboards; runner supplies server key (native env/file, web `?nakama_key=`),
  device uuid, and endpoint override (`--backend`/`?backend=`). `BackendFactory` in `AppOptions`, called once on non-fake endpoint.
- v2 traps found 2026-10-28 (check if fixed): (1) web server key is URL-only, so a plain published link can never be online — no
  data-side `server_key`; (2) runners return `None` factory when the key is missing, so `backend.last_error` reads "no server adapter
  in this build; use fake://" (backend.rs build_adapter) — wrong diagnosis; (3) every factory Err collapses to "could not start (see
  the log)"; (4) Nakama auto-generated usernames shown as display_name, no rename Action; (5) plan's "log line naming the missing
  redeploy" on unknown leaderboard not implemented; (6) no CLI integration test for `ironhold backend config`; (7) stale "(not available
  yet)"/"a later version" comments in backend_demo/backend.ron and docs/20 leaderboards row; (8) `?backend=fake://?fail=a&latency=N`
  needs URL-encoding, undocumented.
- `backend.status` `offline` vs `signed_out` now split (v1 fix).
- public_backend_demo P2 (reviewed 2026-10-30): optional `BackendConfig.server_key` FIXES v2 traps (1) and (2) — runners
  always return a factory; key resolved inside it by `ironhold_nakama::resolve_server_key(file, runner)` (file wins;
  `ResolvedBackendConfig.server_key` is `None` when `BackendEndpointOverride` is active). Format check lives in
  `BackendConfig::problems()` (so runtime + CLI `backend_config` share it); strict kinds `backend_server_key_unused`,
  `backend_default_server_key`. Traps seen: the factory runs at backend.ron resolve (page load), NOT at sign-in, so a
  real-endpoint project writes the device id before any Sign in ("lazy" only vs offline projects); docs/20 "Validation
  rules" bullet list is the usual missed site when a field gains a check; override to the *same* endpoint still drops the
  file key; native missing-key hint says "set server_key in backend.ron" even when `--backend` made that useless.
- public_backend_demo P3 (reviewed 2026-11-03, RON-only, ALIGNED): `backend_demo_online` teaches the friendly-message recipe
  (game var `demo_msg` + own Label, since `backend.*` vars are validate-rejected writes). Traps seen: mapping
  `backend.sign_in_failed` to "server unavailable" misdiagnoses key rotation / game-not-deployed (retry can't help; needs
  reload/admin); recipe covers only sign-in, not in-session `score.submit_failed`/`leaderboard.list_failed`; `backend.offline`
  "not available in this build" misdiagnoses blocked localStorage; load-time `backend.offline` can race the
  `scene.ready` clear of the same var; the `ui/*.ui.ron` copy-snippets drift from the scene (demo_msg label missing).
  docs/20 contradicts itself on gateway-error-page -> sign_in_failed (L5111) vs server unreachable (troubleshooting row).
- backend_dev_stack (reviewed 2026-11-05, dev tooling, ALIGNED): `backend/up.sh` derives the game list from each project's backend.ron
  via `ironhold backend config` (no hardcoded game ids). Doc trap: "the web build cannot use a local server" is overstated — web
  `?backend=` is fake://-only (ironhold_web lib.rs), but `BackendConfig::problems()` accepts `http://` endpoints, so a locally served
  web build with an edited backend.ron (`http://127.0.0.1:7350` + key) CAN reach it; only the *hosted* build can't.
- lobby_rpc_allowlist (reviewed 2026-11-07, server-only, ALIGNED): Nakama 3.41 has no REST `RpcFunc` before-hook, so the guard is
  `game_rpc(id, fn)` in ironhold.lua (regex `^ironhold_lobby_[a-z_]+$`, session game_id required + passed to fn). No RON/Action/CLI
  surface. Gaps seen: "ironhold.lua must be the only module" lives only in a Lua comment + plan, not in docs/20 "Server admin step";
  header still says "reopens `RpcFunc`" though RpcFunc was dropped from the closed list. Future lobby RPCs: check each namespaces
  storage by the passed game_id (wrapper can't enforce that).
- backend v3a-1 lobbies (reviewed 2026-11-07, NEEDS WORK minor): heartbeat 10 s / expiry 60 s / list-min 5 s are protocol+rate-limit
  constants in schema/backend.rs (decision 18) — accept as engine constants (precedent: FIXED_TICK_RATE). Closed vocabulary
  `is_lobby_event`/`is_lobby_variable`/`LOBBY_REASONS` is the designer contract (+ --strict typo check). Traps seen: FakeBackend never
  starts a lobby as a non-host (joiner `lobby.started` unrehearsable); `list_out_of_date` in vocab but never emitted/undocumented;
  `fixed_reason` lets an adapter emit `lobby.closed:full` (outside vocab); runtime sign-out clear hardcodes prefixes instead of
  `RESERVED_VARIABLE_PREFIXES`; LeaveLobby during a pending join answers `backend.busy` and the join then lands; no recovery event after
  `lobby.connection_trouble`; designer-tunable `request_timeout_secs` (to 120) delays heartbeat failure past the fixed 60 s expiry.
  Check these in v3a-2 / v3b reviews.

Related: [[audio-state-pattern]] (engine-written variable precedent), [[real-pause-pattern]], [[diagnostic-only-feature-pattern]], [[web-host-page-contract]].
