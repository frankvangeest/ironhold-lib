---
name: project_action_bar_item_slots_s5_plan_review
description: Item system S5 (ActionSlotDef.item) plan + impl review 2026-11-15 - plan traps all honoured; open: icon/icon_index rows stale, validate msgs name UseItem not the slot, gamepad_index side effects uncommented
metadata:
  type: project
---

Plan-reviewed 2026-11-15 at a40dbd7; impl-reviewed 2026-11-15 (uncommitted on feature/action_bar_item_slots). Verdict: ship after small doc fixes.

Honoured at impl: icon resolved like the bag (item sheet -> InventoryPanel sheet+grid, bar sheet ignored); only `icon:`
switches picture (icon_index alone ignored); runtime warns (item+do_actions, unknown item, no use:, no icon once per item
per SESSION - Local never reset); refusal = no_item only (no pressed/intent/activated/cooldown/cost); demo gamepad_index:0
on 3 player prefabs; count font slot_size*0.3 min 10, bottom-left, `99+`, grey 0.55; RON unwrapped; owner_player co-op note.

Open after impl review:
- docs/20 ActionSlotDef `icon`/`icon_index` rows never mention item slots; icon_index says "icon_sheet on the bar must be set".
- validate unknown-item/no-use errors come from a SYNTHESIZED UseItem: message says `UseItem("x")`/`item_key "x"`, no slot/bar.
- demo prefab comment says gamepad_index is "needed for gamepad_key" but it also turns on stick move/turn + A/B/X/Y.
- Empty slot cooldown: overlay fades to 0.25 then pops to 0.45 dim (no sweep; alpha only).
- Demo player starts with 0 potions -> both slots grey/dim at start; need merchant/chest to test.

**How to apply:** at S6 (runtime-assigned slots) recheck icon rows, validate wording, and label/tooltip default; see
[[project_item_use_s3_plan_review]], [[action-bar-single-player-assumptions]], [[project_ui_label_button_font_and_clip]].
