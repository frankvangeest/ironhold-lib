---
name: world-label-stat-ui-pattern
description: How stat_label/world_stat_bar (and damage popups) reach scene-placed vs dynamically-spawned entities, and why depth_scale:None on dynamic spawns is an accepted limitation not a misalignment
metadata:
  type: project
---

`PrefabDef.stat_label` and `PrefabDef.world_stat_bar` are designer-authored RON fields that
spawn `WorldLabel`-tracked Text2d / Mesh2d widgets following an entity. Two spawn routes exist
and they are DIFFERENT code (do not assume one covers the other):

1. **Scene-placed entities** — `spawn_scene_v2` collects `pending_stat_labels` /
   `pending_world_bars` and spawns the widgets inline (scene_loader.rs ~line 1040+). These DO
   resolve depth scaling via `resolve_label_depth_scale(scene.label_depth_scale, per_label)`.

2. **Dynamic `Action::Spawn` entities** — `drain_spawn_queue_system` (entity_spawner.rs) pushes a
   `DynamicStatUiEntry` (entity + pre-resolved stat keys) onto `DynamicStatUiQueue`
   (mod.rs). `drain_dynamic_stat_ui_system` (scene_loader.rs ~line 1760) drains it next frame and
   spawns the same widget set. Registered in lib.rs after `drain_spawn_queue_system` in the
   chained Update set. `Action::Spawn` is the SOLE dynamic-spawn entry (always queues to
   `PendingEntitySpawns`, drained only by `drain_spawn_queue_system`), so this covers every
   dynamically-spawned prefab regardless of kind (GLB/primitive/composite).

**`depth_scale: None` on dynamic spawns is ACCEPTED, not a misalignment.** Dynamic spawns have no
scene context to read `scene.label_depth_scale` from at drain time. This matches the existing
precedent: transient `ShowDamagePopup` / `ShowFloatingText` widgets in action_executor.rs also use
`depth_scale: None` deliberately. The per-label `depth_scale: Some(true/false)` override on the
prefab's StatLabelDef/WorldStatBarDef is currently ignored on the dynamic path — a corner-case
gap worth noting but low-impact (a designer overriding depth_scale on a *dynamically-spawned*
entity is rare). If a future change needs it, the fix is to carry the resolved `Option<(f32,f32)>`
into `DynamicStatUiEntry` from the active scene's `label_depth_scale`.

**Known duplication (refactor candidate):** the Ascii/Pixel widget-spawn match block is now
triplicated — inline scene path, `drain_dynamic_stat_ui_system`, and the world_stat_bar section.
Same shape as the StatMap-build triplication noted in [[stat_overrides_pattern]]. If reviewing a
change that adds a new WorldStatBarStyle variant or a new widget knob, expect to touch all copies;
flag the divergence risk and suggest extracting a `spawn_world_stat_bar(commands, mats, tracked,
stat_key, wb, depth_scale)` helper.

Motion has the parallel structure: see [[prefab_marker_three_spawn_paths]] — `motion` is inserted
in `spawn_prefab_instance` (covers GLB actors + all dynamic spawns) AND separately in the
single-mesh (scene_loader.rs ~401) and composite (~521) primitive branches that don't call
`spawn_prefab_instance`. Removing the GLB-actor inline motion block while keeping the two primitive
blocks is CORRECT — it dedupes the GLB path without breaking primitives.
