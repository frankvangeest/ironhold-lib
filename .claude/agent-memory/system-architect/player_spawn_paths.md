---
name: player-spawn-paths
description: The FOUR player-construction code paths, their singular-player assumptions, drift risk, and the shared-helper rule for any new player spawn site (multi-player / character-select / respawn)
metadata:
  type: project
---

There are FOUR player-entity construction sites that MUST be kept in sync. Any feature that changes player spawning (local co-op, character select, respawn, possession) must account for all four or players diverge silently. See [[scene-prefab-boundary]] and [[core-architectural-decisions]].

**The sites (verified 2026-07-03):**
1. **GLB collector** — `scene_loader.rs:164` `player_config: Option<PlayerConfig>`, assembled at `:626`. SINGULAR: overwritten each player-tagged entity, so a 2nd GLB player discards the 1st.
2. **Primitive collector + inline spawn** — `scene_loader.rs:166` `primitive_player: Option<(tuple)>`, set at `:244`, spawned INLINE at `:699`–`:862` (builds its own `CharacterController` + `OrbitCamera`; does NOT use `PlayerConfig` at all). Also SINGULAR. **A primitive-capsule demo uses THIS path** — easy to miss because it's not a `PlayerConfig`. `Vec<PlayerConfig>` alone does nothing for primitive players.
3. **Dynamic spawn** — `action_executor.rs:148`–`173` assembles a 3rd `PlayerConfig` literal for `Action::Spawn` on a `tags:["player"]` prefab (character-select). One player per action, so it doesn't break count, but any new `PlayerConfig` field must be populated here too.
4. **`spawn_player_entity`** — `entity_spawner.rs:491`, the shared GLB spawn fn; consumes `QueuedSpawn.player_config` (`mod.rs:173`) and `PendingPlayerConfig` (`mod.rs:346`, NOT 868). GLB non-terrain path calls it directly (`scene_loader.rs:873`); terrain path defers via `PendingPlayerConfig`.

Note: `PlayerConfig` is assembled by hand in TWO literals (`scene_loader.rs:626` + `action_executor.rs:155`); a shared `assemble_player_config` helper was proposed and DROPPED in `claude_suggestions.md:71`. Land it BEFORE adding any new `PlayerConfig` field.

**Detection:** player prefabs are identified by `components.tags.contains("player")` (the `TAG_PLAYER` magic-string constant in scene_loader.rs). Same stringly-typed mechanism for `flycam`.

**Load-bearing components on the player** (omitting any silently breaks movement): `CharacterController`, `SpeedMultiplier(1.0)` (player_movement_system filters on it — caused a real past bug on the GLB path), `LocomotionState`, `AnimationRequests`, `ActiveOverride`, `RigidBody::Dynamic`, capsule `Collider::compound`, `LockedAxes::ROTATION_LOCKED`, plus `tag_spawned_entity` metadata. Camera is a SEPARATE top-level entity with `OrbitCamera{target: player}` — NOT a child.

**Known inconsistency:** GLB `spawn_player_entity` omits the zero-`Friction` component that the primitive and NPC paths include (capsule can snag on edges).

**Rule for new player-spawn paths (e.g. runtime `Action::Spawn` of a player prefab):**
- Factor `PlayerConfig` assembly into ONE shared helper; do not copy the field-by-field construction. The codebase has repeatedly been bitten by per-path divergence (see `tag_spawned_entity` consolidation history).
- Detect the player tag in the ACTION EXECUTOR, not `drain_spawn_queue_system` — the drain system intentionally has no catalog access (`QueuedSpawn` carries pre-resolved data).
- `drain_spawn_queue_system` has no scene context, so tonemapping for a runtime-spawned orbit camera must come from a resource set at scene load (mirror `LoadedSpawnPoints`) or fall back to `Tonemapping::AcesFitted`.
- A no-player scene spawns a "Default Camera"; runtime-spawning a player adds a second `Camera3d`. The dual-camera contract must be decided explicitly.

**How to apply:** When advising on any feature that spawns a player (character select, respawn, possession), insist on the shared `build_player_config` helper and the executor-side detection. Watch for terrain timing: scene-loader defers player spawn via `PendingPlayerConfig` until terrain is ready; a runtime spawn does not, so on terrain scenes the player may spawn before the ground collider exists.
