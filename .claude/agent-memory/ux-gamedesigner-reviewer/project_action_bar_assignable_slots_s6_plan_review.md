---
name: project_action_bar_assignable_slots_s6_plan_review
description: Item system S6 (assignable bar slots, AssignBarSlot/ClearBarSlot, assign mode) plan-review 2026-11-15 - right-click clear clashes with bag right-click=Use and orbit (no UI-hover guard); touch has no cancel/clear; item vs item_key naming
metadata:
  type: project
---

Plan-reviewed 2026-11-15 at 8c6e254. Verdict: Needs more design work (gestures only; schema/persistence fine).

Key findings handed back:
- Right click on a bar slot = Clear contradicts bag right click = Use (same gesture, opposite meaning) and the
  demo's orbit_button "Right"; camera_orbit_system has NO UI-hover guard (docs/dev/action-bar-input-routing.md), so
  a right-drag orbit that starts over the bar would clear a slot. Recommended: no right-click clear; while the bag
  is open a click/tap on a filled assignable slot opens a small "Remove from bar" menu (bag open = edit mode).
- Touch: no right click -> no cancel and no clear for a grey-0 slot whose item left the bag. Recommended a hint
  strip with a [Cancel] button; hint text must not say "right click".
- Action field naming: AssignBarSlot(item:) vs every item action's `item_key:` (AddItem/RemoveItem/SellItem).
- "Put on bar" gate should be "a visible assignable slot in this scene", not "the project has one".
- Hotkey press during assign mode unspecified; closing the bag should cancel (mirrors S4 menu stale-close).
- Demo: slot "8" is a NEW slot (bar has 1-7, KeyE, i); only 2 consumables both already on 6/7; only main.scene has
  the bar, so persistence test = pause -> start menu -> character -> main round trip.
- Docs: S4 menu table (docs/20 ~5160), "not built yet (S6)" (~5220) and "dragging to the action bar" (~5154) lines.

**How to apply:** at S6 impl review, recheck these; see [[project_action_bar_item_slots_s5_plan_review]],
[[project_item_sell_destroy_s4_plan_review]], [[project_mouse_default_binding_test_projects]].
