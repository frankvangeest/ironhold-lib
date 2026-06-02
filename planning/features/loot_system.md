# Feature: Loot System

_Status: Draft_
_Planned at: `6adb6bf` (2026-06-02)_
_Hard dep: Inventory & item system_
_Soft deps: Quest system (Collect objective auto-advance), Equipment system (equippable drops)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: `entity.died` integration — system-detected vs. designer-called.** Options: (a) a `loot_on_death_system` watches `GameEvent::Trigger` for `entity.died:*` and auto-rolls loot for any entity with a `LootTableRef` component; (b) designer explicitly calls `RollLootTable(entity: "{self}")` in the entity's behavior file alongside `Despawn`. Recommendation: **option (b), explicit `RollLootTable` in behavior files** — consistent with the pipeline philosophy (no implicit magic systems); gives designers full control (not all deaths drop loot even if the prefab has a table — e.g. a boss death might have a scripted loot drop instead). The `LootTableRef` component provides the table key, so the action takes only `entity: "{self}"`.
>
> - [ ] **Decide: loot bag entity — prefab key or hardcoded.** The loot bag needs a visual (mesh or sprite). Options: (a) hardcoded internal entity with a default mesh; (b) `loot_bag_prefab: Option<String>` field on `LootTableDef` — designer declares which prefab key to use for the bag. Recommendation: **per-table `loot_bag_prefab: Option<String>`** — designers can use a chest, coin pile, or glowing orb; `None` = spawn no visual entity (for `auto_loot: true` mode).
>
> - [ ] **Decide: `auto_loot` — per-scene or per-table.** Options: (a) `auto_loot: bool` on `GameSceneV2`; (b) per-table flag; (c) both. Recommendation: **per-scene via scene RON** (`auto_loot: true` in scene config) — a full scene can opt into direct-to-inventory drops (e.g. a mobile-style game); individual tables override with `force_bag: true` to always spawn a bag even in auto-loot scenes.
>
> - [ ] **Decide: RNG source.** `rand::thread_rng()` is not deterministic. For the Beta 0.5 deterministic tick milestone, the loot system must use a seeded `DeterministicRng` resource. v1 can use `thread_rng()` with a note that it must be replaced at Beta 0.5.
>
> - [ ] **Decide: nested table references for v1.** A `LootEntry` referencing another table key (for tiered pools like "roll Common table OR Uncommon table") adds implementation complexity. Recommendation: **defer nested tables to v2** — flag it in open questions; v1 supports only flat entry lists.

---

## What

RON-defined loot tables (`loot/loot_tables.ron`) containing weighted drop entries. `RollLootTable` action rolls a table and places results in a `LootBag` entity (or directly in the player's inventory if auto-loot is active). Designers wire the action into behavior files alongside `Despawn`.

---

## Why

Without a loot system, item drops require manually crafting per-enemy `AddItem` rules — verbose and hard to balance. The loot system provides a data-driven, randomised drop layer that integrates cleanly with the inventory system and quest `Collect` objectives.

---

## Schema

### `loot/loot_tables.ron`

```ron
(
    schema_version: 1,
    tables: {
        "bandit_grunt_drops": (
            strategy: RollEach,
            loot_bag_prefab: Some("loot_bag_small"),
            entries: [
                ( item_key: "gold_coin",     chance: 0.9, min_count: 5,  max_count: 15 ),
                ( item_key: "health_potion", chance: 0.3, min_count: 1,  max_count: 1  ),
                ( item_key: "iron_sword",    chance: 0.05, min_count: 1, max_count: 1,
                  quality: Some(Uncommon) ),
            ],
        ),
        "boss_chest_drops": (
            strategy: RollOne,
            loot_bag_prefab: None,   // auto-loot directly; no bag entity
            entries: [
                ( item_key: "legendary_axe", chance: 0.1, min_count: 1, max_count: 1,
                  quality: Some(Legendary) ),
                ( item_key: "epic_ring",     chance: 0.3, min_count: 1, max_count: 1,
                  quality: Some(Epic) ),
                ( item_key: "gold_coin",     chance: 0.6, min_count: 50, max_count: 200 ),
            ],
        ),
    },
)
```

### New types (`schema/loot.rs`)

```rust
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct LootCatalog {
    pub schema_version: u32,
    pub tables: HashMap<String, LootTableDef>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LootTableDef {
    /// RollEach: every entry is rolled independently (multiple drops possible).
    /// RollOne: one entry selected weighted by chance (mutually exclusive drops).
    #[serde(default)]
    pub strategy: LootRollStrategy,
    /// Prefab key for the spawned bag entity. None = no visual; items go to auto-loot or nothing.
    #[serde(default)]
    pub loot_bag_prefab: Option<String>,
    pub entries: Vec<LootEntry>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub enum LootRollStrategy {
    #[default]
    RollEach,
    RollOne,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LootEntry {
    pub item_key: String,
    /// Probability 0.0–1.0 this entry drops. For RollOne, used as relative weight.
    pub chance: f32,
    #[serde(default = "one")]
    pub min_count: u32,
    #[serde(default = "one")]
    pub max_count: u32,
    /// Optional quality tag for icon border tinting in the loot UI.
    #[serde(default)]
    pub quality: Option<ItemQuality>,
}

#[derive(Deserialize, Debug, Clone)]
pub enum ItemQuality {
    Common, Uncommon, Rare, Epic, Legendary,
}
```

### `PrefabDef` addition (`schema/catalog.rs`)

```ron
"bandit_grunt": (
    kind: "actor",
    model: "characters/bandit_grunt",
    loot_table: Some("bandit_grunt_drops"),  // NEW
    // ...
)
```

```rust
// schema/catalog.rs — in PrefabDef
/// Loot table key in LootCatalog. Used by RollLootTable action.
#[serde(default)]
pub loot_table: Option<String>,
```

### `ProjectConfig` addition

```ron
loot_path: Some("loot/loot_tables.ron"),
```

### `GameSceneV2` addition

```ron
auto_loot: false,   // NEW — when true, RollLootTable adds items directly to player inventory
```

```rust
// schema/scene_v2.rs
#[serde(default)]
pub auto_loot: bool,
```

---

## Runtime

### Components (`capabilities/loot.rs`)

```rust
/// Inserted at spawn time when PrefabDef.loot_table is set.
#[derive(Component)]
pub struct LootTableRef(pub String);  // loot table key

/// Marks a loot bag entity. Holds the rolled items before pickup.
#[derive(Component)]
pub struct LootBag {
    pub items: Vec<ItemStack>,  // rolled contents
}
```

### Resources

```rust
#[derive(Resource, Default)]
pub struct LoadedLootCatalog(pub Option<LootCatalog>);

/// Set from scene RON on load.
#[derive(Resource, Default)]
pub struct LootSceneConfig {
    pub auto_loot: bool,
}
```

### New actions (`schema/actions.rs`)

```ron
// Roll loot for the given entity (reads LootTableRef component)
RollLootTable(entity: "{self}")

// Transfer all items from a loot bag to the player's inventory
PickupLoot("{bag_entity_id}")

// Despawn a loot bag without transferring items (e.g. timeout)
ClearLootBag("{bag_entity_id}")
```

```rust
/// Roll loot for the given entity using its attached LootTableRef.
/// Spawns a loot bag (or auto-loots) based on scene config.
/// No-op with warning if the entity has no LootTableRef.
RollLootTable { entity: String },

/// Transfer all items from a loot bag entity to the player.
/// Emits loot.collected per item. Despawns the bag entity.
PickupLoot(String),

/// Despawn a loot bag without transferring. Used for auto-despawn timers.
ClearLootBag(String),
```

### `RollLootTable` executor logic

```rust
Action::RollLootTable { entity } => {
    let ecs_entity = registry.entities.get(&entity)?;
    let table_key = loot_table_ref_query.get(*ecs_entity).ok()?.0.clone();
    let table = loot_catalog.tables.get(&table_key)?;

    let rolled = roll_table(table, &mut rng);  // returns Vec<ItemStack>

    if rolled.is_empty() { return; }

    game_events.write(GameEvent::Trigger(format!("loot.rolled:{}", table_key)));

    for stack in &rolled {
        game_events.write(GameEvent::Trigger(
            format!("loot.item_dropped:{}:{}:{}", table_key, stack.item_key, stack.count)
        ));
    }

    if scene_config.auto_loot || table.loot_bag_prefab.is_none() {
        // Direct-to-inventory
        for stack in &rolled {
            action_queue.push(Action::AddItem {
                entity: "player".into(),
                item_key: stack.item_key.clone(),
                count: stack.count,
            });
            game_events.write(GameEvent::Trigger(
                format!("loot.collected:{}:{}", stack.item_key, stack.count)
            ));
        }
    } else {
        // Spawn a loot bag entity
        let bag_id = format!("loot_bag_{}", registry.next_id());
        let bag_pos = global_transforms.get(*ecs_entity)
            .map(|t| t.translation())
            .unwrap_or(Vec3::ZERO);

        action_queue.push(Action::Spawn {
            prefab: table.loot_bag_prefab.clone().unwrap(),
            id: Some(bag_id.clone()),
            position: Some((bag_pos.x, bag_pos.y, bag_pos.z)),
            spawn_point: None,
            yaw_deg: None,
        });

        // Attach LootBag contents — via a PendingLootBag resource
        pending_loot_bags.insert(bag_id.clone(), rolled);

        game_events.write(GameEvent::Trigger(format!("loot.bag_spawned:{}", bag_id)));
    }
}
```

### `roll_table` function

```rust
fn roll_table(table: &LootTableDef, rng: &mut impl Rng) -> Vec<ItemStack> {
    match table.strategy {
        LootRollStrategy::RollEach => {
            table.entries.iter()
                .filter(|e| rng.gen::<f32>() < e.chance)
                .map(|e| ItemStack {
                    item_key: e.item_key.clone(),
                    count: rng.gen_range(e.min_count..=e.max_count),
                })
                .collect()
        }
        LootRollStrategy::RollOne => {
            let total_weight: f32 = table.entries.iter().map(|e| e.chance).sum();
            let mut roll = rng.gen::<f32>() * total_weight;
            for entry in &table.entries {
                roll -= entry.chance;
                if roll <= 0.0 {
                    return vec![ItemStack {
                        item_key: entry.item_key.clone(),
                        count: rng.gen_range(entry.min_count..=entry.max_count),
                    }];
                }
            }
            vec![]
        }
    }
}
```

### `PickupLoot` executor logic

1. Find loot bag entity by spawn ID.
2. Read `LootBag.items`.
3. For each item: push `Action::AddItem { entity: "player", ... }`.
4. Emit `loot.collected:{item_key}:{count}` per item.
5. Push `Action::Despawn(bag_id)`.

---

## Worked example

```ron
// behaviors/bandit_grunt.behavior.ron
(
    schema_version: 1,
    initial_state: "alive",
    states: [
        ( name: "alive",
          on: [
            ( event: "entity.died:{self}", do_actions: [
                RollLootTable(entity: "{self}"),
                Despawn("{self}"),
            ]),
          ]
        ),
    ],
    transitions: [],
    global_on: [],
)

// In scene rules.ron — player interaction with bag
( on: "entity.interacted:{bag_*}", do_actions: [ PickupLoot("{event_entity}") ] ),
// Note: wildcard matching on bag IDs requires the Custom objective pattern
```

Or via trigger zone on the bag:
```ron
( on: "entity.entered:loot_bag_0", do_actions: [ PickupLoot("loot_bag_0") ] ),
```

---

## New pipeline events

```ron
loot.rolled:{table_key}
loot.item_dropped:{table_key}:{item_key}:{count}
loot.bag_spawned:{bag_entity_id}
loot.collected:{item_key}:{count}
```

---

## New Rust changes

- `schema/loot.rs` (new file) — `LootCatalog`, `LootTableDef`, `LootRollStrategy`, `LootEntry`, `ItemQuality`.
- `schema/catalog.rs` — `loot_table: Option<String>` on `PrefabDef`.
- `schema/scene_v2.rs` — `auto_loot: bool` on `GameSceneV2`.
- `schema/actions.rs` — `RollLootTable`, `PickupLoot`, `ClearLootBag`.
- `capabilities/loot.rs` (new file) — `LootTableRef`, `LootBag`, `LoadedLootCatalog`, `LootSceneConfig`, `roll_table`.
- `capabilities/mod.rs` — register module.
- `runtime/scene_manager/action_executor.rs` — `RollLootTable`, `PickupLoot`, `ClearLootBag` arms.
- `runtime/scene_manager/scene_loader.rs` — populate `LootSceneConfig` from scene RON; insert `LootTableRef` at spawn time.
- `Cargo.toml` — `rand` crate (if not already present).

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `schema/loot.rs` — all types; `ImplicitRonPlugin` registered
- [ ] `loot_path` in project config; `loot_table` on `PrefabDef`; `auto_loot` on scene
- [ ] `LootTableRef` component inserted at spawn time; `LootBag` component
- [ ] `LoadedLootCatalog` + `LootSceneConfig` resources
- [ ] `roll_table` function — `RollEach` and `RollOne` strategies
- [ ] `RollLootTable`, `PickupLoot`, `ClearLootBag` actions + executor arms
- [ ] `PendingLootBag` resource for deferred component attachment to spawned bag entities
- [ ] Pipeline events: `loot.rolled`, `loot.item_dropped`, `loot.bag_spawned`, `loot.collected`
- [ ] Loot bag pickup UI (interactable prompt + item list panel before pickup)
- [ ] Note in code: RNG must migrate to `DeterministicRng` at Beta 0.5
- [ ] Demo: `bandit_grunt.behavior.ron` with `RollLootTable` + `Despawn`; player walks over bag to collect
- [ ] Integration tests: `RollEach` rolls each entry independently; `RollOne` produces exactly one drop; `auto_loot: true` goes direct to inventory; `PickupLoot` transfers and despawns bag; empty roll emits `loot.rolled` but no `loot.bag_spawned`
- [ ] Docs: `LootTableDef`, `loot_table`, `auto_loot` in `docs/20_data_formats.md`; loot actions + events in `docs/30_runtime_events_and_logic.md`

---

## Open questions

- **Nested table references**: `LootEntry` referencing another table key (for tiered pools). Deferred to v2 — requires cycle detection and recursive rolling.
- **Loot bag despawn timer**: bags should despawn after N seconds if not collected. `Action::EmitEventAfterDelay` + `ClearLootBag` from the bag's behavior file is the designer-side solution for v1 — no magic auto-despawn. Designers add to `loot_bag_small.behavior.ron`.
- **Instanced vs. shared loot for multiplayer**: each player rolling their own loot vs. shared bag. Not in scope for v1; document as a networking-milestone concern.
- **`rand` determinism at Beta 0.5**: `thread_rng()` is non-deterministic. The Beta 0.5 milestone introduces `DeterministicRng` resource. Replace `rng: impl Rng` with `Res<DeterministicRng>` at that milestone.

---

## Acceptance criteria

- Given `RollLootTable(entity: "bandit_01")` where the table has `strategy: RollEach`, each entry is rolled independently and `loot.item_dropped` fires for each dropped item.
- Given `strategy: RollOne`, exactly one entry is selected and `loot.item_dropped` fires once.
- Given `auto_loot: false`, a loot bag entity is spawned and `loot.bag_spawned:{id}` is emitted.
- Given `auto_loot: true`, items are added directly to the player's inventory and `loot.collected` fires per item; no bag entity is spawned.
- Given `PickupLoot(bag_id)`, all bag items are added to player inventory, `loot.collected` fires per item, and the bag entity is despawned.
- Given `RollLootTable` on an entity with no `LootTableRef`, a warning is logged and no loot is generated.
- Given `chance: 0.0` on an entry, it never drops across 1000 rolls. Given `chance: 1.0`, it always drops.
