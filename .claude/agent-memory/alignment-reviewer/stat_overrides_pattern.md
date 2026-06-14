---
name: stat-overrides-flow
description: SceneEntityDef.stat_overrides per-instance stat base-value override — correctly wired into all three non-player spawn paths; StatMap-build logic is triplicated (refactor candidate)
metadata:
  type: project
---

`SceneEntityDef.stat_overrides: HashMap<String, f32>` (schema/scene_v2.rs ~259, `#[serde(default)]`) lets a designer override the initial `base` value of named stats from the prefab's `stat_templates` (PrefabDef.stat_templates, catalog.rs ~794). Only `base` changes; `min`/`max`/`regen`/`thresholds` come from the template. Unknown keys and exceeds-max emit `warn!` at load time (good designer feedback).

**Reachable from all three non-player spawn paths (verified ~2026-06-14):**
- GLB actor/prop: `spawn_prefab_instance` takes `stat_overrides: &HashMap<String, f32>` and builds StatMap (entity_spawner.rs ~128-145). Scene caller passes `&entity_def.stat_overrides` (scene_loader.rs ~705).
- Composite primitive (model:""+children): inline StatMap build reading `entity_def.stat_overrides` (scene_loader.rs ~370-390).
- Single-mesh primitive: inline StatMap build reading `entity_def.stat_overrides` (scene_loader.rs ~621-641).

This is the one feature in the [[prefab-marker-three-spawn-paths]] danger zone that gets all three paths right. Use it as the positive reference when reviewing new per-entity spawn fields.

**Dynamic/nested spawns correctly pass empty overrides** (`&Default::default()`): `Action::Spawn` (entity_spawner.rs ~257), foliage trunk (scene_loader.rs ~212), nested prefab reference (scene_loader.rs ~1906). Correct semantics — those callers have no `SceneEntityDef` to source overrides from.

**Refactor candidate (warning, not blocker):** the StatMap-build + warn-checks are triplicated across the three paths. A future per-instance stat feature (e.g. max override) must be added in all three or content silently diverges by prefab kind. Suggested: extract `build_stat_map(prefab, overrides, name)`.
