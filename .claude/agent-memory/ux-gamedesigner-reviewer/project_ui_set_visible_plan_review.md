---
name: ui-set-visible-plan-review
description: SetUiVisible plan-review (2026-10-11, split from responsive_ui) - bool-only vs "toggle button" demo clash; docs/30 ShowUi/HideUi placeholder + docs/20 ~3215 "SetEntityVisible can't hide UI Label" callout must change; scene.ready start-hidden recipe untested
metadata:
  type: project
---

Plan-review of planning/features/ui_set_visible.md (Planned at e3a2ff2). Verdict: Needs work.

Findings to verify on the implementation review:
- Demo says "a button that toggles a Group" but the action is bool-only; one button cannot toggle without
  FSM state. Recommended: two buttons (Hide B / Show B) on ui_demo `station_b` (stations row is Center-justified,
  so A/C visibly reflow). ui_demo has no ActionBar/panel, so rejection is never demoed.
- Since `initially_hidden` is omitted, `SetUiVisible(..., false)` on `scene.ready:<scene>` IS the start-hidden
  recipe; needs an acceptance test + doc line (does the node exist yet; one-frame flash?).
- Designer action-list surfaces actually present: docs/20 Actions table (~3928-3965), docs/30 "Action categories"
  (~193 visibility list + ~198 `UI actions 🧭 ShowUi(panel_id)/HideUi(panel_id)` placeholder - replace it),
  docs/30 implementation list (~355-380), docs/STATUS.md (~105). Plan says "five" without naming them.
- docs/20 ~3215 callout "SetEntityVisible cannot target a scene-authored UI Label ... can't be hidden in v1"
  becomes stale the moment this ships; rewrite to point at SetUiVisible.
- Undocumented behaviours to state: bound labels keep updating while hidden; scene/global key bindings that emit
  the same trigger as a hidden Button still fire; hidden state resets on LoadScene / overlay reopen; no way to hide
  an ActionBar at runtime (say so, don't just say "move it out"); panel-rejection message text missing.

**Why:** these are the first-hour traps for the runtime show/hide primitive every HUD swap will use.
**How to apply:** check each bullet against the shipped docs/20, docs/30, STATUS, docs/15 and ui_demo.
Related: [[project-responsive-ui-plan-review]], [[project-docs-lag-actions]], [[project-ui-trigger-wiring]],
[[project-ui-nesting-flat-scan-sites]].
