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

**RESOLVED — duplicated PlayerConfig assembly.** The old footgun (PlayerConfig built
independently in `scene_loader.rs` and `action_executor.rs`, which had to be kept byte-identical
by hand) is fixed: both sites now call a single shared `assemble_player_config(prefab, prefab_key,
spawn_id, model_path, initial_position, player_nameplate_enabled)` helper in `entity_spawner.rs`
(`pub(crate)`, ~line 1719). `scene_loader.rs`'s scene-placed collector (~line 351/733) and
`action_executor.rs`'s dynamic `Action::Spawn`/`Action::JoinPlayer` arms all route through it, so a
new `PlayerConfig` field only needs adding inside `assemble_player_config` itself, not at every
call site. See "Player-construction sites" in `crates/ironhold_core/src/CLAUDE.md` for the full,
current up-to-five-site inventory (adds a terrain-deferred path and hot-join on top of the two
sites this note originally tracked) — what still needs checking against multiple sites is only
whether a *new* field is forwarded correctly through `assemble_player_config`'s two call sites, not
whether the assembly logic itself has diverged.

**Tonemapping divergence risk:** dynamic spawn reads `ActiveTonemapping` resource (current
scene). The terrain-deferred player path (`spawn_player_when_terrain_ready`) reads
`PendingTonemapping` component, defaulting to AcesFitted. These are separate sources; if a
scene with terrain ever spawns the player via Action::Spawn, confirm ActiveTonemapping is
set before the drain runs (scene_loader inserts it during scene load, so ordering is fine).

**Local co-op (Stage 1) extends this:** `local_coop_foundation.md` plan makes
`player_config: Option<PlayerConfig>` → `Vec<PlayerConfig>` (one player-tagged entity per
entry, spawns a controller+camera rig each). Player identity is RON-authored: each player
prefab carries `tags:["player"]` + distinct `PrefabDef.player_index: u32` + own
`components.inputs` (NO prefab-key string parsing). New `InputMap.gamepad_index: Option<usize>`
routes a player to a specific gamepad (first gamepad consumer in ironhold_core — verify WASM
Gamepad API path). New `PartyOrbitCamera` (targets: Vec<Entity>) + `CameraConfig.party:
PartyZoomDef{zoom_margin}`; the party block (not the raw 2+ player count) should be the
explicit RON switch — warn at load if 2+ players and no party. `GameSceneV2.max_view_box:
Option<(f32,f32,f32,f32)>` = clamp on when present, absent = off. `player_index` now only needs
setting inside `assemble_player_config` (see the RESOLVED note above) rather than at two
independently-maintained assembly sites — the old "must be set at both sites or players collide
on index 0" risk no longer applies now that both call sites share one helper. NOTE: the
gamepad-routing description in this paragraph (`InputMap.gamepad_index` as a live "nth connected
gamepad" lookup) is superseded — see `gamepad_binding_pattern.md` for the current
`BoundGamepad`/`gamepad_bind_system` seed-then-lock model.

**Designer reachability confirmed:** preview-only prefabs (no player tag, no camera/inputs)
used on character-select screens avoid spawning stray cameras/controllers — correct pattern.
The character_select.scene.ron uses `preview_male/female/zombie`; player prefabs stay
`tags:["player"]` and are only spawned via the FSM.
