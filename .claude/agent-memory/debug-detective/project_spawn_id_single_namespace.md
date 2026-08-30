---
name: project-spawn-id-single-namespace
description: Scene-authored entity ids and counter-derived spawn ids share ONE SpawnRegistry namespace with no cross-check; collisions silently orphan the older entity, and the counter resets to 0 per scene
metadata:
  type: project
---

`SpawnRegistry.entities: BTreeMap<String, Entity>` is a **single flat namespace** shared by
scene-placed entities and every dynamic `Action::Spawn`. All of them register through the one
helper `tag_spawned_entity` (`runtime/scene_manager/mod.rs`) — 4 call sites in `scene_loader.rs`,
2 in `entity_spawner.rs`.

The three facts that make this a live hazard:
- `SpawnRegistry.counter` is reset to `0` on every non-overlay `LoadScene` (`scene_loader.rs`), so
  every counter-derived id restarts at 1, 2, 3… each scene.
- `scene_v2.rs` validates "Duplicate scene entity id" **only within one scene's `entities:` list** —
  it has zero visibility into ids generated at runtime.
- On collision, `BTreeMap::insert` silently overwrites: the older entity stays alive in the world
  but becomes unreachable via `Despawn`/`OpenContainer`/registry lookup. This is *already*
  characterised by `test_spawn_id_collision_orphans_old_entity` in `spawn_tests.rs` — an accepted,
  tested behavior, not a panic.

**Why:** the `id: None` fallback format `"{prefab}_{n}"` was accidentally safe (a designer rarely
names a static entity after a prefab key). Anything that lets a designer choose the *prefix* of a
counter-derived id — the `{new_id}` token being the first — removes that accident: short prefixes
like `crate_`, `loot_`, `chest_` plus a small integer are exactly the shape scene authors use for
static ids.

**How to apply:** whenever reviewing a feature that generates or templates a spawn id, do not
accept "guaranteed unique" claims about `SpawnRegistry.counter` — it only guarantees uniqueness
*among counter-derived ids*, never against authored ones. The cheap mitigation is a `warn!` when
`registry.entities` already contains the resolved id (nothing warns today), or namespacing the
generated portion so it can't be typed by hand.

Related: [[project_action_ron_typos_are_silent]].
