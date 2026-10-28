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

Related: [[audio-state-pattern]] (engine-written variable precedent), [[real-pause-pattern]], [[diagnostic-only-feature-pattern]], [[web-host-page-contract]].
