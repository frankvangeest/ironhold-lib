---
name: backend-v3-lobby-plan-review
description: backend_abstraction v3 (lobbies + matchmaker) plan review 2026-11-05; verdict Needs more design work; missing "current lobby" vars, empty-row Join, full/stale/gone reasons
metadata:
  type: project
---

v3 plan (planning/features/backend_abstraction.md "### v3", table ~99-100) reviewed 2026-11-05: Needs more design work.

Gaps raised (re-check when v3 is implemented):
- No "my lobby" variables (lobby.current.name/players/is_host, member list) and no host/ready/start signal -> a waiting room cannot be built.
- Unset vars render empty but buttons stay clickable (see leaderboard.ui.ron comment): JoinLobby(row: 7) on an empty row needs a defined answer.
- lobby.join_failed is one event for full / gone / stale snapshot / server down; designer needs a reason var (lobby.last_error) not new event names.
- lobby.list_failed and join_failed lack the {id} pattern other areas use; unscoped is OK only because there is one list.
- No lobby.<n>.* clearing on SignOut/empty list; lobby.count = 0 "No lobbies yet" recipe needed.
- Event naming: matchmaker.matched vs lobby.joined; no remote-leave/host-left/kicked events; connection_lost behaviour inside a lobby unspecified.
- "matching only" must be on-screen in the demo, not only in docs; recommended a new backend_demo_lobby project.

**Why:** v3 is the first feature where the designer expects a multiplayer game and gets none.
**How to apply:** at v3 impl review check the five doc surfaces (docs/20 actions+vars+last_error table, docs/30 recipe, demo RON comments, STATUS, index card). Related: [[backend-abstraction-plan-review]], [[public-backend-demo-plan-review]].
