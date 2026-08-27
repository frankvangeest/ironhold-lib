---
name: corpse-loot-v2-pattern
description: v2 separate-corpse-entity loot pattern; docs/30's Lootable corpse section is now REWRITTEN to v2 (no longer stale) but still has no "adding a 4th monster" checklist
metadata:
  type: project
---

Shipped 2026-08-26 on `feature/monster-corpse-loot-v1`; corpse death-pose added on
`feature/dynamic-animation-control` the same day.

**Canonical files:** `3rd_person_game_demo/behaviors/lootable_corpse.behavior.ron` (one shared,
fully `{self}`-relative corpse behavior reused by all three `*_corpse` prefabs — sets the death
pose in its `fresh` entry_actions and arms decay via `SetDespawnTimer`),
`prefabs/animation/corpse_policy_{zombie,snake,spider}.ron`,
`enemy_{zombie,snake,spider}.behavior.ron`, `logic/state_machine.ron` (`global_on:` respawn rules
keyed `monster.respawn:{literal_id}`), `scenes/main.scene.ron` `spawn_points:`.

**docs/30 section is NO LONGER STALE.** `docs/30_runtime_events_and_logic.md`
§"Lootable corpse (loot-on-death)" (~lines 506-710) was fully rewritten to v2 and is now genuinely
good: at_entity rationale, the id-reuse guard, why `SetDespawnTimer` not `EmitEventAfterDelay`, why
`global_on` not state-scoped, the corpse-specific `animation_policy` rationale, and the
panels_open/target-clear notes. Do not re-flag it as v1.

**Remaining designer-facing gap:** there is **no single "adding a 4th monster type" checklist**.
The artifacts are scattered across the section as prose asides. The full list is now SEVEN items:
(1) `{monster}_corpse` prefab, (2) a NEW `corpse_policy_{monster}.ron`, (3) `animation_policy:`
pointing at it, (4) the monster behavior's `swap_to_corpse` Despawn→Spawn→Despawn trio,
(5) a `spawn_point`, (6) one `global_on` `monster.respawn:{id}` rule, (7) `PreloadPrefab` for the
corpse. Only (4)/(6) and "the shared behavior needs no changes" are called out explicitly.

**Corpse-policy footgun:** `corpse_policy_zombie.ron` carries
`animation_sources: ["anim_zombie","anim_locomotion","anim_hit_death"]` but the snake and spider
ones carry NO `animation_sources` at all (their Death clip is embedded in the model GLB) — and
both say "See corpse_policy_zombie.ron for the full rationale" without explaining the difference.
A designer copying the zombie file for a 4th monster inherits three irrelevant zombie GLB sources.

**Undocumented engine rule the pattern depends on:** a delayed event outlives the entity that
armed it, so any post-despawn timer must be caught by a `global_on` rule keyed by literal id.
Also: `action_executor_system` runs before `drain_spawn_queue_system`, so a `PlayAnimationOn`
fired alongside the `Spawn` that creates its target silently drops — the corpse's pose is
therefore set from the CORPSE's own `fresh` entry_actions, not the monster's death sequence.

**How to apply:** on any new monster/corpse authoring request, walk the seven-artifact checklist
above. Related: [[animation-policy-doc-gaps]], [[dynamic-animation-control-demo]],
[[container-events-undocumented]], [[inventory-item-system]].
