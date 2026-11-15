---
name: project-slot-menu-system
description: Item S4 bag slot menu (slot_menu.rs) — slot_menu_sync_system PostExecute every frame; closed path alloc-free; open path rebuilds MenuView + clones MerchantDef per frame
metadata:
  type: project
---

Item system S4 (reviewed 2026-11-15, feature/item_sell_destroy): `slot_menu_sync_system` (`GameplaySet::PostExecute`, ungated).

- Closed path: `slot_menu.clone()` of `None` = no alloc; ResMut read via Deref does NOT mark changed; root display write guarded. Effectively free.
- Nobody reads `LoadedInventoryUi.is_changed()` (grep 2026-11-15), so ResMut writes there wake nothing — re-check if a reader is added.
- Open path (per frame, as written at review): SlotMenu clone (1-2 Strings), `MerchantDef.clone()` (deep Vec<ShopEntry>) when a shop is open, currency String clone, menu_view ~3-4 format!, `MenuView::text()` 9 String allocs even when unchanged. Writes are guarded so no relayout/reshape; cost is allocator churn only. Fix advised: borrow MerchantDef, text() -> &str, optionally Local cache of last drawn SlotMenu.
- ChildOf re-parent compare is correct (inserts once per open). Slots are never despawned at runtime, so the menu root parented under a slot survives.
- 1 root + 9 rows + 9 texts per InventoryPanel, all Display::None at spawn: taffy skips, empty Text costs nothing; confirm toggle blanks/refills verb row texts (per-click reshape, fine).
- Zero new deps.

Related: [[project-inventory-system]], [[project-intent-event-layer]]
