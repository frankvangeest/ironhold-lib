# Feature: Unify prefab feature application across all spawn paths

_Status: Ready_
_Planned at: `10a7d47` (2026-06-28)_

## What

After `attach_prefab_features` was extracted to eliminate the composite-vs-single-mesh divergence
for Primitive prefabs, one gap remains: `spawn_prefab_instance` in `entity_spawner.rs` (the GLB
Actor/Prop path) still applies the same six capability features independently. Any new `PrefabDef`
field must be wired into **both** `attach_prefab_features` and `spawn_prefab_instance` or the
two paths silently diverge — the exact bug class the Queued consolidation item was meant to kill.

This feature has `spawn_prefab_instance` call `attach_prefab_features` at its tail, then removes
the now-redundant blocks from entity_spawner.rs. After this lands, a new PrefabDef capability
field only needs to be wired in one place.

## Why

The two-list maintenance burden is a recurring footgun: the `TriggerZone`-missing-from-composite
bug and the `stat_templates`-missing-from-dynamic-spawn bug were both caused by the same
pattern. `attach_prefab_features` closed the Primitive gap; this closes the GLB gap.

## Approach

`spawn_prefab_instance` currently applies the six features via an `ec = commands.entity(parent)`
chain (with trigger_zone and stat_templates inserted after `ec` is released). The plan:

1. Remove the following blocks from `spawn_prefab_instance` in `entity_spawner.rs`:
   - `behavior` (lines ~59–64, currently inside the `ec` chain)
   - `interactable` (lines ~66–71, `ec` chain)
   - `dialogue` (lines ~73–75, `ec` chain)
   - `inventory` (lines ~77–86, `ec` chain)
   - `trigger_zone` (lines ~120–131, after `ec`)
   - `stat_templates` (lines ~155–182, after `ec`)

2. Add a call to `attach_prefab_features` at the tail of `spawn_prefab_instance` (after all
   existing inserts — animation policy, trigger_zone, colliders, stat_templates, motion, NPC
   agent), using `name` for both `entity_id` and `prefab_key`, and `stat_overrides` from the
   existing param.

3. Move `attach_prefab_features` from `scene_loader.rs` to `entity_spawner.rs` (or a shared
   location such as `mod.rs`) so `spawn_prefab_instance` can call it. Alternatively, keep it
   in `scene_loader.rs` and re-export, but a `pub(super)` in `mod.rs` is cleaner.

**Order note:** The six features are independent of animation policy, NPC agent, and colliders —
component insertion order does not matter for Bevy. Moving them to the tail is safe.

**Borrow note:** The `ec` chain in `spawn_prefab_instance` already releases before the
trigger_zone/stat_templates blocks; `attach_prefab_features` uses separate
`commands.entity(entity).insert(...)` calls (no long-lived borrow), so there is no E0499 risk.

## Tasks

- [ ] Move `attach_prefab_features` to `entity_spawner.rs` (it needs access to `resolve_project_path`, `PendingBehavior`, etc. — check imports)
- [ ] Re-export or re-reference it from `scene_loader.rs` (or duplicate the import chain)
- [ ] Remove the six duplicate feature blocks from `spawn_prefab_instance`
- [ ] Add `attach_prefab_features` call at the tail of `spawn_prefab_instance`
- [ ] Verify `animation_policy_loader_system` unaffected (it reads components, not writes)
- [ ] Tests pass: `cargo test -p ironhold_core --test integration_tests --test ron_validation --test ron_lint`
- [ ] `cargo check -p ironhold_cli`
- [ ] WASM dev build
- [ ] Play-test: `3rd_person_game_demo` (GLB Actor behavior/interactable/stat_templates), `entity_logic_demo` (Primitive), `primitive_world` (composite Primitive)

## Open questions

- Should `attach_prefab_features` live in `entity_spawner.rs`, `mod.rs`, or remain in `scene_loader.rs` with a `pub(super)` re-export? `entity_spawner.rs` is the natural home since `spawn_prefab_instance` owns the GLB path.

## Acceptance criteria

- A new `PrefabDef` capability field wired into `attach_prefab_features` automatically applies to GLB Actor/Prop, composite Primitive, and single-mesh Primitive spawn paths.
- All existing integration tests pass with no changes to test code.
- No behavioral change for any existing prefab — pure structural refactor.
