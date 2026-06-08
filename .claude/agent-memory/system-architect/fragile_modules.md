---
name: fragile-modules
description: Modules in ironhold_core that are fragile or frequently misunderstood; consult when reviewing changes to these areas
metadata:
  type: project
---

## Composite prefab spawning (`runtime/scene_manager/scene_loader.rs`)

`spawn_primitive_children` handles both inline primitive children and nested prefab references. There are two code paths (single-mesh and composite/multi-child); TriggerZone and PendingBehavior must be wired up in **both** paths, not just one. This has been the source of multiple bugs where a feature worked for single-mesh prefabs but silently failed for composite ones.

Key invariant: all child-spawning goes through `spawn_primitive_children` — do not duplicate the mesh/material dispatch match arms. The two call sites are composite non-player prefabs and player cosmetic children.

## Spawn-site metadata divergence — RESOLVED for the universal set (`tag_spawned_entity`)

This was the single most recurring defect class in the spawner. As of the spawn-site consolidation refactor (2026-06), the **universal entity-identity set** is fixed: `tag_spawned_entity(ec, registry, id, prefab_key, click_selectable, targetable)` in `runtime/scene_manager/mod.rs` is the single source of truth for `SpawnId` + `PrefabKey` + `LevelEntity` + `SpawnRegistry` insertion + `ClickSelectable`/`Targetable` markers. All 7 spawn sites route through it. The two bugs that motivated it (GLB-actor missing `SpawnId`, GLB-player missing `SpeedMultiplier`/`SpawnId`) are now structurally impossible at those sites.

Interface note: it takes two **bools** (`click_selectable`, `targetable`) NOT `&PrefabDef` — deliberately, because both player paths have no `PrefabDef` in scope (primitive player carries a decomposed tuple; GLB player works from `PlayerConfig`). Do not "improve" it to take `&PrefabDef` — that reintroduces per-path divergence. Player-specific components (`SpeedMultiplier`, `CharacterController`, physics, camera) correctly stay at the call site.

Verification rule when reviewing: grep `SpawnId\(|PrefabKey\(|\.entities\.insert` across `src/` — the only hits should be the helper body and the struct defs. Any other hit is a regression of the consolidation.

Harmless residual: composite parent (`scene_loader.rs` ~248), GLB player (`entity_spawner.rs` ~323), and every GLB parent (`model_spawner.rs` ~35) still spawn with an inline `LevelEntity` that the helper then re-inserts. Idempotent ZST double-insert — no bug, mild doc smell. Leave `model_spawner.rs` (shared infra used beyond the 7 sites).

## Sibling divergence — conditional prefab-feature application (STILL LIVE, same bug class)

NOT covered by `tag_spawned_entity` (correctly out of its scope — it owns universal metadata, not conditional features). The *conditional, prefab-driven* components — `interactable`, `trigger_zone`, `behavior`/`PendingBehavior`, `stat_templates`/`StatMap`, plus primitive-only `motion` and `npc` — are still applied by **independent match arms in two paths**:

1. `spawn_prefab_instance` (`entity_spawner.rs` ~53-74, ~126) — used by GLB actor/prop, foliage trunk, dynamic `Action::Spawn`. Has `&PrefabDef`.
2. Single-mesh primitive branch (`scene_loader.rs` ~583-627) — applies its own `interactable`/`trigger_zone`/`behavior`/`stat_templates`/`motion`/`npc`.

Same silent-drift failure mode as the old metadata divergence, one abstraction level down (e.g. `spawn_prefab_instance` reads `interactable.hint_text`; the primitive path must keep that in sync — currently does). When reviewing ANY new conditional `PrefabDef` feature field, grep across both files. Candidate fix: `apply_prefab_features(ec, prefab, project_root, asset_server)` shared by both — but needs design (primitive path reads `motion`/`npc` the GLB path doesn't), so feature note not inline fix.

## EffectDef / LayerDef sync (`capabilities/particles.rs` or similar)

`EffectDef` has `deny_unknown_fields`. Any new field added to `EffectDef` must also be added to `LayerDef` AND copied in `From<&EffectDef> for LayerDef`. A mismatch causes `deny_unknown_fields` to silently kill the entire asset catalog parse — no error is surfaced to the designer, the effect just never loads.

## WebGPU 16-byte alignment (any new GPU struct)

All structs bound as uniforms (`AsBindGroup`) must use `Vec4` (16 bytes) for every field. Never bind `f32`, `Vec2`, or `Vec3` directly. Violations cause `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` panics in web builds only — native wgpu is permissive and won't catch this during development.

## Particle pipeline warmup (`capabilities/particles.rs`)

Three separate pipeline variants compile separately on WASM: additive (StandardMaterial + AlphaMode::Add), blend (StandardMaterial + AlphaMode::Blend), flame distort (PoolFlameMaterial + AlphaMode::Add). Each must be explicitly warmed by firing a SpawnEffect at y=-100 during scene load. Adding a new blend mode or material type means adding a 4th warmup call.

The warmup SpawnEffect calls consume ParticleBudget — scenes with tight budgets (e.g. 100) can exhaust them before the player can interact if warmup is not accounted for.

## Terrain generation (`capabilities/terrain.rs`)

Terrain mesh generation is async (AsyncComputeTaskPool). Never block the main thread. The shader is embedded via `include_str!` as a deliberate WebGPU bootstrapping exception — the asset loader and pipeline compiler race on WASM; embedding the shader avoids that race. This means terrain shader changes require recompiling (known constraint, documented).

## Spawn queue (`runtime/scene_manager/action_executor.rs`)

`Action::Spawn` does not call `spawn_prefab_instance` inline. It pushes to `PendingEntitySpawns`; `drain_spawn_queue_system` processes at most `SPAWNS_PER_FRAME = 2` per frame. This caps WebGPU pipeline-compile stalls on WASM. Do not change this cap without testing large wave spawns in a WASM build.
