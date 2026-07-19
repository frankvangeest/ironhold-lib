---
name: project-player-spawn-unification
description: Player body construction (GLB + primitive) unified in spawn_player_entity_core — spawn-time-only, no per-frame path
metadata:
  type: project
---

Player spawning unified via `spawn_player_entity_core` (entity_spawner.rs) dispatching on `PlayerConfig.model_source: PlayerModelSource` (Glb(String) | Primitive{shape,params,children}). Replaced the old ~165-line inline primitive-player block in scene_loader.rs.

**Why:** feature `player_model_source_unification` (v1) — "single-player is multiplayer-with-1"; removes silent per-field divergence between GLB and primitive player paths.

**How to apply (perf/WASM):**
- Entirely spawn-time / per-scene-load. No new Update system, no per-frame allocation. `spawn_scene_v2` is one-shot.
- `PrimitivePlayerCtx<'a>` (child_ctx: ChildSpawnCtx, prefab_catalog, load_errors) is built unconditionally in scene_loader's non-terrain branch even for all-GLB scenes — but it is only cheap borrows, negligible. Reborrowed per-player (≤~4) in `spawn_players_and_camera`.
- `spawn_primitive_children(..., &mut HashSet::new(), ...)` allocates a fresh HashSet per primitive player spawn — spawn-time only, same as pre-refactor, no regression.
- Zero new deps; net code reduction; no WASM-incompatible constructs (Rapier compound collider + Friction already used; GLB via async asset_server.load).
- `spawn_player_entity_core` `.expect()`-panics if given a Primitive source with no ctx — programming-invariant guard, unreachable in v1 (only scene-load path builds Primitive configs). Single panic string, negligible size.
- Nit: action_executor.rs `Action::Spawn` error branch does `tags.contains(&"player".to_string())` (allocates a String) — but only on the model-lookup-failure path, per-action not per-frame; use `.iter().any(|t| t == "player")` to avoid the alloc.
- Terrain-deferred + character-select primitive players are v3-deferred (pass `None` ctx); those still lack primitive support and warn-skip.
