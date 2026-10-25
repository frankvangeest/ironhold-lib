---
name: backend-abstraction-plan-review
description: 2026-10-25 plan review of planning/features/backend_abstraction.md (Nakama online actions); verdict Needs more design work; key designer gaps to re-check when v1 lands
metadata:
  type: project
---

Reviewed `planning/features/backend_abstraction.md` (Draft, planned at cd90a12) on 2026-10-25. Verdict: Needs more design work.

Gaps to re-check at implementation review time:
- `lobby.listed:{json}` / `matchmaker.matched:{json}` break the exact-match `<category>.<verb>:<id>` rule (docs/20 ~3968); no UI can show a JSON list; no text-input widget exists, so player-named lobbies/display names are impossible.
- No action to READ a leaderboard (only SubmitScore); `score.submitted` has no board payload; plan text itself says `backend.score_submitted` in one place.
- Cloud save payload undefined; local `save_load_game_state.md` is still Draft (not shipped) and uses `save.written/loaded/load_failed:{slot}` — cloud names should mirror it.
- `backend.ron` needs schema_version + a `backend_path` in ProjectConfig (no convention-path fallback exists); `game_id` overlaps `project_id`.
- Async replies vs state-scoped `on:` (same trap as EmitEventAfterDelay); delayed events can't be cancelled so designer-built matchmaking timeouts misfire -> want `timeout_secs`.
- v3 matchmaking/lobbies are a dead end for designers until the v4 relay exists.
- Leaderboard config changes need regenerate + redeploy via CLI that designers may not have.

**Why:** online features are the first async, seconds-late, can-fail event source in the pipeline; every existing pattern assumes same/next-frame replies.
**How to apply:** when v1 code/docs arrive, check each item above; also check docs/30 Same-frame event order lists backend events, and docs/20 engine-written GameVariables table gains backend keys. Related: [[project_docs_lag_actions]], [[project_auto_written_gamevariables_undocumented]], [[project_fsm_designer_traps]].
