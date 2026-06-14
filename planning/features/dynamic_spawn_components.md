# Dynamic Spawn — Missing Components (motion, stat_label, world_stat_bar)

Planned at: df8c94b (2026-06-14)

## Problem

`spawn_prefab_instance` (called by `drain_spawn_queue_system` for `Action::Spawn`) handles
behavior, stat_templates, interactable, and trigger_zone correctly, but silently skips three
prefab-derived components that scene-placed entities receive:

| Component | Effect when missing |
|---|---|
| `motion` | rotate / bob animation never starts on dynamically spawned entities |
| `stat_label` | no floating world-space health label above entity |
| `world_stat_bar` | no GLB health bar above entity |

A scene-placed enemy gets all three. A rule-spawned `Action::Spawn` enemy gets none — no error,
no warning.

## Fix Strategy

### `motion`

`MotionDef` is entirely prefab-derived (no per-entity-def field). Move the motion `insert` from the
primitive and GLB branches in `scene_loader.rs` into `spawn_prefab_instance`. The call sites in
scene_loader that already call `spawn_prefab_instance` can then drop their own motion inserts.

### `stat_label` and `world_stat_bar`

These currently work by pushing spawn descriptors onto `StatLabelSpawnQueue` /
`WorldStatBarSpawnQueue` `Vec`s in `scene_loader.rs`, which are drained at end of the scene load
frame. Dynamic spawns don't participate in that frame-end drain, so they're silently skipped.

**Chosen approach — `Added<StatMap>` reactive system:**

Add a new system `attach_dynamic_stat_ui_system` that runs after `drain_spawn_queue_system` and
queries `(Entity, &PrefabKey, Added<StatMap>)`. For each newly-added `StatMap` it:
1. Looks up the `PrefabDef` from `LoadedPrefabCatalog`
2. If the prefab has `stat_label`, pushes to `StatLabelSpawnQueue`
3. If the prefab has `world_stat_bar`, pushes to `WorldStatBarSpawnQueue`

This is identical to what `scene_loader.rs` does for placed entities — same queues, same drain
path — so label/bar rendering is pixel-for-pixel identical regardless of how the entity was
spawned.

**Why not push directly from `spawn_prefab_instance`?**

`spawn_prefab_instance` is a `Commands`-based function, not a system. It can insert components but
cannot access resources like `StatLabelSpawnQueue` without threading mutable references through
every caller. The reactive system approach keeps `spawn_prefab_instance` free of system resource
dependencies.

## Files to Touch

| File | Change |
|---|---|
| `entity_spawner.rs` | move `motion` insert here from scene_loader branches |
| `scene_loader.rs` | remove motion inserts from GLB and primitive paths; they're now in helper |
| new system `attach_dynamic_stat_ui_system` | react to `Added<StatMap>` + `PrefabKey`, push to queues |
| `runtime/mod.rs` or wherever systems are registered | add new system after `drain_spawn_queue_system` |

## Schema / RON Impact

None — no new fields, no version bump.

## Acceptance

- A dynamically spawned `enemy_snake` via `Action::Spawn` shows a health bar, stat label, and
  patrol motion identical to a scene-placed instance
- Integration test: spawn entity with `motion_def` via `Action::Spawn`, assert `Motion` component
  is present
- Integration test: spawn entity with `stat_templates` + `world_stat_bar` via `Action::Spawn`,
  assert world stat bar appears (entity count in stat bar system > 0)
- Existing scene-placed stat label and world stat bar tests still pass
