---
name: backend-abstraction-pattern
description: Online backend v1 (backend.ron/BackendHub/FakeBackend/SignIn/SubmitScore/ShowLeaderboard) — request/reply Action pattern, reserved engine-written vars, and the designer traps found in review 2026-10-27
metadata:
  type: project
---

Online actions are **request Actions**: executor arm -> `dispatch_backend_action` (runtime/backend.rs) pushes onto a resource outbox in
`BackendHub` (SceneStateParams), `backend_glue_system` in `GameStreamSet::Backend` emits GameEvent + writes `backend.*`/`leaderboard.*` vars.
No ActionQueue push from the backend — correct pattern. Refusals (busy/offline) are emitted by the executor itself (next-frame visible).

**Why:** v1 review of feature/backend_abstraction (2026-10-27). v2 adds `ironhold_nakama`; v3 lobbies/matchmaker reuse the same shape.

**How to apply (checklist for v2/v3 additions):**
- New online Action touches: schema actions.rs (+ `is_presentation_only` exhaustive arm), executor arm, both substitution fns
  (action_substitution.rs ~122/251 + ~147/276), CLI `backend_checks::is_online_action`, docs/20 + docs/30 tables.
- Traps found in v1 (check if fixed): no `backend.ready` event when backend.ron finishes loading, so `scene.ready` -> SignIn can
  race into `backend.busy` with nothing to re-trigger; `FakeFailures` not RON-reachable (only tests) though a doc comment claims
  designers can use it; 15 s timeout hardcoded (`DEFAULT_REQUEST_TIMEOUT_SECS`) with no backend.ron field; reserved
  `backend.*`/`leaderboard.*` vars only protected by a doc sentence (no validate check on SetVariable to them); effective game_id
  from `project_id` fallback is not charset-validated; docs/30 "searching state" example lacks transitions for
  sign_in_failed/busy/offline (gets stuck); plan's `--strict` "bindings on result events no action can produce" not implemented.
- `backend.status` conflates "no backend configured" and "not signed in" (both `offline`).

Related: [[audio-state-pattern]] (engine-written variable precedent), [[real-pause-pattern]], [[diagnostic-only-feature-pattern]].
