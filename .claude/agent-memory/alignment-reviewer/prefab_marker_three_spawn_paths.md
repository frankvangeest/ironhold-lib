---
name: prefab-marker-three-spawn-paths
description: Any PrefabDef field that inserts a marker/component at spawn must be wired into ALL THREE spawn paths in scene_loader.rs, not just spawn_prefab_instance — GLB-only wiring silently breaks primitive/composite prefabs
metadata:
  type: project
---

When a new `PrefabDef` field (e.g. `click_selectable`, `targetable`, `trigger_zone`, `interactable`, `motion`, `behavior`, `stat_templates`) causes a component to be inserted at spawn time, the insertion logic lives in `entity_spawner.rs::spawn_prefab_instance` — but that function is **only called for GLB prefabs** (`kind: Actor` / `kind: Prop`) and nested-prefab references.

Two other spawn paths in `runtime/scene_manager/scene_loader.rs` manually re-implement component insertion and DO NOT call `spawn_prefab_instance`:

1. **Composite primitive** (`kind: Primitive`, `model: ""`, non-empty `children`) — branch around scene_loader.rs:241.
2. **Single-mesh primitive** (`kind: Primitive`, has `shape`, no children) — branch around scene_loader.rs:410.

Plus a fourth, the **primitive player** branch (~line 692) which spawns its own entity.

**Designer-reachability test for any new spawn-time marker:** set the new field on a `kind: Primitive` prefab (a bare cube) and a composite prefab, not just a GLB. If the marker only attaches for GLB kinds, a designer using primitives gets silent no-op — no parse error, no warning, nothing.

This is the same footgun documented for TriggerZone (see auto-memory "TriggerZone missing from composite path"). It recurs because the three branches duplicate insertion logic instead of sharing a helper. When reviewing a new PrefabDef marker field, grep the field name in scene_loader.rs — expect to find it (or a shared helper call) in the composite branch AND the single-mesh branch, not only entity_spawner.rs.

Observed concrete miss (targeting feature, 2026-06): `click_selectable`/`targetable` insert `Pickable`/`ClickSelectable`/`Targetable` at entity_spawner.rs:100-108 only. Primitive and composite prefabs with these fields set get nothing. Demo masked the gap because `enemy_orc_melee` is `kind: Actor`.

Suggested durable fix worth recommending: extract an `insert_prefab_markers(ec, prefab, ...)` helper called from all three/four branches so future PrefabDef fields can't be wired into only one path.
