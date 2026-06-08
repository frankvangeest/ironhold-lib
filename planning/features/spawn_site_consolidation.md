# Feature: Consolidate entity-spawn component insertion

_Status: Draft — awaiting design decisions before coding_
_Planned at: `661ecd1` (2026-06-08)_

---

> ## Pre-implementation decisions (need Frank's input before coding)
>
> - [ ] **D1 — Helper shape.** A free function `tag_spawned_entity(ec: &mut EntityCommands, registry: &mut SpawnRegistry, id: &str, prefab_key: &str, prefab: &PrefabDef)` that inserts the common components AND registers in `SpawnRegistry`, vs. a `SpawnedEntity` **Bundle** (components only) + a separate one-line registry call at each site. A Bundle can't touch the `SpawnRegistry` resource, so the registry call would still be duplicated. **Recommendation: the free function** — it's the only form that removes *all* the divergence (components + registry) in one call.
>
> - [ ] **D2 — Should the helper also insert the targeting markers?** `ClickSelectable`/`Targetable` are currently inserted in 3 different places (spawn_prefab_instance, single-mesh branch, composite branch). Folding them into the helper (it reads `prefab.click_selectable`/`prefab.targetable`) removes that divergence too. **Recommendation: yes**, pass `&PrefabDef` so the helper sets markers from the flags.
>
> - [ ] **D3 — Route players through the helper for their common metadata?** Both player paths add lots of player-specific components (CharacterController, SpeedMultiplier, physics) that stay site-specific — but the *common* metadata (SpawnId, PrefabKey, LevelEntity, registry entry) should go through the helper. The **GLB player currently has none of SpawnId/PrefabKey/registry**; routing it through the helper fixes that. **Recommendation: yes** (helper for common metadata; player extras stay inline).
>
> - [ ] **D4 — Dynamic `Action::Spawn` spawns: add `PrefabKey` AND `LevelEntity`?** Today `drain_spawn_queue_system` adds only `SpawnId` + registry — **no `PrefabKey`** (logged gap) and **no `LevelEntity`**. The missing `LevelEntity` means dynamically-spawned entities are **not cleaned up on scene change** (they leak across scenes). Need confirmation this is unintended (almost certainly is). Requires threading the prefab catalog key into `QueuedSpawn` (available at the `Action::Spawn` call site). **Recommendation: add both**, but confirm the `LevelEntity` behavior change is desired (it may surface latent "entity persisted across scene load" assumptions in existing projects).
>
> - [ ] **D5 — Route the Foliage trunk root through the helper too?** It currently gets SpawnId + registry + LevelEntity but **no `PrefabKey`**. **Recommendation: yes** for consistency.

---

## What

Replace the ~7 independent entity-spawn sites' hand-rolled component-insertion lists with a single shared helper, so every addressable spawned entity gets a consistent metadata set (`SpawnId`, `PrefabKey`, `LevelEntity`, `SpawnRegistry` entry, and the optional targeting markers).

## Why

The spawn sites have drifted apart, and the divergence has already caused real, hard-to-diagnose bugs:

- **GLB player** had no `SpeedMultiplier` → `player_movement_system`'s query silently skipped it → WASD/Space dead for every GLB-model player (fixed in `34bc77d`).
- **GLB scene actors** had no `SpawnId`/registry entry → couldn't be targeted, despawned, or decaled by id (fixed in `34bc77d`).
- **GLB player** still has no `SpawnId`/`PrefabKey`/registry entry (the primitive player does).
- **Dynamic spawns** have no `PrefabKey` (id-only targeting display) and no `LevelEntity` (possible cross-scene leak).
- **Foliage root** has no `PrefabKey`.

Each fix this session was a one-line patch to one site — the underlying problem is that there is no single place that defines "what every spawned entity gets." A shared helper makes the "works for primitive, silently broken for GLB" footgun structurally impossible: add a field once, every site gets it.

## Current state — inventory (at `661ecd1`)

| Spawn site (file:line) | `SpawnId` | `PrefabKey` | registry | `LevelEntity` | markers | notes |
|---|---|---|---|---|---|---|
| GLB actor/prop — `scene_loader.rs:700` | ✓ | ✓ | ✓ | ✓ | ✓ (via `spawn_prefab_instance`) | reference shape |
| Single-mesh primitive — `scene_loader.rs:502` | ✓ | ✓ | ✓ | ✓ | ✓ | |
| Composite primitive — `scene_loader.rs:271` | ✓ | ✓ | ✓ | ✓ | ✓ | |
| Foliage root — `scene_loader.rs:202` | ✓ | ✗ | ✓ | ✓ | ✗ | missing PrefabKey |
| Primitive player — `scene_loader.rs:753` | ✓ | ✗ | ✓ | ✓ | — | missing PrefabKey |
| GLB player — `entity_spawner.rs:316` (`spawn_player_entity`) | ✗ | ✗ | ✗ | ✓ | — | missing all 3 metadata |
| Dynamic `Action::Spawn` — `entity_spawner.rs:190` (`drain_spawn_queue_system`) | ✓ | ✗ | ✓ | ✗ | ✓ (via `spawn_prefab_instance`) | missing PrefabKey + LevelEntity |

## Proposed design (pending D1–D5)

A free function in `runtime/scene_manager` (next to `SpawnId`/`PrefabKey`/`SpawnRegistry`):

```rust
/// Attach the standard metadata every addressable spawned entity needs, and register it.
/// Call from every spawn site so the set can never drift per-path again.
pub fn tag_spawned_entity(
    ec: &mut EntityCommands,
    registry: &mut SpawnRegistry,
    id: &str,
    prefab_key: &str,
    prefab: &PrefabDef,   // for click_selectable / targetable flags (D2)
) {
    ec.insert((SpawnId(id.into()), PrefabKey(prefab_key.into()), LevelEntity));
    if prefab.click_selectable { ec.insert(ClickSelectable); }
    if prefab.targetable { ec.insert(Targetable); }
    registry.entities.insert(id.into(), ec.id());
}
```

Each of the 7 sites replaces its bespoke insert list + registry call with one `tag_spawned_entity(...)` call (players additionally insert their player-specific components inline; dynamic spawns thread `prefab_key` via `QueuedSpawn`).

## Tasks (after D1–D5 resolved)

- [ ] Decisions D1–D5 resolved
- [ ] Add `tag_spawned_entity` helper (or Bundle per D1) in `runtime/scene_manager`
- [ ] Route all 7 spawn sites through it; thread `prefab_key` into `QueuedSpawn` for the dynamic path
- [ ] Confirm/adjust `LevelEntity` on dynamic spawns (D4) and add a regression note
- [ ] Tests: a spawn-coverage integration test asserting that a spawned entity of each kind (GLB actor, primitive, composite, GLB player, dynamic spawn) has `SpawnId` + `PrefabKey` + is in `SpawnRegistry`
- [ ] `cargo test` (integration_tests, ron_validation, ron_lint) green; CLI check; dev WASM build
- [ ] Docs: note the helper as the single source of truth for spawn metadata in `crates/ironhold_core/src/CLAUDE.md`

## Risks

- **Behavior change (D4):** adding `LevelEntity` to dynamic spawns changes scene-unload cleanup — could break a project that relied on dynamic entities persisting across `LoadScene`. Verify against existing projects (none currently rely on this, but confirm).
- **EntityCommands lifetime/borrow:** `ec.id()` + mutating the `registry` resource in the same helper is fine (disjoint), but the call sites must have `&mut SpawnRegistry` in scope — `spawn_player_entity` does not currently take it; signature change needed.
- Pure refactor otherwise — covered by the existing integration suite plus the new spawn-coverage test.
