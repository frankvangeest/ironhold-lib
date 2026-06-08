---
name: fragile-modules
description: Modules in ironhold_core that are fragile or frequently misunderstood; consult when reviewing changes to these areas
metadata:
  type: project
---

## Composite prefab spawning (`runtime/scene_manager/scene_loader.rs`)

`spawn_primitive_children` handles both inline primitive children and nested prefab references. There are two code paths (single-mesh and composite/multi-child); TriggerZone and PendingBehavior must be wired up in **both** paths, not just one. This has been the source of multiple bugs where a feature worked for single-mesh prefabs but silently failed for composite ones.

Key invariant: all child-spawning goes through `spawn_primitive_children` — do not duplicate the mesh/material dispatch match arms. The two call sites are composite non-player prefabs and player cosmetic children.

## Five-way spawn-site divergence (the recurring footgun)

This is the single most recurring defect class in the spawner. Entity-identity components (`SpawnId`, `PrefabKey`, `SpawnRegistry` insertion) AND optional marker components (`ClickSelectable`, `Targetable`, `Interactable`, `TriggerZone`, `Motion`, `StatMap`, `PendingBehavior`, `npc`) are attached **independently in five spawn sites** with no shared helper and no compiler enforcement:

1. Single-mesh primitive — `scene_loader.rs` ~line 501
2. Composite primitive — `scene_loader.rs` ~line 270
3. GLB actor/prop — `scene_loader.rs` ~line 699
4. `spawn_prefab_instance` (used by foliage trunk + dynamic `Action::Spawn`) — `entity_spawner.rs` ~line 100; note `SpawnId` is added by the *caller* (`drain_spawn_queue_system` ~line 190), NOT inside the fn
5. Primitive player / GLB player — `scene_loader.rs` ~line 753 / `entity_spawner.rs` ~line 316

Failure mode is **silent** (no parse error, no warning) and only manifests for the prefab `kind` the author didn't test. History: TriggerZone (composite path), then composite markers, then targeting (2026-06 — GLB-actor path was missing `SpawnId` so `Query<&SpawnId, With<ClickSelectable>>` skipped GLB enemies; GLB *player* was missing `SpeedMultiplier` so it never moved). Known remaining gap as of 2026-06: dynamic `Action::Spawn` entities get `SpawnId` but no `PrefabKey` (`QueuedSpawn` doesn't carry the key).

**Recommended durable fix (also independently proposed by alignment-reviewer):** extract `attach_standard_prefab_components(ec, registry, prefab, id, prefab_key, ...)` in `entity_spawner.rs`, called by every branch. Make `SpawnId`/`PrefabKey`/registry insertion unconditional inside it; move the pure-component markers (`ClickSelectable`, `Targetable`, `Interactable`, `TriggerZone`) in verbatim. Path-specific physics bodies (player capsule, NPC dynamic body) stay inline. When reviewing ANY new `PrefabDef` marker field, grep the field name across both files and expect it in every branch (or one shared helper call).

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
