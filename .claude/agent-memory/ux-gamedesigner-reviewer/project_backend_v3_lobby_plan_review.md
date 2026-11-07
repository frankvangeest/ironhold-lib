---
name: backend-v3-lobby-plan-review
description: backend_abstraction v3/v3a lobby plan reviews (2026-11-05, 2026-11-07); both Needs more design work; core gap = RON rules cannot branch on variable values
metadata:
  type: project
---

Round 1 (2026-11-05, v3 three bullets): Needs more design work. Led to v3a (lobby.current.*, member.<n>, StartLobby, lobby.last_error reasons).

Round 2 (2026-11-07, v3a "Ready" section ~260-325): Needs more design work. Key finding:
- **State-machine rules have no variable conditions** (only dialogue `condition: HasVariable`). So the plan's "rows hidden with SetUiVisible keyed on lobby.count", an empty-state label, and "designers word messages from lobby.last_error (demo_msg pattern)" are all impossible in RON. demo_msg in backend_demo_online works only because each failure is a DIFFERENT EVENT. I wrongly recommended reason-in-variable in round 1; correct fix is reason-suffixed events (`lobby.join_failed:full`, emitted after the generic one) plus per-row events (`lobby.row_filled:<n>`/`lobby.row_empty:<n>`) or a `lobby.list_empty` event, or ship backlog's Label/Group hide-when-empty.
- Other open items: member.<n> cleared on SignOut/empty LIST instead of left/closed; no `not_signed_in`/`not_in_lobby`/`started`/`name_invalid` reasons; backend.busy vs lobby.*_failed; identical designer-constant lobby names (suggest `lobby.<n>.host`); fake:// demo card "Finds and joins players" untrue (bots); `fake://?fail=` lobby options + heartbeat-drop rehearsal missing; docs plan omits docs/15 validate rows and the docs/20 `features` row ("nothing reads them"); `query events` lists only used events, so lobby vocabulary not discoverable; deferred result-event --strict check should land for closed `lobby.` vocabulary; Cloudflare /v2/rpc rate rule vs designer refresh loops.

Impl review (2026-11-07, feature/backend_v3a_1): row/member_row/list_empty + reason-suffixed events SHIPPED (round-2 gap closed). Open: fake_note 88 chars in 900px box (docs/20 rule size/(font*0.6) = ~68 chars) + 375px phone at min_scale 0.5 runs off-screen; snippet comments claim "labels do not wrap" vs docs/20 ~842 "wrapped text"; docs/30 ~442 "event names carry only ids you wrote, reason in backend.last_error" now contradicted by lobby.*_failed:<reason>; SignOut -> lobby.left handler calls ListLobbies (order vs backend.signed_out undocumented); browse panel visible before sign-in/after SignOut; create_failed:already_in_lobby has no Leave affordance in browse (real-server reload trap); fake member script undocumented; reason-per-operation matrix missing; misleading indentation of rows in lobby.ui.ron; backend.ron cites planning/ path.

**Why:** v3 is the first feature where the designer expects multiplayer; any feature promising "SetUiVisible keyed on a variable" must be checked against the no-conditions rule.
**How to apply:** for ANY future plan saying "designer branches on / words messages from variable X", verify an event exists per value. Related: [[backend-abstraction-plan-review]], [[public-backend-demo-plan-review]], [[ui-set-visible-plan-review]].
