---
name: bar-assign-click-ownership-hazards
description: S6 assign mode — right click over a bag slot still fires S4's use_item shortcut before the mode cancels; "bag open" = root Visibility != Hidden, so an initially_hidden:false bag is permanent edit mode
metadata:
  type: project
---

Found in the S6 (`feature/action_bar_assignable_slots`, 2026-11-16) review, uncommitted diff on top of `7bbed7d`:

- `inventory_slot_click_system` runs BEFORE `bar_assign_click_system` in the EmitSet::Ui chain and its step 2 emits `use_item:{key}` for a
  right click on a *hovered* bag slot unconditionally. So "a right click in assign mode only cancels" is false whenever the cursor is over
  the bag (and the mode's strip lives inside the bag panel). Tests press Right with nothing hovered, so they miss it.
- `bag_shown()` = any `InventoryPanelMarker` root `Visibility != Hidden`. A panel with `initially_hidden: false` is always "open", so a
  mouse click on a filled assignable slot is ALWAYS a pick-up (never a use) and the bag-hidden cancel never fires. Root Visibility also
  ignores a hidden ancestor.
- The gesture move pushes `ClearBarSlot(from)` BEFORE `AssignBarSlot(to)`: a refused assign (currency/no-use default, only strict/non-fatal
  in validate) loses the origin.

**Why:** Each new "mode" layered on the bag inherits S4's click semantics unless explicitly gated; check every earlier system in the
EmitSet::Ui chain for emissions the new mode should suppress.
**How to apply:** When reviewing any future bag/bar mode (drag, equip, split), check the S4 right-click shortcut and the
`initially_hidden: false` panel case first. Related: [[ui-pick-blocking]], [[in-place-confirm-double-click]].
