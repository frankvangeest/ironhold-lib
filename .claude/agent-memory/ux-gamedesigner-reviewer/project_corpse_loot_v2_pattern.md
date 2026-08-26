---
name: corpse-loot-v2-pattern
description: v2 separate-corpse-entity loot pattern (Action::Spawn.at_entity + per-instance global respawn rules) — docs/30's "Lootable corpse" section still describes the deleted v1 same-entity design
metadata:
  type: project
---

Shipped 2026-08-26 on `feature/monster-corpse-loot-v1` (v2 rewrite; v1's RON no longer exists).

**Canonical files:** `3rd_person_game_demo/behaviors/lootable_corpse.behavior.ron` (one shared,
fully `{self}`-relative corpse behavior reused by all three `*_corpse` prefabs),
`enemy_{zombie,snake,spider}.behavior.ron` (single `dead` state: arm respawn timer, then
`Despawn("{self}_corpse")` → `Spawn(at_entity: "{self}")` → `Despawn("{self}")`),
`logic/state_machine.ron` `"playing"` state (six literal-id `{type}.respawn:{id}` global rules),
`scenes/main.scene.ron` `spawn_points:` (six hand-copied per-instance points).

**Recurring doc gap to check on any corpse/loot/respawn work:** `docs/30_runtime_events_and_logic.md`
§"Lootable corpse (loot-on-death)" (~lines 506-631) is entirely v1 — `dead_full`/`dead_looted`,
`interactable:`/`inventory:` on the monster prefab, `RemoveItem`+`AddItem` re-seeding,
`entity.respawned` hide+restore. All of that RON was deleted. `at_entity` appears ONLY in
docs/20's action table row (~3677); zero docs/30 coverage, zero designer-facing coverage of the
global-respawn half.

**Undocumented engine rule that this pattern depends on:** a delayed event outlives the entity
that armed it, so `entity_fsm_interpreter_system` has no live entity to match a per-entity `on:`
handler against — any post-despawn timer MUST be caught by a global rule in `state_machine.ron`
keyed by literal id. Not stated anywhere in `docs/30` (its `EmitEventAfterDelay` entries only
mention "cleared on LoadScene").

**Footguns when replicating on a new monster:** (1) event prefix is per-type
(`snake.respawn:`), so a copied behavior file with a stale prefix silently never respawns — no
warning; (2) `spawn_point` coords + `yaw_deg` are hand-copied from the scene entity's own
`transform`, and silently drift if the monster is moved; (3) forgetting the corpse's
`PreloadPrefab` costs a WASM stall on first death; (4) `{self}_corpse` is a reused literal id, so
a second death cuts short a still-unlooted earlier corpse (accepted, Icebox).

**How to apply:** treat the docs/30 section as stale until rewritten; on any new monster/corpse
authoring request, walk the five-artifact checklist (corpse prefab, shared corpse behavior,
monster behavior, spawn_point, global rule, PreloadPrefab). Related:
[[container-events-undocumented]], [[inventory-item-system]].
