---
name: action-spawn-rejects-primitive-prefabs
description: Action::Spawn of any kind: Primitive prefab (model "") is skipped with a warn — only scene-load spawns primitives; validate does not flag it
metadata:
  type: project
---

`Action::Spawn` (action_executor.rs) looks up `asset_catalog.models.get(&prefab_def.model)` and `continue`s with
`warn!("model key \"\" not found")` for every `kind: Primitive` prefab; `drain_spawn_queue_system` only has the GLB
`spawn_prefab_instance` path anyway. Primitive meshes/colliders are built only in scene_loader's scene-load branches.
`ironhold validate` has no check for it, so a RON "respawn" rule targeting a primitive prefab passes validate and
silently never spawns (found in first_person_starter v2 review, 2026-10-22: training_dummy respawn rules).

**Why:** a designer-facing dead end with zero diagnostics beyond a runtime console line.
**How to apply:** when reviewing any RON that `Spawn`s a prefab, check the prefab's `kind`; when a fix lands,
re-verify with grep for `models.get(&prefab_def.model)` in action_executor.rs. Related: [[selectaimheight-only-with-clickselectable]].
