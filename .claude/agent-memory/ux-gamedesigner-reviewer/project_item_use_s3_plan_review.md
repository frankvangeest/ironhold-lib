---
name: project_item_use_s3_plan_review
description: Item system S3 (UseItem, use: block, default-backed buy_item/use_item clicks) plan + impl review - "bound anywhere" means do_actions: [] in ONE state disables the built-in in EVERY state; co-op bar slot {self}=player 1
metadata:
  type: project
---

Plan-reviewed 2026-11-14 (60e28b0); impl-reviewed 2026-11-15 on feature/item_use. Plan points all closed in the impl
(do_actions naming, buy_item dropped from unreachable_trigger, transitions/behaviors do not count, no default for items
without use:, buy-message promise reworded, demo HUD hint).

Open at impl review (check at S4/S5):
- **"Bound anywhere" trap:** `BoundPanelEvents` (capabilities/panel_defaults.rs) is one global set. docs/20 "Using items"
  and the plan both said "to switch the built-in off in some state, bind the event with `do_actions: []`" - wrong: that
  switches it off in EVERY state. Correct recipe = bind `[BuyItem(k)]`/`[UseItem(k)]` in each state where it should work
  (what the demo's `playing` Buy rules do). Exact-match only, so one binding per item key.
- `default_panel_actions_system` has no SimClock/FSM gate: with no rule, a Buy click works in any FSM state, incl. a
  real pause if the shop is visible. Slot clicks ARE pause-gated (inventory_slot_click_system).
- UseItem `{self}` = primary player (lowest player_index) also when fired from player 2's action bar slot.
- gamepad_key in the docs' bar-slot recipe needs `inputs.gamepad_index` on the owner (`gamepad_key_without_gamepad_index`).
- Right-click on a slot vs 3rd_person `orbit_button: "Right"` leak: playtest item only, undocumented.

**How to apply:** S4 (menu, Sell) will add more default-backed verbs - recheck the same per-state recipe text; see
[[project_item_txn_s2_plan_review]], [[project_panel_world_blocking]], [[project_ui_trigger_wiring]], [[project_fsm_designer_traps]].
