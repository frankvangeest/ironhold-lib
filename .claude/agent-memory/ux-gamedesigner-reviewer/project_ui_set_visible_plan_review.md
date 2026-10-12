---
name: ui-set-visible-plan-review
description: SetUiVisible plan + implementation review (2026-10-11/12) - shipped surfaces, open gaps (overlay start-hidden has no recipe, same-list-as-LoadScene no-op undocumented, room8 join_prompt still unwired, validate msgs have embedded space runs)
metadata:
  type: project
---

Plan-review 2026-10-11 (Planned at e3a2ff2), implementation review 2026-10-12 on feature/ui_set_visible.

Shipped and fine: two-button demo (ui_demo Hide B / Show B on `station_b`); docs/20 Actions row with anchor
`#setuivisible`; Group trip-up #10; docs/30 x2 rows; STATUS; docs/15 two validate rows; scene.ready start-hidden
recipe documented (with "one-frame flash possible"); panel/ActionBar rejection + "ActionBar can't be hidden yet".

Open at implementation review (check whether fixed next time):
- validate.rs set_ui_visible_checks messages (~901/920/925) contain ~27-30 literal spaces mid-sentence (lost `\`
  line continuation) - designer sees "Groups and                leaves".
- ActionBar message says "move the bar outside the Group" even when the target IS the bar.
- No doc says SetUiVisible in the same do_actions list as LoadScene/LoadSceneOverlay no-ops (UI spawns deferred),
  and overlays get no scene.ready, so there is NO start-hidden recipe for overlay nodes.
- docs/20 ~3217 coop callout still headlined "can't be hidden in v1 ... author it always-visible" then says hide
  it with SetUiVisible (self-contradictory); room8 join_prompt still not wired and its RON comment says it can't be.
- docs/15 rows filed under "Action bars" instead of "Scenes and UI layout".
- Jargon in designer docs: Display::None, Visibility::Hidden, UiNodeDefs.
- Hidden Button's trigger still fires from scene_key_bindings - undocumented.

**Why:** these are the first-hour traps for the runtime show/hide primitive every HUD swap (responsive_ui
phase 3) will use.
**How to apply:** on responsive_ui phase 3 review, re-check each open bullet.
Related: [[project-responsive-ui-plan-review]], [[project-docs-lag-actions]], [[project-ui-trigger-wiring]],
[[project-ui-nesting-flat-scan-sites]].
