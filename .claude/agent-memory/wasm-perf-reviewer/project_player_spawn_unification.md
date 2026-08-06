---
name: project-player-spawn-unification
description: Player body construction (GLB + primitive) unified in spawn_player_entity_core — spawn-time-only, no per-frame path; v2 adds universal zero-Friction
metadata:
  type: project
---

Player spawning unified via `spawn_player_entity_core` (entity_spawner.rs) dispatching on `PlayerConfig.model_source: PlayerModelSource` (Glb(String) | Primitive{shape,params,children}). Replaced the old ~165-line inline primitive-player block in scene_loader.rs.

**Why:** feature `player_model_source_unification` — "single-player is multiplayer-with-1"; removes silent per-field divergence between GLB and primitive player paths.

**How to apply (perf/WASM):**
- Entirely spawn-time / per-scene-load. No new Update system, no per-frame allocation. `spawn_scene_v2` is one-shot.
- `PrimitivePlayerCtx<'a>` (child_ctx, prefab_catalog, load_errors) is built unconditionally in scene_loader's non-terrain branch even for all-GLB scenes — cheap borrows only. Reborrowed per-player (≤~4).
- `spawn_primitive_children(..., &mut HashSet::new(), ...)` allocates a fresh HashSet per primitive player spawn — spawn-time only.
- Zero new deps in v1 or v2; no WASM-incompatible constructs.
- Terrain-deferred + character-select + hot-join primitive players are deferred (pass `None` ctx); they warn-skip.

**v2 (reviewed 2026-08-06): zero-`Friction` now inserted unconditionally for every player**, inside the main 13-element bundle tuple (was a separate primitive-only `commands.entity().insert()` behind a `matches!` guard). Perf effect is a small *improvement*: one fewer archetype relocation at spawn for primitive players, and `coefficient: 0.0` / `CoefficientCombineRule::Min` means fewer friction constraint rows for Rapier to solve on player contacts than the previous GLB default 0.5/Average. No system in `ironhold_core` queries `&Friction` or `Without<Friction>`, so no query-shape/archetype-fragmentation side effect. Binary-size delta sub-KB.

**`material:` on a prefab overrides ALL mesh descendants, including composed `children:`.** `apply_material_overrides` (material_factory.rs, ~line 198) does `children.iter_descendants(root).filter(has Mesh3d)` — every authored per-child `color`/`metallic`/`roughness` is silently replaced. `spawn_primitive_children` has already created one `StandardMaterial` asset per child by then, so those are orphaned after ~1 frame (wasted spawn-time asset churn + a GPU material write for materials that never render). First surfaced by `local_coop_demo`'s `player_p2_primitive_split_ring` — as of 2026-08-06 the only prefab in the repo combining `material:` with `children:`. Cheaper *and* visually correct fix: drop `material:` and put the tint in the root's `primitive: (color: ...)`, letting children keep authored colors and skipping the override pass entirely.

**`spawn_primitive_children` never dedups meshes or materials** — two identical child shapes = two separate `Mesh` assets + two `StandardMaterial` assets, so they cannot batch/instance together. Fine at the 3-4 children existing prefabs use (`portal_to_room9`, the composed player); scales badly past ~20 children, and under split-screen each child is multiplied by (views × (main + directional shadow cascade)).

**A `children:` entry with `primitive.physics: true` inserts `RigidBody::Fixed` on the *parent*.** On a player prefab that would clobber the player's `RigidBody::Dynamic` and freeze them. No shipped prefab does this yet, but composed player bodies are now a copyable pattern — watch for it.

**Physics timestep is `TimestepMode::Variable { max_dt: 1/60, substeps: 1 }`** (bevy_rapier3d 0.33 default; `capabilities/physics.rs` does not override it) while `player_movement_system`'s `idle_drag` runs in FixedUpdate at 64 Hz. On a slow browser frame rapier clamps dt to 16.7 ms while FixedUpdate still ticks ~2x, so drag is applied *more* per unit simulated time in the web build than at high native refresh — the browser is the conservative side of any friction/slope-creep change. A passing browser playtest does not prove native is fine.
