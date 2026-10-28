---
name: backend-abstraction-plan-review
description: backend_abstraction (Nakama online actions) plan review 2026-10-25, v1 impl review 2026-10-27, v2 nakama_adapter docs review 2026-10-28; open designer gaps
metadata:
  type: project
---

Plan review 2026-10-25 -> v1 reviewed 2026-10-27 (canonical example `assets/projects/backend_demo`, docs/20 `backend.ron` section)
-> v2 (feature/nakama_adapter) docs reviewed 2026-10-28: docs/20 "Running against a Nakama server", docs/15 "Online backend".

Closed by v2: docs/30 "connecting" pattern now exits on failed/unreachable/offline; leaderboard vars cleared on SignOut;
docs/README has an online row; `ironhold backend config` documented; fake:// failure rehearsal (`?fail=`, `latency=`).

Open at v2 review (re-check next time):
- Missing server key makes the runner install NO factory, so `backend.last_error` reads "no server adapter in this build; use
  fake://" (core runtime/backend.rs build_adapter) - contradicts docs saying the reason is shown. Same for unavailable localStorage.
- No designer-facing source for the server key value (Nakama `socket.server_key`, default "defaultkey") and no troubleshooting
  table mapping last_error texts ("the server rejected the sign-in" = wrong key, "unknown game" = game not in deployed
  ironhold_games.lua) to fixes.
- `ironhold backend config` output REPLACES the whole games list: regenerating with one project drops the others -> their
  sign-in fails "unknown game". Not warned. Changing ranking/keep of an existing board likely no-ops (leaderboard_create on an
  existing id) - unverified, undocumented.
- Stale text: docs/20 leaderboards row "(a later version)"; backend_demo/backend.ron comment "(not available yet)"; STATUS
  "hosted web build still uses the fake" (really: the demo's backend.ron; ?backend= can repoint it). Hardcoded "15 seconds" vs
  request_timeout_secs.
- Device id = account: clearing site data / new browser = new account, scores orphaned; display_name is Nakama's random username.
- Still from v1: backend.busy unscoped; validate does not check Label bind leaderboard.<x> names.

**Why:** online is the first async, can-fail event source; real-adapter failures surface only as last_error text.
**How to apply:** for any backend change, check last_error wording against docs and the troubleshooting path. Related:
[[project_docs_lag_actions]], [[project_auto_written_gamevariables_undocumented]], [[project_fsm_designer_traps]],
[[project_warn_vs_silent_fallback_principle]].
