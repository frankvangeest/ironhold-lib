---
name: backend-abstraction-plan-review
description: backend_abstraction (Nakama online actions) plan review 2026-10-25 + v1 implementation review 2026-10-27; open designer gaps to re-check at v2
metadata:
  type: project
---

Plan review 2026-10-25 (Needs more design work) -> plan revised (no payloads in event names, per-op failure events,
indexed `leaderboard.<name>.<n>.*` variables, backend_path + schema_version). v1 implementation reviewed 2026-10-27
(worktree ironhold-lib-backend_abstraction, canonical example `assets/projects/backend_demo`, docs/20 `backend.ron` section).

Open at v1 review (re-check at v2):
- docs/30 "searching state with a timeout" pattern has no exit on `backend.sign_in_failed`/`backend.offline`/`backend.busy` -> stuck in `connecting`.
- `backend.status` value `offline` means "signed out" too, while event `backend.offline` means "no backend at all"; offline reason (bad backend.ron, no adapter for https endpoint) only goes to console, never `backend.last_error`.
- `backend.busy` is unscoped (no leaderboard id) and sets no last_error; SubmitScore while signing in answers busy, not submit_failed.
- FakeBackend answers in ~2 frames and never fails -> designers cannot rehearse connecting/failure/timeout paths (only tests can inject).
- `ShowLeaderboard` only fetches into variables (name implies UI). Snippet rows render "4. " / "Signed in as: " when empty (format prefix stays).
- backend.*/leaderboard.* keys missing from docs/20 Label "GameVariables auto-written" table; docs/30 frame-order list lacks backend replies (refusals are executor-emitted = next frame, replies same frame).
- validate does not check Label `bind: "leaderboard.<x>..."` against declared boards (find-and-replace snippet workflow makes this the likely typo).
- Leaderboard vars not cleared on SignOut. docs/README index has no online entry.
- Still v2+: `ironhold backend config` server-admin step undocumented; cloud save waits on save_load_game_state.

**Why:** online is the first async, can-fail event source; every existing pattern assumes same/next-frame replies.
**How to apply:** at v2 review check each item, plus real-adapter latency makes the stuck-state and busy gaps bite. Related: [[project_docs_lag_actions]], [[project_auto_written_gamevariables_undocumented]], [[project_fsm_designer_traps]], [[project_same_frame_event_order]].
