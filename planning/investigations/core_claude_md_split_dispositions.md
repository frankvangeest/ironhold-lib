# Disposition table: where each block went (core CLAUDE.md split, after Phase 4)

_Generated 2026-10-08 from `core_claude_md_split_blocks.json` and the destination files. "Was" is the original block (`git show d222d91:crates/ironhold_core/src/CLAUDE.md`); "Now" is the characters of the condensed section(s) in the folder/parent files; "Topic" is the verbatim copy in `docs/dev/`. "Narrative" says where cut text lives: a **plan file** (verbatim append), the **topic doc**, **kept** (rule kept as is) or **cut** (derivable/duplicate, reason in the sidecar). Block ids are named after the first line in `ba4088d`._

| Block | Was | Now | Topic | Safety | Destination | Narrative | Title |
|---|---|---|---|---|---|---|---|
| b:1 | 2447 | 1798 | 0 | Y | parent | kept | Message→Interpreter→Action→Executor pipeline; sens |
| b:35 | 1268 | 1105 | 0 | Y | SCH | plan: `done/action_deny_unknown_fields.md` | Adding new actions + `deny_unknown_fields` consequ |
| b:54 | 648 | 1028 | 0 | N | SCH | kept | RON struct vs tuple variant syntax |
| b:70 | 1455 | 963 | 0 | Y | SM | plan: `done/rules_to_state_machine_consolidation.md` | Conditions on rules: only `LogicState`; `EnterStat |
| b:81 | 831 | 776 | 0 | Y | parent | kept | No hardcoded assets; sRGB colours only |
| b:92 | 2031 | 1186 | 0 | Y | SM | plan: `done/nested_prefabs.md` | Composite/nested prefab spawning via `spawn_primit |
| b:108 | 746 | 2204 | 0 | Y | parent, SCH | plan: `ui_flex_group.md` | `scene.ui` must be walked via `walk_ui_nodes` |
| b:111 | 981 | 697 | 0 | Y | SM | plan: `done/entity_logic.md` | Entity FSM overview, interpreter chain order, neve |
| b:127 | 1488 | 0 | 0 | N | (cut) | cut: derivable from `action_substitution.rs` (`rewrite_self`); ** | Supported `{self}` targets (15 examples) |
| b:144 | 3709 | 1648 | 0 | Y | SM | plan: `done/monotonic_entity_id.md` | `{new_id}` semantics, collisions, resolved at exec |
| b:188 | 483 | 474 | 0 | N | SM | plan: `done/targeting_system.md` | `{target}` substitution via `CurrentTarget` |
| b:190 | 1382 | 0 | 1441 | N | TOPIC:action-bar-input-routing | topic doc | Per-player targeting Phase 1: `PlayerTarget` vs `C |
| b:206 | 3801 | 0 | 3815 | N | TOPIC:action-bar-input-routing | topic doc | Per-player action bars Phase 2: `owner_player`, `o |
| b:247 | 733 | 786 | 0 | Y | CAP, SM | plan: `done/per_player_split_screen_targeting.md` | `action_bar_input_system` requires `PlayerTarget`  |
| b:256 | 2466 | 0 | 2471 | N | TOPIC:action-bar-input-routing | topic doc | Gamepad-routed action-bar slots (`gamepad_key`, `B |
| b:284 | 2333 | 932 | 2333 | Y | CAP, TOPIC:action-bar-input-routing | topic doc | Mouse-click action-bar slots (`Option<Ref<Interact |
| b:286 | 2414 | 1149 | 0 | Y | CAP | plan: `done/per_player_split_screen_targeting.md` | Targeting chain ordering `.before(action_bar_input |
| b:315 | 619 | 0 | 0 | N | (cut) | cut: verified duplicate of crates/ironhold_core/tests/CLAUDE.md:1 | `Targetable` test fixtures must register in `Spawn |
| b:323 | 2353 | 883 | 0 | N | CAP | plan: `done/per_player_split_screen_targeting.md` | `target.*` events, screen-space selection, `select |
| b:352 | 2404 | 664 | 2412 | N | CAP, TOPIC:split-screen-cameras-and-widgets | topic doc | Target indicator ring, per-player, colour preceden |
| b:383 | 1180 | 0 | 1206 | N | TOPIC:split-screen-cameras-and-widgets | topic doc | Per-viewport ring visibility: RenderLayers intro |
| b:396 | 802 | 0 | 802 | N | TOPIC:split-screen-cameras-and-widgets | topic doc | Orbit camera / ring layer assignment |
| b:406 | 710 | 396 | 0 | N | CAP | plan: `done/per_viewport_target_ring_visibility.md` | Party camera must carry `all_ring_layers()` |
| b:414 | 870 | 0 | 870 | N | TOPIC:split-screen-cameras-and-widgets | topic doc | `TargetRingVisibilityMode` lifecycle |
| b:424 | 748 | 0 | 0 | N | (cut) | cut: derivable from the two warn! sites (spawn_players_and_camera | Warnings: `player_index` collision, non-hot-join S |
| b:433 | 408 | 176 | 0 | N | CAP | plan: `done/per_viewport_target_ring_visibility.md` | `pipeline_warmup_system` doesn't touch RenderLayer |
| b:439 | 586 | 355 | 0 | N | CAP | plan: `done/per_viewport_target_ring_visibility.md` | Light visibility intersects light RenderLayers (sh |
| b:446 | 369 | 0 | 0 | N | (cut) | cut: grep gives the same | "Reader-facing entry points" for the ring feature |
| b:452 | 1032 | 204 | 1032 | N | CAP, TOPIC:split-screen-cameras-and-widgets | topic doc | Per-viewport target HUD readout |
| b:464 | 323 | 161 | 0 | N | SM | plan: `done/entity_logic.md` | `behavior` works on composite primitive prefabs |
| b:468 | 394 | 0 | 0 | N | (cut) | cut: covered by docs/20 | `TriggerZone` description |
| b:472 | 1222 | 643 | 0 | N | CAP | plan: `done/item_gated_interactable.md` | `Interactable` fires for every in-range interactab |
| b:478 | 880 | 0 | 879 | N | TOPIC:lootable-corpse | topic doc | Lootable corpse design summary |
| b:489 | 1567 | 715 | 0 | Y | SM | plan: `done/monster_corpse_loot.md` | `Action::Spawn.at_entity` copies full transform; s |
| b:507 | 1497 | 0 | 0 | N | (cut) | cut: history of a retrofit; the one current-state line moves into | Corpse ids unique via `{new_id}` (retrofit history |
| b:524 | 1901 | 399 | 0 | N | CAP | plan: `done/monster_corpse_loot.md` | Prefer `SetDespawnTimer` over `EmitEventAfterDelay |
| b:545 | 682 | 299 | 0 | N | CAP | plan: `done/monster_corpse_loot.md` | `target_auto_clear_system` clears on despawn |
| b:554 | 648 | 329 | 0 | N | SM | plan: `done/monster_corpse_loot.md` | `Action::Despawn` closes an open container panel |
| b:563 | 1055 | 0 | 1055 | N | TOPIC:lootable-corpse | topic doc | A dying entity can't catch its own respawn timer;  |
| b:575 | 1026 | 289 | 1026 | N | parent, TOPIC:lootable-corpse, TOPIC:lootable-corpse | topic doc | Respawn rules must be in `global_on`, not state-sc |
| b:587 | 944 | 630 | 0 | N | SM | plan: `done/monster_corpse_loot.md` | `OpenContainer` must not double-count `panels_open |
| b:598 | 800 | 485 | 0 | Y | SM | plan: `done/monster_corpse_loot.md` | No `trigger_zone` on a Dynamic rigid-body prefab |
| b:608 | 1141 | 0 | 0 | N | (cut) | cut: the surviving reason lives in the 524-543 rule | Superseded same-entity corpse design (cautionary) |
| b:622 | 1179 | 0 | 1212 | N | TOPIC:animation-pipeline | topic doc | Animation resolver/playback pipeline, field-owners |
| b:638 | 706 | 0 | 705 | N | TOPIC:animation-pipeline | topic doc | `pending_seek` purpose |
| b:647 | 846 | 321 | 0 | N | CAP | plan: `done/dynamic_animation_control.md` | Paused clip must be resumed before next play |
| b:658 | 882 | 289 | 0 | N | CAP | plan: `done/dynamic_animation_control.md` | `set_seek_time` not `seek_to`; duration via the li |
| b:670 | 539 | 281 | 0 | Y | CAP | plan: `done/dynamic_animation_control.md` | `ActiveOverride.seek_fraction`/`frozen` must be du |
| b:677 | 639 | 0 | 639 | N | TOPIC:animation-pipeline | topic doc | Spawn-already-posed needs a minimal `AnimationPoli |
| b:686 | 1381 | 789 | 0 | N | CAP | plan: `done/dialogue_system.md` | Dialogue system |
| b:706 | 534 | 499 | 0 | Y | parent | kept | WebGPU 16-byte alignment for GPU structs |
| b:711 | 1122 | 919 | 0 | Y | ASSETS | kept | WGSL authoring rules for designer-facing shaders |
| b:722 | 1173 | 685 | 0 | Y | parent | kept | Engine-internal shaders embedded via `include_str! |
| b:729 | 69 | 111 | 0 | N | parent | kept | Pointer to `docs/25_custom_shaders.md` |
| b:731 | 340 | 336 | 0 | Y | parent | kept | Physics/movement must run in `FixedUpdate` (camera |
| b:734 | 1434 | 830 | 0 | Y | parent | kept | Rapier steps in `FixedUpdate` at 64Hz; multi-tick  |
| b:750 | 811 | 463 | 0 | Y | parent | kept | Update systems must not read `&GlobalTransform`; u |
| b:759 | 567 | 330 | 0 | N | CAP | plan: `deterministic_fixed_timestep.md` | "Confirmed fine, deliberately unconverted" list |
| b:765 | 216 | 141 | 0 | N | CAP | plan: `deterministic_fixed_timestep.md` | Faster-than-64Hz displays step motion (accepted) |
| b:769 | 797 | 462 | 0 | N | SCH | plan: `deterministic_fixed_timestep.md` | `ProjectConfig.max_frame_delta_secs` caps catch-up |
| b:779 | 3000 | 0 | 3069 | N | TOPIC:player-ground-detection | topic doc | Jump reset can't rely on a ground-check edge; slop |
| b:815 | 3344 | 0 | 3361 | N | TOPIC:player-ground-detection | topic doc | `jumps_used` reset: grace ticks, velocity, liftoff |
| b:856 | 2308 | 487 | 2347 | Y | CAP, TOPIC:player-ground-detection | topic doc | Reset must read `raw_grounded`, never coyote-buffe |
| b:883 | 2375 | 0 | 2427 | N | TOPIC:player-ground-detection | topic doc | Coyote time: debounced grounding |
| b:912 | 1473 | 397 | 1472 | Y | CAP, TOPIC:player-ground-detection | topic doc | `can_jump`: coyote buffer unlocks only the first j |
| b:929 | 1857 | 598 | 1890 | Y | CAP, TOPIC:player-ground-detection | topic doc | Ground shape-cast must `.exclude_sensors()` and `n |
| b:950 | 4605 | 0 | 4611 | N | TOPIC:player-ground-detection | topic doc | `ground_cast` re-query loop, "underfoot", `is_walk |
| b:1005 | 1411 | 0 | 1433 | N | TOPIC:player-ground-detection | topic doc | `jump_air_grace_ticks` NaN clamp; accepted pogo co |
| b:1023 | 3769 | 2296 | 0 | Y | parent | kept | Deterministic iteration order: BTreeMap/IndexMap;  |
| b:1027 | 172 | 142 | 0 | Y | CAP | kept | Terrain generation is async |
| b:1030 | 356 | 316 | 0 | Y | parent | kept | Inspector isolation (`cfg_attr(feature = "inspecto |
| b:1033 | 950 | 612 | 0 | N | parent | kept | Native frame cap, unfocused throttle, pipeline war |
| b:1041 | 768 | 605 | 0 | Y | parent | kept | Change-detection discipline for render-affecting c |
| b:1053 | 725 | 430 | 0 | N | SM | plan: `done/audio_mute_toggle.md` | Audio preloading: `preload_audio_system`, `LoadedA |
| b:1060 | 910 | 796 | 0 | N | SM | plan: `done/audio_volume_readout.md` | `audio_volume_var_system` mirrors `AudioState` to  |
| b:1063 | 602 | 0 | 0 | N | (cut) | cut: already at docs/20:1928; add the decoder start-up-on-first-p | SFX: no leading silence, WAV only |
| b:1067 | 1209 | 921 | 0 | Y | SM | plan: `done/spawn_site_consolidation.md` | `tag_spawned_entity` is the single spawn-metadata  |
| b:1084 | 2315 | 938 | 0 | N | parent | kept | Prefer `try_despawn()` when an entity may already  |
| b:1111 | 854 | 530 | 0 | Y | parent | kept | Child `LevelEntity`/`OverlayEntity` self-tagging i |
| b:1121 | 6516 | 689 | 6678 | N | SM, TOPIC:player-spawn-sites | topic doc | Player-construction sites inventory (1-5) + post-d |
| b:1201 | 957 | 536 | 0 | N | SM | plan: `done/player_stat_widgets.md` | Player `stat_label`/`world_stat_bar` routing; 16-p |
| b:1213 | 946 | 901 | 0 | N | SCH, SM | plan: `player_model_source_unification.md` | `PrefabDef.material` not auto-applied on the playe |
| b:1224 | 3144 | 1215 | 3195 | Y | CAP, SM, TOPIC:player-ground-detection | topic doc | Collider divergence; Friction 0.15 idle-only after |
| b:1261 | 6085 | 0 | 6120 | N | TOPIC:split-screen-cameras-and-widgets | topic doc | Local co-op: Party, split, Grid, dynamic split, fa |
| b:1331 | 2371 | 575 | 0 | N | CAP | plan: `done/camera_modes.md` | Split prefabs need `zoom_speed:0`, `orbit_button:" |
| b:1359 | 1347 | 517 | 0 | N | CAP | plan: `done/per_player_camera_look_controls.md` | Keyboard camera look |
| b:1374 | 719 | 314 | 0 | N | CAP | plan: `done/camera_modes.md` | `CameraShake` query must exclude Fixed/FirstPerson |
| b:1383 | 1948 | 1151 | 0 | Y | CAP | plan: `done/camera_modes.md` | `SetCameraMode`; Authored vs Active camera mode; ` |
| b:1406 | 4956 | 911 | 4979 | N | CAP, TOPIC:split-screen-cameras-and-widgets | topic doc | `WorldLabelRank` viewport-aware labels; per-rank w |
| b:1463 | 1072 | 0 | 1063 | N | TOPIC:split-screen-cameras-and-widgets | topic doc | `particle_renderer` billboard viewport-aware (Phas |
| b:1476 | 1028 | 213 | 1028 | N | CAP, TOPIC:split-screen-cameras-and-widgets | topic doc | Targeting `click_select` viewport-aware (Phase 2) |
| b:1488 | 2057 | 273 | 2057 | N | CAP, TOPIC:split-screen-cameras-and-widgets | topic doc | Nameplate distance culling via stored camera dista |
| b:1511 | 1688 | 0 | 1715 | N | TOPIC:split-screen-cameras-and-widgets | topic doc | Split-screen player HUD labels (P{n}) |
| b:1530 | 4152 | 2254 | 4174 | Y | CAP, RT, TOPIC:gamepad-routing, TOPIC:gamepad-routing | topic doc | Gamepad routing: `BoundGamepad`, `claimed` invaria |
| b:1576 | 1082 | 0 | 1082 | N | TOPIC:gamepad-routing | topic doc | Gamepad button/axis RON mapping, parity gap |
| b:1589 | 1396 | 743 | 0 | N | SCH | plan: `done/local_coop_foundation.md` | Duplicate `gamepad_index`/`player_index` validatio |
| b:1606 | 2864 | 1447 | 2928 | Y | RT, SM, TOPIC:gamepad-routing, TOPIC:gamepad-routing | topic doc | Gamepad-triggered hot join: `PendingJoinGamepad` |
| b:1638 | 623 | 399 | 0 | N | CAP | plan: `done/local_coop_4way_split.md` | View-box clamp |
| b:1646 | 936 | 607 | 0 | Y | SM | plan: `done/dynamic_spawn_components.md` | Spawn queue: `SPAWNS_PER_FRAME = 2` |
| b:1657 | 3944 | 1870 | 0 | N | SM | plan: `done/dynamic_spawn_components.md` | Component parity: dynamic vs scene-placed (namepla |
| b:1668 | 3844 | 1750 | 0 | N | SM | plan: `done/label_depth_scale_validation.md` | Label depth scale + validation coverage |
| b:1695 | 1566 | 0 | 0 | N | (cut) | cut: accepted CLI-vs-runtime asymmetry; one sentence kept in scen | CLI vs runtime camera-set asymmetry for label dept |
| b:1713 | 1442 | 515 | 0 | Y | SM | plan: `scene_preloading.md` | GLB preloading `PreloadPrefab`/`PreloadGlb` |
| b:1731 | 2012 | 666 | 0 | N | CAP | plan: `particle_system_v2.md` | Particle pipeline warmup + `ParticleBudget` footgu |

Totals: was 160310 chars across 104 blocks; now 54444 chars in the crate parent and folder files; 77527 chars of verbatim topic-doc text in `docs/dev/`. The old file was 163,228 chars (about 41k est. tokens) loaded on every touch under `crates/ironhold_core/src/`; the parent that loads now is 13320 chars.
