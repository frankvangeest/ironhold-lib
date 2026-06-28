---
name: prefab-marker-three-spawn-paths
description: Any PrefabDef field that inserts a marker/component at spawn must be wired into ALL THREE spawn paths in scene_loader.rs, not just spawn_prefab_instance — GLB-only wiring silently breaks primitive/composite prefabs
metadata:
  type: project
---

**UPDATE (spawn-site consolidation, ~2026-06-08):** the *standard* per-entity metadata —
`SpawnId` + `PrefabKey` + `LevelEntity` + `SpawnRegistry` registration + the `ClickSelectable`/
`Targetable` markers — is now centralized in **`tag_spawned_entity(ec, registry, id, prefab_key,
click_selectable, targetable)`** in `runtime/scene_manager/mod.rs`. All 7 spawn sites route through
it (GLB actor/prop, single-mesh primitive, composite primitive, foliage root, primitive player, GLB
player, dynamic `Action::Spawn` in `drain_spawn_queue_system`). `spawn_prefab_instance` no longer
inserts those itself. So for the *targeting markers + SpawnId/PrefabKey/LevelEntity* class of field,
the multi-path footgun is closed: add the field to the helper once. When reviewing a change to that
helper, verify every call site still passes the right flags (players pass `false,false` for markers
deliberately — selecting the player is nonsensical).

**UPDATE 2 (capability-feature consolidation, ~2026-06-28):** the SIX features `behavior`,
`interactable`, `dialogue`, `inventory`, `stat_templates`, `trigger_zone` are now in a SINGLE helper
**`attach_prefab_features(commands, entity, prefab, project_root, asset_server, entity_id,
stat_overrides, prefab_key)`** — now defined `pub(super)` in **`entity_spawner.rs`** (top of file,
just above `spawn_prefab_instance`), NOT in scene_loader.rs. `scene_loader.rs` imports it from
`super::entity_spawner`. ALL THREE spawn paths route through this ONE function: GLB path calls it at
the TAIL of `spawn_prefab_instance` (after animation_policy/colliders/motion/npc, passing
`spawned.parent` and `name` for both entity_id and prefab_key); composite primitive branch
(scene_loader ~351, passes `parent`); single-mesh primitive branch (scene_loader ~570, passes
`spawned`). So the THREE-copies risk for these six features is now CLOSED — there is genuinely one
source of truth (verified 2026-06-28 review of the consolidation refactor). A new feature added to
`attach_prefab_features` propagates to all three paths automatically. The 3 removed imports
(`TriggerZone`, `TriggerZoneId`, `Interactable`, `LiveStat`, `StatMap`) are gone from scene_loader.rs
with no residual refs. Note `motion`, `colliders`, `npc`, `Collectable`, material override,
`stat_label`/`world_stat_bar`, `nameplate` are STILL inserted inline per-path — those keep the
multi-path footgun. Trigger-zone block is LAST in the helper (spawns a sensor child; needs a fresh
Commands borrow, comment in code explains it). All call sites pass `&mut commands` not a held
`EntityCommands`, so no borrow conflict.

BUT the footgun below STILL applies to **every other** spawn-time field that is NOT part of the
standard metadata set and NOT in `attach_prefab_features` — `motion`, `npc`, `colliders`,
`Collectable`, material override, `stat_label`/`world_stat_bar`, `nameplate`, etc. Those are still
inserted per-path. The GLB insertion logic lives in `entity_spawner.rs::spawn_prefab_instance`
— but that function is **only called for GLB prefabs** (`kind: Actor` / `kind: Prop`) and nested-prefab
references.

Two other spawn paths in `runtime/scene_manager/scene_loader.rs` manually re-implement component insertion and DO NOT call `spawn_prefab_instance`:

1. **Composite primitive** (`kind: Primitive`, `model: ""`, non-empty `children`) — branch around scene_loader.rs:241.
2. **Single-mesh primitive** (`kind: Primitive`, has `shape`, no children) — branch around scene_loader.rs:410.

Plus a fourth, the **primitive player** branch (~line 692) which spawns its own entity.

**Designer-reachability test for any new spawn-time marker:** set the new field on a `kind: Primitive` prefab (a bare cube) and a composite prefab, not just a GLB. If the marker only attaches for GLB kinds, a designer using primitives gets silent no-op — no parse error, no warning, nothing.

This is the same footgun documented for TriggerZone (see auto-memory "TriggerZone missing from composite path"). It recurs because the three branches duplicate insertion logic instead of sharing a helper. When reviewing a new PrefabDef marker field, grep the field name in scene_loader.rs — expect to find it (or a shared helper call) in the composite branch AND the single-mesh branch, not only entity_spawner.rs.

Observed concrete miss (targeting feature, 2026-06): `click_selectable`/`targetable` insert `Pickable`/`ClickSelectable`/`Targetable` at entity_spawner.rs:100-108 only. Primitive and composite prefabs with these fields set get nothing. Demo masked the gap because `enemy_orc_melee` is `kind: Actor`.

Suggested durable fix worth recommending: extract an `insert_prefab_markers(ec, prefab, ...)` helper called from all three/four branches so future PrefabDef fields can't be wired into only one path.
