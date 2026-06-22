# Feature: Equipment System

_Status: Draft_
_Planned at: `6adb6bf` (2026-06-02)_
_Hard dep: Inventory & item system_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Stat/actions/UI — slot system, `Equip`/`Unequip`, stat bonuses, `EquipmentPanel` | Queued | — |
| v2 | Visual mesh attachment — `AttachmentDef`, bone socket authoring, runtime bone queries (Icebox) | Icebox | — |

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: slot identity — string keys vs. fixed enum.** Options: (a) fixed enum (`MainHand`, `OffHand`, `Head`, `Chest`, `Legs`, `Feet`, `Ring`, `Neck`); (b) string keys declared in `EquipmentSlotsDef` per character. Recommendation: **string keys** — more designer-friendly; a sword-only enemy declares `slots: { "weapon": (...) }`; a fully-geared player declares all standard slots; custom game types (mech loadout, ship modules) declare arbitrary slot names without changing the engine.
>
> - [ ] **Decide: stat bonus application — inline `ModifierDef` vs. reuse existing modifier key.** Options: (a) `stat_bonuses: Vec<StatBonusDef>` inline on `ItemDef`, each with `stat_key` + `ModifierKind`; (b) `modifier_key: String` reference pointing to a named `ModifierDef` in `stats.ron`. Recommendation: **inline `StatBonusDef`** — equip bonuses are per-item, not reusable templates; inline keeps `ItemDef` self-contained; no cross-file dependency for simple cases. Add a `modifier_keys: Vec<String>` optional list for the case where a designer wants to share a complex bonus.
>
> - [ ] **Decide: visual swap on equip (bone/socket attachment) — v1 scope.** Swapping a mesh at a skeleton bone requires knowing the GLB bone name and spawning a child entity with a `SceneRoot`. This is complex and requires GLB authoring conventions. Recommendation: **defer visual swap to a v2 follow-up.** v1 implements stat bonuses, conditions, and UI only. Note `model_attachment: Option<AttachmentDef>` as a placeholder field on `ItemDef` that is parsed but not yet acted on.
>
> - [ ] **Decide: stat bonus removal on unequip — reference-counted vs. snapshot delta.** When an item is unequipped, its bonuses must be removed. `RemoveModifier` removes ALL instances of a named modifier — but equipment bonuses are inline, not named modifiers. Solution: **snapshot the effective stat delta at equip time**, store it as `EquipmentBonusSnapshot` on the entity, and subtract it on unequip. Simpler than named modifiers; works for Additive bonuses. For Multiplicative bonuses, store the inverse multiplier.
>
> - [ ] **Decide: two-handed and dual-wield exclusion.** `ItemDef` can declare `two_handed: bool`. When a two-handed weapon is equipped in `MainHand`, `OffHand` is automatically unequipped. Declare as a boolean on `ItemDef` in the schema; implement exclusion logic in the `Equip` executor arm. Confirm this is in v1 scope.

---

## What

Equippable items that carry stat bonuses. Entities declare available slot keys in `EquipmentSlotsDef` on their prefab. `Equip` / `Unequip` actions manage `EquipmentMap` components. Bonuses are applied to `StatMap` / `LoadedStats` on equip and reversed on unequip.

v1 scope: stat bonuses + UI. Visual mesh swap deferred.

---

## Why

Without equipment, items have no mechanical effect. Weapons, armour, and accessories are the primary stat progression mechanism in RPGs and action games.

Hard dep on Inventory (items must exist before they can be equipped). Soft dep on Group system (faction stance bonus items possible but not in v1).

---

## Schema

### `ItemDef` additions (`schema/items.rs`)

```ron
"iron_sword": (
    display_name: "Iron Sword",
    icon: "icons/items/iron_sword",
    stackable: false,
    max_stack: 1,
    weight: 3.5,
    tags: ["weapon", "equippable"],
    equippable: true,                   // NEW
    slot: Some("main_hand"),            // NEW — slot key this item occupies
    two_handed: false,                  // NEW — if true, OffHand is cleared on equip
    stat_bonuses: [                     // NEW
        ( stat_key: "attack", kind: Additive(12.0) ),
        ( stat_key: "speed",  kind: Multiplicative(0.95) ),
    ],
    modifier_keys: [],                  // NEW — optional named modifiers from stats.ron
    model_attachment: None,             // NEW — placeholder, not yet implemented
),
```

```rust
// schema/items.rs — appended to ItemDef
#[serde(default)]
pub equippable: bool,

/// Slot key this item occupies when equipped. Required when equippable: true.
#[serde(default)]
pub slot: Option<String>,

/// When true, equipping this in the primary slot auto-unequips the off-hand slot.
#[serde(default)]
pub two_handed: bool,

/// Inline stat bonuses applied when equipped and reversed when unequipped.
#[serde(default)]
pub stat_bonuses: Vec<StatBonusDef>,

/// Named modifier keys from stats.ron applied additionally on equip.
#[serde(default)]
pub modifier_keys: Vec<String>,

/// Reserved for v2 visual mesh attachment. Parsed but not acted on.
#[serde(default)]
pub model_attachment: Option<AttachmentDef>,
```

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct StatBonusDef {
    /// Stat key to affect (e.g. "attack", "player.health", "{self}.speed").
    pub stat_key: String,
    pub kind: ModifierKind,  // reuse existing type from schema/stats.rs
}

#[derive(Deserialize, Debug, Clone)]
pub struct AttachmentDef {
    pub bone: String,            // GLB bone name (e.g. "RightHand")
    pub model: Option<String>,   // asset catalog model key; None = invisible in slot
}
```

### `PrefabDef` additions (`schema/catalog.rs`)

```ron
// prefabs/prefabs.ron
"player": (
    // ...
    equipment_slots: Some((
        slots: {
            "main_hand": ( display_name: "Main Hand", accepts_tags: ["weapon"] ),
            "off_hand":  ( display_name: "Off Hand",  accepts_tags: ["shield", "weapon"] ),
            "head":      ( display_name: "Head",      accepts_tags: ["armour"] ),
            "chest":     ( display_name: "Chest",     accepts_tags: ["armour"] ),
            "legs":      ( display_name: "Legs",      accepts_tags: ["armour"] ),
            "ring":      ( display_name: "Ring",      accepts_tags: ["accessory"] ),
        },
    )),
)

"orc_warrior": (
    // Only has one relevant slot
    equipment_slots: Some((
        slots: {
            "weapon": ( display_name: "Weapon", accepts_tags: ["weapon"] ),
        },
    )),
)
```

```rust
// schema/catalog.rs — in PrefabDef
#[serde(default)]
pub equipment_slots: Option<EquipmentSlotsDef>,
```

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct EquipmentSlotsDef {
    pub slots: HashMap<String, EquipmentSlotSpec>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct EquipmentSlotSpec {
    pub display_name: String,
    /// Item tags allowed in this slot. Empty = accept any equippable item.
    #[serde(default)]
    pub accepts_tags: Vec<String>,
}
```

---

## Runtime

### `EquipmentMap` component (`capabilities/equipment.rs`)

```rust
/// Maps slot_key → currently equipped item_key. One entry per occupied slot.
#[derive(Component, Default)]
pub struct EquipmentMap {
    pub equipped: HashMap<String, EquippedItem>,
}

pub struct EquippedItem {
    pub item_key: String,
    /// Stat deltas that were applied at equip time, for reversal on unequip.
    pub bonus_snapshots: Vec<(String, f32)>,  // (stat_key, additive_delta applied)
}
```

### `PlayerEquipment` resource

Like `PlayerInventory`, `EquipmentMap` on the player needs to survive scene transitions:

```rust
#[derive(Resource, Default)]
pub struct PlayerEquipment(pub EquipmentMap);
```

Scene loader copies `PlayerEquipment` → player entity `EquipmentMap` at spawn. Non-player entities have entity-attached `EquipmentMap` only.

### New actions (`schema/actions.rs`)

```ron
Equip(entity: "player", item_key: "iron_sword")      // resolves slot from ItemDef.slot
Unequip(entity: "player", slot: "main_hand")
UnequipAll("player")
```

```rust
Equip { entity: String, item_key: String },
Unequip { entity: String, slot: String },
UnequipAll(String),
```

### `Equip` executor logic

```rust
Action::Equip { entity, item_key } => {
    let item_def = item_catalog.get(&item_key)?;
    let slot = item_def.slot.as_deref()?;

    // Validate tag acceptance
    let slot_spec = equipment_slots_query.get(entity)?;
    // (validation: item tags must overlap accepts_tags or accepts_tags is empty)

    // Two-handed exclusion
    if item_def.two_handed {
        if let Some(off_hand_item) = equipment_map.equipped.remove("off_hand") {
            reverse_bonuses(entity, &off_hand_item, /* stats */);
        }
    }

    // Unequip existing item in this slot
    if let Some(prev) = equipment_map.equipped.remove(slot) {
        reverse_bonuses(entity, &prev, /* stats */);
        game_events.write(GameEvent::Trigger(format!("equipment.unequipped:{}:{}", slot, prev.item_key)));
    }

    // Apply new item bonuses, record snapshot
    let bonus_snapshots = apply_bonuses(&item_def.stat_bonuses, entity, /* stats */);
    equipment_map.equipped.insert(slot.to_string(), EquippedItem { item_key: item_key.clone(), bonus_snapshots });

    game_events.write(GameEvent::Trigger(format!("equipment.equipped:{}:{}", slot, item_key)));
}
```

### New pipeline events

```ron
equipment.equipped:{slot}:{item_key}     // item successfully equipped
equipment.unequipped:{slot}:{item_key}   // item removed from slot
equipment.swap:{slot}:{old}:{new}        // item replaced (equip into occupied slot)
equipment.rejected:{entity}:{item_key}  // tag mismatch or slot not available
```

### Conditions (for dialogue + quest use)

```rust
// DialogueCondition additions (patch to dialogue_system feature)
HasEquipped { entity: String, slot: String, item_key: String },
SlotEmpty { entity: String, slot: String },
SlotFilled { entity: String, slot: String },
```

---

## Worked example

```ron
// stats/stats.ron — player stats include attack
"attack": ( base: 5.0, min: 0.0, max: 999.0 ),

// Equip sword on game start
( on: "scene.ready:dungeon", do_actions: [
    AddItem(entity: "player", item_key: "iron_sword", count: 1),
    Equip(entity: "player", item_key: "iron_sword"),
] ),

// React to equip
( on: "equipment.equipped:main_hand:iron_sword", do_actions: [
    SetVariable("weapon_name", "Iron Sword"),
] ),
```

---

## New Rust changes

- `schema/items.rs` — add `equippable`, `slot`, `two_handed`, `stat_bonuses`, `model_attachment` to `ItemDef`; new `StatBonusDef`, `AttachmentDef` types.
- `schema/catalog.rs` — `equipment_slots: Option<EquipmentSlotsDef>` on `PrefabDef`; `EquipmentSlotsDef`, `EquipmentSlotSpec`.
- `capabilities/equipment.rs` (new file) — `EquipmentMap`, `EquippedItem`, `PlayerEquipment`, `apply_bonuses`, `reverse_bonuses`.
- `capabilities/mod.rs` — register module.
- `runtime/scene_manager/action_executor.rs` — `Equip`, `Unequip`, `UnequipAll` arms.
- `runtime/scene_manager/scene_loader.rs` — copy `PlayerEquipment` → entity `EquipmentMap` at player spawn; insert `EquipmentMap` on entities with `equipment_slots`.
- `runtime/scene_manager/mod.rs` — `PlayerEquipment` not cleared on `LoadScene`.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `StatBonusDef` + `AttachmentDef` in `schema/items.rs`; `equippable` + `slot` + `stat_bonuses` on `ItemDef`
- [ ] `EquipmentSlotsDef` + `EquipmentSlotSpec` in `schema/catalog.rs`; `equipment_slots` on `PrefabDef`
- [ ] `EquipmentMap` component + `PlayerEquipment` resource
- [ ] `apply_bonuses` + `reverse_bonuses` helpers (stat delta snapshot pattern)
- [ ] `Equip`, `Unequip`, `UnequipAll` actions + executor arms
- [ ] Two-handed exclusion in `Equip` arm
- [ ] Tag-acceptance validation in `Equip` arm (soft reject with warning + `equipment.rejected` event)
- [ ] Scene loader: `PlayerEquipment` copy; `EquipmentMap` on entities with `equipment_slots`
- [ ] `DialogueCondition::HasEquipped`, `SlotEmpty`, `SlotFilled` patch to dialogue system
- [ ] Pipeline events: `equipment.equipped`, `equipment.unequipped`, `equipment.swap`, `equipment.rejected`
- [ ] Equipment UI: `EquipmentPanel` UI node showing slot grid (character silhouette optional — v1 can be a simple labelled slot list)
- [ ] Demo: player equips a weapon in `3rd_person_game_demo`; attack stat increases; unequip reverses it
- [ ] Integration tests: equip applies stat bonus; unequip reverses it exactly; two-handed clears off-hand; tag rejection emits event; `PlayerEquipment` persists across scene load
- [ ] Docs: `equippable`, `slot`, `stat_bonuses`, `equipment_slots` in `docs/20_data_formats.md`

---

## Acceptance criteria

- Given `Equip(entity: "player", item_key: "iron_sword")`, the player's `attack` stat increases by the sword's bonus and `equipment.equipped:main_hand:iron_sword` is emitted.
- Given `Unequip(entity: "player", slot: "main_hand")`, `attack` returns to its pre-equip value and `equipment.unequipped:main_hand:iron_sword` is emitted.
- Given a two-handed weapon is equipped, the `off_hand` slot is cleared automatically.
- Given `Equip` with an item whose tags don't match the slot's `accepts_tags`, `equipment.rejected` is emitted and nothing changes.
- Given a scene transition, `PlayerEquipment` retains its equipped items.
- Given `Equip` into an already-occupied slot, the previous item is first unequipped (bonus reversed) and the new item is equipped.
