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

**The 6-step "Adding a corpse for a new monster type" checklist NOW EXISTS** in docs/30 (~636-656)
— that gap is closed. Its step 3 still said "Despawn → Spawn → Despawn trio" after the `{new_id}`
retrofit removed the guard Despawn (now just Spawn → Despawn); re-check that wording.

**`{new_id}` retrofit (2026-08-30, uncommitted on `integration`):** corpse ids became
`"{self}_corpse_{new_id}"` and the `Despawn("{self}_corpse")` guard was dropped from all three
monster behaviors — multiple corpses from the same slot now coexist for up to 300s. Recurring
review trap: the "Key notes" bullets at the END of docs/30's corpse section (~730-751) still
described the old guard/id-reuse residual and cited a backlog Icebox item this change resolves.
When any corpse mechanic changes, read the section's tail bullets, not just the prose above them.

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
