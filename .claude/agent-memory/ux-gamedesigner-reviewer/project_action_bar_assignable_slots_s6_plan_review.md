---
name: project_action_bar_assignable_slots_s6_plan_review
description: Item system S6 (assignable bar slots) plan-review 2026-11-15 + impl review 2026-11-16 - plan traps honoured; open: scene.ready AssignBarSlot doc example clobbers player layout, move emits slot_cleared on source (undocumented), no swap, layout survives "new game"
metadata:
  type: project
---

Plan-reviewed 2026-11-15 at 8c6e254 (gestures needed work). Impl-reviewed 2026-11-16 (uncommitted on
feature/action_bar_assignable_slots): all plan findings honoured - no right-click clear (right click only cancels),
bag open = edit mode (click filled assignable slot picks up), strip with Cancel + Remove from bar (40px buttons),
`item_key:` naming, Put on bar gated on an assignable slot in the scene, two-form slot_assigned, demo slots 7/8 + hint.

Open after impl review:
- docs/20 Assignable example fires AssignBarSlot on `scene.ready:town`: re-runs on every load and overwrites the
  player's own choice. Recommend a one-time trigger.
- A move = built-in ClearBarSlot(source)+AssignBarSlot(target): `slot_cleared:{source}` fires too; docs only tie
  slot_cleared to ClearBarSlot/Remove. Override of `assign_slot:{key}` can't see the item OR the source slot.
- Dropping onto a filled slot replaces (no swap); undocumented.
- Layout lives until page reload, so "back to start menu -> new game" keeps the old layout.
- Demo bag_hint_label grew 47->60 chars in a 500px box (Label ~11px/char): likely overflow.
- Clicking the bar's own bag toggle ("i") during assign mode only cancels (swallowed), bag stays open.

**How to apply:** see [[project_action_bar_item_slots_s5_plan_review]], [[project_item_sell_destroy_s4_plan_review]],
[[project_mouse_default_binding_test_projects]], [[project_ui_label_button_font_and_clip]].
