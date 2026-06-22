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
- **v1 is display-only for shops.** No buy/sell transaction; `buy_price`/`sell_price`/`currency_stat`/`stock_count` deduction not wired. Documented in 3 places (ShopPanel note, MerchantDef row, bottom scope note) — well covered.
- **`AddItem(entity: "player")` is a magic string** routing to the persistent `PlayerInventory` resource; any OTHER string routes to a container entity by spawn id. This is the key conceptual split designers must learn. The chest rule uses `entity: "player"` while the adjacent `ShowFloatingText` uses `entity: "player_01"` (the spawn id) — both correct but the mismatch in one rule block looks like a bug to a designer.
- **`currency_stat` should reference a real stat key.** Shipped merchant uses `currency_stat: "gold"` but `3rd_person_game_demo/stats/stats.ron` only defines `player_health`/`player_mana`. No error in v1 (display-only) but it is a misleading example to copy.
- **PlayerInventory persists across scenes; container Inventory resets on LoadScene** (owned by LevelEntity). Documented at items.ron section intro.
- Doc prefab example uses quoted `kind: "Prop"`/`kind: "Actor"`; shipped prefabs.ron uses UNQUOTED `kind: Prop`/`kind: Actor` (enum). Quoted form is the wrong convention to copy — flag if it persists.

**Why:** designers copy examples verbatim with no source/error feedback; the player-vs-spawn-id routing and the dead `gold` stat are the two things most likely to waste their time.
