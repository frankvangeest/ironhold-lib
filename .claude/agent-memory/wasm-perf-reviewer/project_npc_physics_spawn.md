---
name: npc-physics-spawn
description: spawn_prefab_instance inserts Dynamic RigidBody+capsule+Velocity for any prefab with components.npc; per-spawn, not per-frame; WASM-safe (Rapier is single-threaded-fine)
metadata:
  type: project
---

`spawn_prefab_instance` (runtime/scene_manager/entity_spawner.rs) inserts, when `prefab.components.npc` is set: `NpcAgent`, `RigidBody::Dynamic`, `Collider::compound([capsule_y])`, `LockedAxes::ROTATION_LOCKED`, `Damping`, `Velocity`, `Friction`. Hardcoded conservative capsule: radius 0.35 m, total height 1.6 m.

Frequency: **per-spawn / per-scene-load**, NOT per-frame. For 3rd_person_game_demo this fires 4x at load (2 snakes, 2 spiders). Dynamic Action::Spawn routes through drain_spawn_queue_system (SPAWNS_PER_FRAME=2) so even wave spawns of NPCs are rate-limited.

WASM notes:
- Rapier3D dynamic bodies are fine on single-threaded WASM. bevy_rapier's parallel solver is not relied upon for correctness; it degrades to single-threaded cleanly in the browser. No std::thread/rayon hard dependency on the web path here.
- `Collider::compound` for a single capsule allocates a Vec of one element per spawn — trivial, per-spawn only. Mirrors the player spawn path (spawn_player_entity uses the same compound-of-one pattern), so consistent.
- Each NEW mesh+material combo still triggers a synchronous WebGPU pipeline compile on first render (~100-300 ms) — that is the GLB/material cost, not the physics insert. Pre-warm via PreloadPrefab on scene.ready (documented pattern) if NPCs are dynamically spawned.

Cost of N dynamic NPC bodies per FixedUpdate tick is the Rapier broad/narrow-phase + solver, single-threaded on web. 4 bodies is negligible; dozens of always-active Dynamic bodies (no sleep) would be the thing to watch — Rapier sleeps idle bodies, but NpcAgent writes Velocity every tick (drag multiply), which keeps them awake. See [[npc-locomotion-bridge]].

**How to apply:** Physics-body insertion at spawn is web-safe and correctly per-spawn. The scaling risk is total count of never-sleeping Dynamic bodies (kept awake by per-tick Velocity writes), not the insertion itself.
