# Feature: Inventory & Item System

_Status: Active — v1 scope note: `MerchantDef.buy_price`/`sell_price`/`currency_stat` are display-only; buy/sell transactions (stat deduction + item transfer) are planned for v1.1._
_Planned at: `e9a421e` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [x] **Decide: player inventory persistence across scenes.** The player entity is tagged `LevelEntity` and despawned on `LoadScene`. An `Inventory` component on the player would reset every scene. Solution: a `PlayerInventory` resource (parallel to `LoadedStats`) that persists across scenes. At player spawn time, the scene loader copies `PlayerInventory` into the player's `Inventory` component. Scene-specific containers (chests, crates) use entity-attached `Inventory` components only and reset on scene load. Confirm this split before coding.
>
> - [x] **Decide: `ItemCatalog` file location.** Options: (a) `items/items.ron` per project; (b) `items` map in `assets.ron`. Recommendation: **separate `items/items.ron`** — items are gameplay data, not rendering assets; separate file keeps `assets.ron` focused on paths; mirrors the pattern for `groups.ron` and `stats.ron`.
>
> - [x] **Decide: inventory slot model — indexed vs. unordered bag.** Options: (a) fixed-size indexed grid (`Vec<Option<ItemStack>>`, `max_slots`) — visual grid UI, easy drag-and-drop; (b) unordered bag (`HashMap<String, u32>`) — no visual grid but simpler query. Recommendation: **indexed slots** (`Vec<Option<ItemStack>>`) — v1 UI will be a grid; stackable items fill the first available slot with the same key; non-stackable items each occupy one slot.
>
> - [x] **Decide: `AddItem` target — player only or any entity.** Chests, crates, and merchants all have inventories. `AddItem` takes an `entity: String` (spawn ID) parameter. When `entity` is `"player"` (a reserved ID), it routes to `PlayerInventory`. For other entities, it targets the entity's `Inventory` component. Confirm this routing before coding.
>
> - [x] **Decide: `HasItem` condition scope for v1.** Rule-level `Condition` expressions don't exist yet (icebox). `HasItem` as a dialogue choice condition will be added to `DialogueCondition` when this feature ships (a small patch to the dialogue system). Quest objectives handle their own item checks internally. Do not add a general rule-level `HasItem` in v1.
>
> - [x] **Decide: merchant UI — inline shop panel or scene overlay.** Options: (a) `OpenShop` action opens a full-screen overlay scene; (b) a `ShopPanel` UI node declared in the scene. Recommendation: **`ShopPanel` UI node in scene RON** — consistent with `DialoguePanel`; declared once per scene, reused by any `OpenShop` action.

---

## What

A data-driven item catalog (`items/items.ron`) and inventory system. Items are defined as `ItemDef` entries with a display name, icon, and optional properties (stackable, weight, tags). Entities carry `Inventory` components; the player's inventory persists across scenes in a `PlayerInventory` resource. Merchants are declared inline on `PrefabDef` with a stock list and prices denominated in a stat currency.

---

## Why

Required foundation for equipment stat bonuses, quest collect objectives, loot drops, and any reward system. Without items there is no economy, no progression artefacts, and no meaningful pickups.

Unblocks: Equipment system (hard dep), Quest `Collect` objectives (dep), Loot system (hard dep), Dialogue `HasItem` condition (soft dep).

---

## Schema

### `items/items.ron`

```ron
(
    schema_version: 1,
    items: {
        "health_potion": (
            display_name: "Health Potion",
            icon: "icons/items/health_potion",  // texture catalog key
            stackable: true,
            max_stack: 10,
            weight: 0.2,
            tags: ["consumable", "potion"],
        ),
        "iron_sword": (
            display_name: "Iron Sword",
            icon: "icons/items/iron_sword",
            stackable: false,
            max_stack: 1,
            weight: 3.5,
            tags: ["weapon", "equippable"],
        ),
        "gold_coin": (
            display_name: "Gold",
            icon: "icons/items/gold_coin",
            stackable: true,
            max_stack: 9999,
            weight: 0.01,
            tags: ["currency"],
        ),
    },
)
```

### New `ItemCatalog` + `ItemDef` (`schema/items.rs`)

```rust
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct ItemCatalog {
    pub schema_version: u32,
    pub items: HashMap<String, ItemDef>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    pub display_name: String,
    /// Texture catalog key for the item's icon.
    pub icon: String,
    /// When true, multiple units stack in one inventory slot.
    #[serde(default = "default_true")]
    pub stackable: bool,
    /// Max units per stack. Ignored when `stackable: false` (treated as 1). Default: 99.
    #[serde(default = "default_max_stack")]
    pub max_stack: u32,
    /// Item weight in arbitrary units. Used for future carry-weight system. Default: 1.0.
    #[serde(default = "default_weight")]
    pub weight: f32,
    /// Designer-defined tags for filtering, sorting, and condition matching.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ItemStack {
    pub item_key: String,
    pub count: u32,
}
```

### `PrefabDef` additions (`schema/catalog.rs`)

```ron
// prefabs/prefabs.ron
"chest_prop": (
    kind: "prop",
    model: "props/chest",
    interactable: ( radius: 2.0 ),
    inventory: Some(( max_slots: 9 )),     // NEW — has an inventory container
)

"merchant_npc": (
    kind: "actor",
    model: "characters/merchant",
    merchant: Some((                        // NEW — merchant stock declaration
        stock: [
            ( item_key: "health_potion", buy_price: 10, sell_price: 5, stock_count: None ),
            ( item_key: "iron_sword", buy_price: 150, sell_price: 60, stock_count: Some(1) ),
        ],
        currency_stat: "gold",             // stat key used as currency (global LoadedStats)
    )),
)
```

```rust
// schema/catalog.rs — in PrefabDef
/// When set, this entity has an inventory container. Max slots declared here.
#[serde(default)]
pub inventory: Option<InventoryContainerDef>,

/// When set, this entity acts as a merchant. OpenShop fires on interaction.
#[serde(default)]
pub merchant: Option<MerchantDef>,
```

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct InventoryContainerDef {
    #[serde(default = "default_max_slots")]
    pub max_slots: usize,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct MerchantDef {
    pub stock: Vec<ShopEntry>,
    /// Global stat key used as currency. Default: "gold".
    #[serde(default = "default_currency")]
    pub currency_stat: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ShopEntry {
    pub item_key: String,
    pub buy_price: u32,
    pub sell_price: u32,
    /// None = unlimited stock. Some(n) = restocks n items on scene load.
    #[serde(default)]
    pub stock_count: Option<u32>,
}
```

### `ProjectConfig` addition

```ron
items_path: Some("items/items.ron"),
```

### Scene RON — `InventoryPanel` + `ShopPanel` UI nodes

```ron
// scenes/town.scene.ron
ui: [
    ( id: "inventory_panel", kind: InventoryPanel((
        position: (20.0, 20.0),
        columns: 5,
        rows: 6,
        slot_size: 48.0,
        initially_hidden: true,
    ))),
    ( id: "shop_panel", kind: ShopPanel((
        position: (500.0, 20.0),
        columns: 4,
        rows: 4,
        slot_size: 48.0,
        initially_hidden: true,
    ))),
]
```

---

## Runtime

### Resources (`capabilities/inventory.rs`)

```rust
/// Player inventory — persists across scene transitions (like LoadedStats).
#[derive(Resource, Default)]
pub struct PlayerInventory {
    pub slots: Vec<Option<ItemStack>>,
    pub max_slots: usize,
}

/// Loaded from items/items.ron.
#[derive(Resource, Default)]
pub struct LoadedItemCatalog(pub Option<ItemCatalog>);
```

### `Inventory` component

```rust
/// Entity-attached inventory for containers (chests, crates). Cleared on LoadScene.
#[derive(Component, Default)]
pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
    pub max_slots: usize,
}
```

### New actions (`schema/actions.rs`)

```ron
AddItem(entity: "player", item_key: "health_potion", count: 3)
RemoveItem(entity: "player", item_key: "health_potion", count: 1)
TransferItem(from: "chest_01", to: "player", item_key: "iron_sword", count: 1)
OpenInventory         // show InventoryPanel (player bag)
CloseInventory        // hide InventoryPanel
OpenShop("merchant_01")   // show ShopPanel populated from merchant's MerchantDef
CloseShop
```

```rust
AddItem { entity: String, item_key: String, #[serde(default = "one")] count: u32 },
RemoveItem { entity: String, item_key: String, #[serde(default = "one")] count: u32 },
TransferItem { from: String, to: String, item_key: String, #[serde(default = "one")] count: u32 },
OpenInventory,
CloseInventory,
OpenShop(String),  // spawn ID of the merchant entity
CloseShop,
```

### New pipeline events

```ron
inventory.added:{entity_id}:{item_key}:{count}
inventory.removed:{entity_id}:{item_key}:{count}
inventory.full:{entity_id}                       // AddItem blocked; no space
inventory.transferred:{from}:{to}:{item_key}
shop.purchased:{item_key}:{count}
shop.sold:{item_key}:{count}
```

### `AddItem` routing logic

```rust
fn add_item(entity_id: &str, item_key: &str, count: u32, /* resources */) {
    let slots = if entity_id == "player" {
        &mut player_inventory.slots
    } else {
        let entity = registry.entities.get(entity_id)?;
        &mut inventory_query.get_mut(*entity).ok()?.slots
    };
    // find existing stack or empty slot, add items, emit event
}
```

---

## `PrefabKey` component

Add `PrefabKey(String)` component to all spawned entities at spawn time. Required by the Quest system's `KillCount` objective and the Loot system's death-loot dispatcher.

```rust
#[derive(Component, Debug, Clone)]
pub struct PrefabKey(pub String);  // set in entity_spawner.rs at spawn time
```

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `schema/items.rs` — `ItemCatalog`, `ItemDef`, `ItemStack`
- [ ] `items_path` in project config; loaded by `project_loader`
- [ ] `PlayerInventory` + `LoadedItemCatalog` resources
- [ ] `Inventory` component; `InventoryContainerDef` + `MerchantDef` + `ShopEntry` on `PrefabDef`
- [ ] `PrefabKey` component inserted at spawn time in `entity_spawner.rs`
- [ ] `InventoryPanel` + `ShopPanel` `UiNodeDef` variants; scene loader spawns them
- [ ] `AddItem`, `RemoveItem`, `TransferItem`, `OpenInventory`, `CloseInventory`, `OpenShop`, `CloseShop` actions
- [ ] Scene loader: copy `PlayerInventory` → entity `Inventory` at player spawn; `Inventory` container prefabs
- [ ] `PlayerInventory` NOT cleared on `LoadScene` (persists); `Inventory` components cleared with `LevelEntity`
- [ ] `DialogueCondition::HasItem` patch to dialogue system when this feature ships
- [ ] Pipeline events: `inventory.added`, `inventory.removed`, `inventory.full`, `shop.purchased`, `shop.sold`
- [ ] Demo: chest with items in `entity_logic_demo`; `TransferItem` on interact; shop NPC in `3rd_person_game_demo`
- [ ] Integration tests: add/remove/transfer items; full inventory emits `inventory.full`; player inventory persists across scene load; merchant stock respected
- [ ] Docs: `ItemDef`, `Inventory`, `MerchantDef` in `docs/20_data_formats.md`

---

## Acceptance criteria

- Given `AddItem(entity: "player", item_key: "health_potion", count: 3)`, the player has 3 potions and `inventory.added:player:health_potion:3` is emitted.
- Given the player inventory is full, `AddItem` emits `inventory.full:player` and does not add the item.
- Given `RemoveItem` with a count exceeding what the player holds, it removes all held and emits the event with the actual count removed.
- Given a scene transition, `PlayerInventory` retains its contents; a chest's `Inventory` is reset.
- Given `OpenShop("merchant_01")`, the `ShopPanel` shows the merchant's stock with buy/sell prices.
- Given `shop.purchased:health_potion:1`, the player's gold stat decreases by the buy price.
