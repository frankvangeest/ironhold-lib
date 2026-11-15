---
name: project-action-bar-item-slots
description: Item system S5 action-bar item slots - refresh system is change-gated (free on idle), visual system adds a per-child SlotItemCount lookup per slot per frame (negligible), zero deps
metadata:
  type: project
---

Reviewed 2026-11-15 (feature/action_bar_item_slots, uncommitted at the time).

- `action_bar_item_slot_system` (GameplaySet::PostExecute) early-outs unless PlayerInventory / LoadedItemCatalog / LoadedInventoryUi changed or `Or<(Changed<ActionSlotItem>, Added<SlotIcon>)>` is non-empty. Verified no per-frame writer of those three resources: PlayerInventory is mutated only in executor txn arms; LoadedInventoryUi only on bag-menu clicks (inventory_slot_click_system, slot_menu_sync_system stale-close, take_menu_quantity) and scene load; catalog only at project load. So idle frames do almost nothing (a non-archetypal Changed scan over item-slot entities only).
- It is now the first `is_changed` reader of LoadedInventoryUi (S4 memory said there were none): every bag-menu open/close/confirm triggers a full item-slot refresh. Cheap, O(item slots x bag slots).
- `action_bar_visual_system`: `children.iter().find_map(|c| counts.get(c).ok())` per slot per frame = ~3-5 failed entity lookups per non-item slot. Sub-microsecond for typical bars; flagged as a note only.
- `warned.insert(item.0.clone())` clones before the set check (allocation per refresh per icon-less slot). Nit.
- Spawn-time item icon is an ImageNode with Visibility::Hidden + default handles: same UI pipeline, no new pipeline variant.

**Why:** future reviews of S6 (runtime-assignable slots inserting/removing ActionSlotItem) should know the refresh gate relies on Changed<ActionSlotItem>; RemovedComponents<ActionSlotItem> is NOT in the gate.
**How to apply:** when S6 lands, check removal triggers a refresh and the icon/count children are despawned or hidden. Related: [[project-inventory-system]], [[project-slot-menu-system]], [[project-intent-event-layer]].
