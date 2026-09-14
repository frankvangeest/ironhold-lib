---
name: project-player-spawn-skips-attach-prefab-features
description: Player-tagged prefabs never call attach_prefab_features, so inventory/interactable/dialogue/behavior/trigger_zone are silently dropped for them - only stat_templates is shared
metadata:
  type: project
---

A `tags: ["player"]` prefab silently loses five whole `PrefabDef` blocks: `inventory`,
`interactable`, `dialogue`, `behavior`, and `trigger_zone`. `attach_prefab_features`
(`entity_spawner.rs`) has exactly **3** call sites — two in `scene_loader.rs` (composite +
single-mesh branches) and one inside `spawn_prefab_instance` — and **none** of them is on a
player path. `drain_spawn_queue_system`'s player branch `continue`s before ever reaching
`spawn_prefab_instance`, and `spawn_player_entity_core` shares only
`build_stat_map_from_templates` with `attach_prefab_features`, not the function itself.

Consequence: `PlayerInventory` (the resource) is never seeded from a prefab's
`inventory.initial_items` — only `Action::AddItem`/`BuyItem` ever fill it. No warn fires.

**Why:** found while reviewing `feature/inventory_max_stack_fix` (2026-09-12), which threaded the
real `ItemCatalog` into `attach_prefab_features`; the fix is correct but cannot reach players at
all, so "player inventory ignores max_stack" would be a *different* bug with a different cause.

**How to apply:** any bug of the shape "designer set `<field>` on a prefab and nothing happened"
— check whether the prefab is player-tagged before looking anywhere else. When adding a new
`PrefabDef` block, decide explicitly whether `spawn_player_entity_core` needs it too; the
stat_templates split is the precedent for "shared helper, two call sites", not "players get it
for free". Related: [[project-assemble-player-config-primitive-panic]] (the other
player-spawn-path divergence).
