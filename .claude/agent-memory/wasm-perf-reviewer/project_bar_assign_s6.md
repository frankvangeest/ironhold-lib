---
name: project-bar-assign-s6
description: Item system S6 assignable action bar slots (bar_assign.rs) — idle cost, LoadedInventoryUi change-flag discipline, per-frame clones only while assign mode is on
metadata:
  type: project
---

Reviewed 2026-11-16 (feature/action_bar_assignable_slots, uncommitted diff on 7bbed7d).

- `bar_assign_click_system` (EmitSet::Ui) and `bar_assign_sync_system` (PostExecute) both run every frame with `ResMut<LoadedInventoryUi>`, but on idle frames only *read* through Deref (`ui.assign_mode.clone()` of None = no alloc), so they do NOT flip the change flag that `action_bar_item_slot_system` gates on via `inv_ui.is_changed()`. `default_panel_actions_system` end-of-mode block is also read-guarded. Re-check this invariant on any edit: one stray `&mut *ui` / unconditional field write makes the item-slot refresh run every frame.
- Do NOT recommend `run_if` on `bar_assign_click_system`: its `Ref<Interaction>` is_changed baselines depend on the system running every frame (documented in the fn doc).
- Only per-frame allocations: `assign_mode.clone()` in both systems WHILE the mode is on (1-3 Strings each) — transient state, flagged should-fix (borrow via `as_ref()`, clone lazily on a press).
- `ActionBarAssignments::get` allocates 2 Strings per lookup (owned tuple key) — only called at spawn / gated reconcile, nit.
- slot_menu open path gained `assignable.iter().filter().count()` (O(≤12), alloc-free) + "Put on bar" String per frame, on top of the pre-existing per-frame menu label churn ([[project-slot-menu-system]]).
- Zero new deps.

**Why:** LoadedInventoryUi is now a shared hub with is_changed readers; change-flag hygiene is the main hot-path risk.
**How to apply:** when reviewing later item-system stages, grep every `ResMut<LoadedInventoryUi>` system for unconditional writes.
