---
name: player-spawn-paths
description: The (now three) player-construction code paths, their drift risk, and the shared-helper rule for any new player spawn site
metadata:
  type: project
---

There are multiple player-entity construction sites that MUST be kept in sync. See [[scene-prefab-boundary]] and [[core-architectural-decisions]].

**The sites (as of 2026-06):**
1. Primitive player — `scene_loader.rs` (~line 769), spawns capsule+mesh+camera inline.
2. GLB player — `entity_spawner.rs::spawn_player_entity` (~line 402), spawns GLB+capsule+orbit-camera inline; takes a `PlayerConfig`.
3. `PlayerConfig` assembly — `scene_loader.rs` (~line 685), builds the config from a `tags:["player"]` prefab.

**Detection:** player prefabs are identified by `components.tags.contains("player")` (the `TAG_PLAYER` magic-string constant in scene_loader.rs). Same stringly-typed mechanism for `flycam`.

**Load-bearing components on the player** (omitting any silently breaks movement): `CharacterController`, `SpeedMultiplier(1.0)` (player_movement_system filters on it — caused a real past bug on the GLB path), `LocomotionState`, `AnimationRequests`, `ActiveOverride`, `RigidBody::Dynamic`, capsule `Collider::compound`, `LockedAxes::ROTATION_LOCKED`, plus `tag_spawned_entity` metadata. Camera is a SEPARATE top-level entity with `OrbitCamera{target: player}` — NOT a child.

**Known inconsistency:** GLB `spawn_player_entity` omits the zero-`Friction` component that the primitive and NPC paths include (capsule can snag on edges).

**Rule for new player-spawn paths (e.g. runtime `Action::Spawn` of a player prefab):**
- Factor `PlayerConfig` assembly into ONE shared helper; do not copy the field-by-field construction. The codebase has repeatedly been bitten by per-path divergence (see `tag_spawned_entity` consolidation history).
- Detect the player tag in the ACTION EXECUTOR, not `drain_spawn_queue_system` — the drain system intentionally has no catalog access (`QueuedSpawn` carries pre-resolved data).
- `drain_spawn_queue_system` has no scene context, so tonemapping for a runtime-spawned orbit camera must come from a resource set at scene load (mirror `LoadedSpawnPoints`) or fall back to `Tonemapping::AcesFitted`.
- A no-player scene spawns a "Default Camera"; runtime-spawning a player adds a second `Camera3d`. The dual-camera contract must be decided explicitly.

**How to apply:** When advising on any feature that spawns a player (character select, respawn, possession), insist on the shared `build_player_config` helper and the executor-side detection. Watch for terrain timing: scene-loader defers player spawn via `PendingPlayerConfig` until terrain is ready; a runtime spawn does not, so on terrain scenes the player may spawn before the ground collider exists.
