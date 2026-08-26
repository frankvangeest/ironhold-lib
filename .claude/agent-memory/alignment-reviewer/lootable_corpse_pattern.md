---
name: lootable-corpse-pattern
description: Monster corpse-loot v2 (separate corpse entity via Action::Spawn.at_entity) — the generic at_entity engine field, the 5-file RON replication recipe, and the traps a designer hits on a 4th monster type
metadata:
  type: project
---

**v2 (2026-08-26) fully replaced v1.** v1's same-entity model (monster IS the container, revived
with `ResetToSpawn`) no longer exists on disk — its RON was rewritten. Kept below only where a v1
finding still generalizes. v2: monster despawns on death and is replaced by a *separate disposable
corpse entity* spawned at the same transform, with an independent fixed respawn timer.

## The engine field: `Action::Spawn.at_entity: Option<String>`

`schema/actions.rs` — genuinely generic, not corpse-specific. Reusable for any "spawn X where
live entity Y is". Reference this as the model for a well-shaped new Action field:
- `#[serde(default)] Option<String>` → every existing RON keeps parsing.
- Resolved in `action_executor.rs`'s Spawn arm via `registry.entities.get(id)` →
  `GlobalTransform::compute_transform()` (same `SpawnRegistry` route as `SpawnEffect.entity`).
- **Warn-and-skip when unresolvable with no `position`/`spawn_point` fallback** — deliberately
  does NOT fall back to the origin (unlike a missing `spawn_point`, which silently uses origin).
  Copy this choice for anything gameplay-important.
- Substituted in BOTH `rewrite_self` and `rewrite_target` (message_interpreter.rs) and gated in
  `action_bar.rs::action_needs_target`. CLI matches `Action::Spawn { .. }` → no exhaustiveness break.

Known limits of `at_entity` (all confirmed, all non-blocking, all worth re-flagging if extended):
- Copies the **whole** transform incl. scale + pitch/roll, not just "position + facing" as the
  docstring says. Harmless at scale 1; a scaled/tilted source silently propagates.
- **No offset knob.** `position` is ignored entirely once `at_entity` resolves, and
  `overrides/model_fixes.ron` is keyed by GLB path — which the corpse prefab *shares* with the live
  monster — so a corpse-only Y correction is NOT authorable in RON. Any creature whose root isn't
  at ground level will float/sink with no RON fix. Suggested: `at_entity_offset`, or treat
  `position` as a delta when `at_entity` is set.
- `action_needs_target` uses `at_entity.as_deref() == Some("{target}")` (exact match, copied from
  SpawnEffect) while `Despawn`/`PlayAnimationOn` use `.contains()`; `Spawn.id`/`spawn_point` aren't
  checked at all despite being `{target}`-substituted.
- **`dialogue.rs::substitute_self_in_action` has no `Action::Spawn` arm at all** — a dialogue
  choice's `Spawn(..., at_entity: "{self}")` keeps the literal `{self}` and dies with a confusing
  "at_entity {self} not found in registry" warn. 4th substitution site; always check it.

## The v2 RON recipe (5 files, zero Rust) for a new monster type

1. `prefabs.ron`: `enemy_X` (behavior + `stat_templates` w/ `stat.{self}.health.depleted`
   threshold) **and** `X_corpse` (`kind: Prop`, same `model:`, `interactable:`, `inventory:`, no
   colliders, no trigger_zone, no stat_templates, `nameplate: false`).
2. `behaviors/enemy_X.behavior.ron`: single `dead` state; entry arms
   `X.swap_to_corpse:{self}` (= death-clip length) + `X.respawn:{self}` (fixed);
   `on: swap_to_corpse → [Despawn("{self}_corpse"), Spawn(prefab:"X_corpse", id:"{self}_corpse",
   at_entity:"{self}"), Despawn("{self}")]`.
3. `behaviors/lootable_corpse.behavior.ron` — **shared, fully `{self}`-relative, reuse unchanged**
   across all corpse prefabs. Its `fresh` entry_actions fire on dynamic spawn because
   `resolve_pending_behaviors_system` (entity_spawner.rs ~494) runs initial-state entry_actions.
4. `scenes/main.scene.ron`: the entity + a `spawn_points` entry hand-duplicating its
   `transform.translation`.
5. `logic/state_machine.ron`: one respawn rule **per instance**, literal id, + `PreloadPrefab`.

## v2 replication traps (check every one)

1. **Respawn rules in a state's `on:` are dropped if the FSM is in another state when the timer
   fires.** 3rd_person_game_demo puts all six in `playing`'s `on:` — but `tick_delayed_events_system`
   (lib.rs ~691) uses raw `Time` with no pause gate and the `paused` state is just an overlay, so
   pausing across the 60 s window loses that monster permanently (until scene reload). Fix is RON:
   put cross-state respawn rules in `global_on:`.
2. **No wildcard in rule events** → respawn authoring is O(instances): 6 monsters = 6 near-identical
   rules + 6 spawn_points. Doesn't scale to a real level.
3. **`spawn_point` names are NOT CLI-validated** (grep: `spawn_points` appears nowhere in
   `ironhold_cli/src`). A typo warns at runtime and spawns the monster at the world **origin**.
   Also nothing keeps `spawn_points` in sync with the entity's authored `translation`/`yaw`.
4. **Stale delayed events still bite, because the corpse id is reused.** The plan claims decay
   staleness is a no-op ("entity is gone"); it isn't — `{self}_corpse` is the same literal id every
   generation, so generation N's 300 s `corpse.decay:{id}` fires against generation N+k's live
   `fresh` corpse and despawns it early. Same root cause as the logged "corpse-id reuse" icebox
   item; needs a unique-id or cancellable-timer primitive.
5. **Death-anim delay is a hand-measured magic number** (3.0/2.5/2.25 s). There is **no**
   `animation.finished` event anywhere in the engine — verified. Wrong number = corpse pops early
   or body T-poses.
6. `drain_dynamic_stat_ui_system` (scene_loader.rs ~2772) has **no liveness check** on
   `entry.entity` — spawn+despawn of a widget-carrying prefab inside one frame re-creates the
   orphaned-widget warn-spam that `stat_widget_cleanup_system` was added to fix.

## Still-true v1 findings

- `attach_prefab_features` (entity_spawner.rs) is the single source of truth for
  behavior/interactable/inventory/trigger_zone/stat_templates across all spawn paths — the old
  [[prefab-marker-three-spawn-paths]] footgun does not apply here.
- `interactable_system` emits `GameEvent` only, never touches ActionQueue. Correct capability shape.
  `stat_widget_cleanup_system` likewise (pure `RemovedComponents` → `try_despawn`, no ActionQueue).
- `interactable_system` fires for EVERY in-radius interactable, not the nearest → two corpses close
  together both `OpenContainer`; FIFO means the last wins. Corpses land wherever the fight ended.
- `TakeAllFromContainer` early-returns on an empty container, so `container.looted` never fires for
  a loot-less corpse → it can only ever hit the ambient decay path.
- `CloseContainer` takes no id (singleton panel) — any handler closes whatever is open.
- `trigger_zone` on a Dynamic-body NPC has no `ColliderMassProperties` override (~146× mass, real
  engine bug in backlog) — both v1 and v2 avoid `trigger_zone` on monsters/corpses for this reason.
