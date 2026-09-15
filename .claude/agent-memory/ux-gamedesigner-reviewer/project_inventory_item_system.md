---
name: inventory-item-system
description: Inventory & item system (ItemCatalog/ItemDef/MerchantDef/ShopEntry, InventoryPanel/ShopPanel) v1 scope, designer footguns, canonical example
metadata:
  type: project
---

Inventory & item system, v1 (added ~2026-06). Canonical example: `3rd_person_game_demo` — `items/items.ron` (5 items), `merchant_vendor` + `chest_01` prefabs, `merchant_01` scene entity, `InventoryPanel`/`ShopPanel` UI nodes, wiring in `logic/state_machine.ron` playing state.

Schema/doc locations:
- `docs/20_data_formats.md`: ItemCatalog/ItemDef/InventoryContainerDef/MerchantDef/ShopEntry tables (~line 2552); InventoryPanel (~715) and ShopPanel (~742) UI sections; `items_path` ProjectConfig row (~104).
- `docs/30_runtime_events_and_logic.md`: 4 `inventory.*` events (~124); 7 actions AddItem/RemoveItem/TransferItem/OpenInventory/CloseInventory/OpenShop/CloseShop (~310).

Designer footguns / scope:
- ~~v1 is display-only for shops.~~ **STALE — `BuyItem` now ships and works.** `3rd_person_game_demo`'s `state_machine.ron` "playing" state wires `ui.button_pressed:buy_item:{key}` → `BuyItem("{key}")` for every merchant stock entry, and the player starts with `gold: 200` in `stats/stats.ron`. `ironhold_cli validate` covers `BuyItem` item keys. Verify current state before repeating any "shops are display-only" claim.
- **`interactable.requires_item`** (shipped 2026-09-14) gates an interact on `PlayerInventory` possession; it does **not** consume the item. Canonical example: `3rd_person_game_demo` `seal_door` prefab + `entity.interact_blocked:seal_door` wiring in `state_machine.ron`'s `global_on:`. See [[entity-event-doc-surfaces]].
- **`AddItem(entity: "player")` is a magic string** routing to the persistent `PlayerInventory` resource; any OTHER string routes to a container entity by spawn id. This is the key conceptual split designers must learn. The chest rule uses `entity: "player"` while the adjacent `ShowFloatingText` uses `entity: "player_01"` (the spawn id) — both correct but the mismatch in one rule block looks like a bug to a designer.
- **PlayerInventory persists across scenes; container Inventory resets on LoadScene** (owned by LevelEntity). Documented at items.ron section intro.

**CLOSED — both prior footguns fixed:** `3rd_person_game_demo/stats/stats.ron` now defines a real
`"gold"` stat entry, so the shipped merchant's `currency_stat: "gold"` is no longer a dead
reference to copy. And `prefabs.ron`'s `merchant_vendor` now uses unquoted `kind: Actor` (the
correct enum convention), not a quoted string. Do not re-flag either.

**Why:** designers copy examples verbatim with no source/error feedback; the player-vs-spawn-id routing and the dead `gold` stat are the two things most likely to waste their time.
