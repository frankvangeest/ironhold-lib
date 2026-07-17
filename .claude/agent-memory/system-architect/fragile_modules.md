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

### DOUBLE-INSERTION trap on the GLB else-branch (observed 2026-06, working-tree change)
The GLB actor/prop else-branch in `scene_loader.rs` (~693) **calls `spawn_prefab_instance`** (which already inserts `PendingBehavior`/`Interactable`/`TriggerZone`/`StatMap`) and historically only added `label`. A working-tree change re-inserted behavior/interactable/trigger_zone/stat_templates inline on the same `parent` — duplicating what `spawn_prefab_instance` does. Result: TriggerZone collider/Sensor inserted twice (last-write-wins, harmless-ish but wasteful), StatMap built twice. The ONLY things the GLB else-branch legitimately needs beyond `spawn_prefab_instance` are: `stat_overrides` application (spawn_prefab_instance can't — no `entity_def`), `stat_label`, `world_stat_bar`, and `motion` (none of which spawn_prefab_instance handles). Lesson when reviewing this branch: anything `spawn_prefab_instance` already does must NOT be re-added at the call site — only the entity_def-derived and label/bar/motion extras belong there. The clean fix is to push `stat_overrides` (a `&HashMap<String,f32>` arg, default empty) into `spawn_prefab_instance` so the StatMap is built once, and have it also handle `motion`/`stat_label`/`world_stat_bar` — collapsing the 3-way duplication to one site.

## EffectDef / LayerDef sync (`capabilities/particles.rs` or similar)

`EffectDef` has `deny_unknown_fields`. Any new field added to `EffectDef` must also be added to `LayerDef` AND copied in `From<&EffectDef> for LayerDef`. A mismatch causes `deny_unknown_fields` to silently kill the entire asset catalog parse — no error is surfaced to the designer, the effect just never loads.

## WebGPU 16-byte alignment (any new GPU struct)

All structs bound as uniforms (`AsBindGroup`) must use `Vec4` (16 bytes) for every field. Never bind `f32`, `Vec2`, or `Vec3` directly. Violations cause `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` panics in web builds only — native wgpu is permissive and won't catch this during development.

## Particle pipeline warmup (`capabilities/particles.rs`)

Three separate pipeline variants compile separately on WASM: additive (StandardMaterial + AlphaMode::Add), blend (StandardMaterial + AlphaMode::Blend), flame distort (PoolFlameMaterial + AlphaMode::Add). Each must be explicitly warmed by firing a SpawnEffect at y=-100 during scene load. Adding a new blend mode or material type means adding a 4th warmup call.

The warmup SpawnEffect calls consume ParticleBudget — scenes with tight budgets (e.g. 100) can exhaust them before the player can interact if warmup is not accounted for.

## Terrain generation (`capabilities/terrain.rs`)

Terrain mesh generation is async (AsyncComputeTaskPool). Never block the main thread. The shader is embedded via `include_str!` as a deliberate WebGPU bootstrapping exception — the asset loader and pipeline compiler race on WASM; embedding the shader avoids that race. This means terrain shader changes require recompiling (known constraint, documented).

Current vertex layout (as of 2026-06): single-mesh, indexed `TriangleList`, attributes = POSITION (vec3 f32) + NORMAL (vec3 f32) + UV_0 (vec2 f32) = 32 B/vertex. No custom vertex shader — `TerrainMaterial` overrides fragment only. `chunk_size: u32` exists in `TerrainConfigV2` (default 64) but the generator does NOT use it — terrain is one giant mesh (no per-chunk culling, one collider).

### Terrain optimisation: the custom-vertex-shader gate
Most "terrain GPU optimisation" wins (procedural XZ from `@builtin(vertex_index)`, octahedral normal compression, geo-sink LOD) REQUIRE a custom vertex shader. Per core CLAUDE.md, vertex-shader override is "planned but not yet implemented" for `CustomMaterial` and unproven for `TerrainMaterial`. **Treat a custom-vertex-shader PoC (does it pass WebGPU validation in this Bevy 0.18 setup?) as the gate for all those features.** Without it, the only safe vertex-memory win is dropping UV_0 (derive from world_position.xz in fragment) ≈ 25%, plus U16 indices when verts < 65536.

Realistic-win framing (the Vercidium "10×" figure is for an unoptimised non-indexed OpenGL renderer): Ironhold already indexes meshes and Bevy already auto-batches, so batching/strips give little here. The high-value structural change is **mesh chunking** (wire up the existing `chunk_size`): enables per-chunk frustum culling, granular colliders, incremental async generation (mitigates the WASM first-frame `block_on` stall), and unblocks queued backlog items 77 (pre-baked LOD swap) and 179 (chunked streaming). Chunking needs no custom shader and is the recommended first structural step.

Collider trap for procedural-XZ phase: if positions live only in the vertex shader, Rapier `Collider::from_bevy_mesh` has no geometry — must keep a CPU-only position/height array for collider build.

`TerrainMaterial.uv_scale: Vec4` is documented in THREE doc sites as "only .x used; padded for alignment." Any repurposing of `.yzw` (e.g. packing terrain world-extent for fragment UV derivation) must update all three or future readers assume they're dead.

## Spawn queue (`runtime/scene_manager/action_executor.rs`)

`Action::Spawn` does not call `spawn_prefab_instance` inline. It pushes to `PendingEntitySpawns`; `drain_spawn_queue_system` processes at most `SPAWNS_PER_FRAME = 2` per frame. This caps WebGPU pipeline-compile stalls on WASM. Do not change this cap without testing large wave spawns in a WASM build.

## `spawn_scene_v2` is at the 16-param SystemParam ceiling (`runtime/scene_manager/scene_loader.rs` ~L43)

As of 2026-07-17, `spawn_scene_v2` has **exactly 16 top-level system params** (commands, `SceneV2Params`, asset_server, events, next_state, state, level_entities, overlay_entities, scene_events, model_spawner, merged_fixes, `SceneMaterialParams`, spawn_registry, load_mode, project_key_bindings, loaded_key_bindings). Bevy derives `SystemParam` tuples only up to arity 16 — **adding a 17th top-level param is a compile error**. Any change that needs a new resource in the scene-load system (e.g. threading `DynamicStatUiQueue` in to route players through the dynamic-widget queue) must bundle it into an existing `SystemParam` struct (`SceneV2Params` is the natural home), not add a bare param. Two sibling player-spawn callers are ordinary systems with headroom: `drain_spawn_queue_system` already holds `ResMut<DynamicStatUiQueue>`; `spawn_delayed_players_system` (terrain-delayed) can add it freely.
