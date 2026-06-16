---
name: Player spawn via Action::Spawn pattern
description: How Action::Spawn promotes a tags:["player"] prefab to a full player (camera+controller); touchpoints and the duplicated PlayerConfig-assembly footgun
metadata:
  type: project
---

`Action::Spawn` can spawn a full player (orbit camera + CharacterController) when the
referenced prefab has `tags: ["player"]`. This makes character-select / deferred player
spawn fully RON-authorable: a `spawning_*` FSM state fires `Spawn(prefab:"player_male",
id:"player_01", spawn_point:"player_start")` on `scene.ready:main`.

**Why it's aligned:** the player/non-player decision gates entirely on RON-authored
`prefab.components.tags` (TAG_PLAYER = "player" in scene_loader.rs). No hardcoded prefab
names in core. Camera/inputs/movement/animation_policy all read from the prefab's
`components`, falling back to `default_camera_config`/`default_input_map` when absent.

**Touchpoints (all six wired correctly as of 2026-06-16):**
1. `QueuedSpawn.player_config: Option<PlayerConfig>` (scene_manager/mod.rs)
2. `action_executor.rs` Action::Spawn arm: assembles PlayerConfig when tags contain "player"
3. `entity_spawner.rs` drain_spawn_queue_system: calls spawn_player_entity when player_config Some
4. `ActiveTonemapping` resource (mod.rs) — inserted in scene_loader.rs, init in lib.rs, read by drain
5. `PlayerConfig` schema (schema/player.rs) — spawn_id/prefab_key default empty for RON use
6. tag_spawned_entity called inside spawn_player_entity (SpawnId/PrefabKey/registry)

**FOOTGUN — duplicated PlayerConfig assembly.** The PlayerConfig is built in TWO places
that must stay byte-identical:
- `scene_loader.rs` ~line 685 (scene-placed `tags:["player"]` entity)
- `action_executor.rs` ~line 148 (dynamic Action::Spawn)
Both use `prefab.components.camera/inputs/movement` + `prefab.animation_policy`. If a new
PlayerConfig field is added, BOTH sites need it or scene-placed vs dynamic players diverge
silently. Candidate for a shared `assemble_player_config(prefab, model_path, pos, id, key)`
helper — not yet extracted (note for future review/suggestion).

**Tonemapping divergence risk:** dynamic spawn reads `ActiveTonemapping` resource (current
scene). The terrain-deferred player path (`spawn_player_when_terrain_ready`) reads
`PendingTonemapping` component, defaulting to AcesFitted. These are separate sources; if a
scene with terrain ever spawns the player via Action::Spawn, confirm ActiveTonemapping is
set before the drain runs (scene_loader inserts it during scene load, so ordering is fine).

**Designer reachability confirmed:** preview-only prefabs (no player tag, no camera/inputs)
used on character-select screens avoid spawning stray cameras/controllers — correct pattern.
The character_select.scene.ron uses `preview_male/female/zombie`; player prefabs stay
`tags:["player"]` and are only spawned via the FSM.
