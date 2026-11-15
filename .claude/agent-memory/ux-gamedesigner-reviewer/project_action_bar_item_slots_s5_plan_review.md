---
name: project_action_bar_item_slots_s5_plan_review
description: Item system S5 (ActionSlotDef.item) plan-review 2026-11-15 - icon_index is u32 default 0 so "own icon wins when set" is undetectable; bar-sheet fallback gives wrong icon in demo; demo player has no gamepad_index
metadata:
  type: project
---

Plan-reviewed 2026-11-15 at a40dbd7 (planning/features/action_bar_item_slots.md). Verdict: Needs more design work (small).

Traps found (recheck at impl review):
- `ActionSlotDef.icon_index` is `u32` default 0 (docs/20 ~1035), not Option: "own icon_index wins when set" can't tell
  0-authored from omitted. Only `icon` (String "") and `icon_color` (Option) are detectable.
- Plan's icon fallback "ItemDef.icon_sheet else the BAR's icon_sheet": 3rd_person bar uses `icons_status_effects`,
  items have no icon_sheet and rely on InventoryPanel's `icons_items` -> potion slot would show status-effect cell 2
  (Dual Strike). Same 8x8x64 grid, different sheet. Recommended: resolve like the bag (item sheet -> InventoryPanel sheet+grid).
- 3rd_person_game_demo player has NO `inputs.gamepad_index` -> any `gamepad_key` there is a hard validate error.
- docs/20 ~5148 recipe `do_actions: [UseItem]` slot burns cooldown when empty; S5 item: slots refuse - two semantics.
- Plan RON example used `Some(...)` on cooldown_secs/icon_color; demo style is unwrapped.
- `label` on slots still unrendered; item slot should default it to display_name for the future tooltip.

**How to apply:** at S5 impl review and S6 (runtime-assigned slots) check these; see [[project_item_use_s3_plan_review]],
[[action-bar-single-player-assumptions]], [[action-bar-click-activation]], [[project_ui_label_button_font_and_clip]].
