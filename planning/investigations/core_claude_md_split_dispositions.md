# Disposition table: where each block went (Phase 1, destinations built so far)

_Generated from `core_claude_md_split_blocks.json` and the destination files on 2026-10-08. "Now" is the characters of the condensed section(s) in the destination file(s); "Was" is the original block in the base. "Narrative" says where the cut text lives: **topic doc** (written in Phase 2, full text), a **plan file** (verbatim append), **kept** (nothing cut) or **cut** (derivable, with the reason in the sidecar)._

| Block | Was | Now | Safety | Destination | Narrative | Title |
|---|---|---|---|---|---|---|
| b:35 | 1268 | 1105 | Y | SCH | plan: `done/action_deny_unknown_fields.md` | Adding new actions + `deny_unknown_fields` consequences |
| b:54 | 648 | 1028 | N | SCH | kept | RON struct vs tuple variant syntax |
| b:70 | 1455 | 963 | Y | SM | plan: `done/rules_to_state_machine_consolidation.md` | Conditions on rules: only `LogicState`; `EnterState` re |
| b:92 | 2031 | 1186 | Y | SM | plan: `done/nested_prefabs.md` | Composite/nested prefab spawning via `spawn_primitive_c |
| b:108 | 746 | 753 | Y | SCH | plan: `ui_flex_group.md` | `scene.ui` must be walked via `walk_ui_nodes` |
| b:111 | 981 | 697 | Y | SM | plan: `done/entity_logic.md` | Entity FSM overview, interpreter chain order, never byp |
| b:127 | 1488 | 0 | N | (not written yet) | cut: derivable from `action_substitution.rs` (`rewrite_self`); **hook `acti | Supported `{self}` targets (15 examples) |
| b:144 | 3709 | 1648 | Y | SM | plan: `done/monotonic_entity_id.md` | `{new_id}` semantics, collisions, resolved at executor |
| b:188 | 483 | 474 | N | SM | plan: `done/targeting_system.md` | `{target}` substitution via `CurrentTarget` |
| b:190 | 1382 | 0 | N | (not written yet), TOPIC:action-bar-input-routing | topic doc | Per-player targeting Phase 1: `PlayerTarget` vs `Curren |
| b:206 | 3801 | 0 | N | (not written yet), TOPIC:action-bar-input-routing | topic doc | Per-player action bars Phase 2: `owner_player`, `owns_s |
| b:247 | 733 | 786 | Y | CAP, SM | plan: `done/per_player_split_screen_targeting.md` | `action_bar_input_system` requires `PlayerTarget` on ev |
| b:256 | 2466 | 0 | N | (not written yet), TOPIC:action-bar-input-routing | topic doc | Gamepad-routed action-bar slots (`gamepad_key`, `BoundG |
| b:284 | 2333 | 932 | Y | CAP, TOPIC:action-bar-input-routing | topic doc | Mouse-click action-bar slots (`Option<Ref<Interaction>> |
| b:286 | 2414 | 1149 | Y | CAP | plan: `done/per_player_split_screen_targeting.md` | Targeting chain ordering `.before(action_bar_input_syst |
| b:323 | 2353 | 883 | N | CAP | plan: `done/per_player_split_screen_targeting.md` | `target.*` events, screen-space selection, `select_aim_ |
| b:352 | 2404 | 0 | N | (not written yet), TOPIC:split-screen-cameras-and-widgets | topic doc | Target indicator ring, per-player, colour precedence |
| b:383 | 1180 | 0 | N | (not written yet), TOPIC:split-screen-cameras-and-widgets | topic doc | Per-viewport ring visibility: RenderLayers intro |
| b:396 | 802 | 0 | N | (not written yet), TOPIC:split-screen-cameras-and-widgets | topic doc | Orbit camera / ring layer assignment |
| b:406 | 710 | 396 | N | CAP | plan: `done/per_viewport_target_ring_visibility.md` | Party camera must carry `all_ring_layers()` |
| b:414 | 870 | 0 | N | (not written yet), TOPIC:split-screen-cameras-and-widgets | topic doc | `TargetRingVisibilityMode` lifecycle |
| b:424 | 748 | 0 | N | (not written yet) | cut: derivable from the two warn! sites (spawn_players_and_camera, drain_sp | Warnings: `player_index` collision, non-hot-join Spawn |
| b:433 | 408 | 176 | N | CAP | plan: `done/per_viewport_target_ring_visibility.md` | `pipeline_warmup_system` doesn't touch RenderLayers |
| b:439 | 586 | 355 | N | CAP | plan: `done/per_viewport_target_ring_visibility.md` | Light visibility intersects light RenderLayers (shadowl |
| b:446 | 369 | 0 | N | (not written yet) | cut: grep gives the same | "Reader-facing entry points" for the ring feature |
| b:452 | 1032 | 204 | N | CAP, TOPIC:split-screen-cameras-and-widgets | topic doc | Per-viewport target HUD readout |
| b:464 | 323 | 161 | N | SM | plan: `done/entity_logic.md` | `behavior` works on composite primitive prefabs |
| b:468 | 394 | 0 | N | (not written yet) | cut: covered by docs/20 | `TriggerZone` description |
| b:472 | 1222 | 643 | N | CAP | plan: `done/item_gated_interactable.md` | `Interactable` fires for every in-range interactable |
| b:478 | 880 | 0 | N | (not written yet), TOPIC:lootable-corpse | topic doc | Lootable corpse design summary |
| b:489 | 1567 | 715 | Y | SM | plan: `done/monster_corpse_loot.md` | `Action::Spawn.at_entity` copies full transform; skip w |
| b:507 | 1497 | 0 | N | (not written yet) | cut: history of a retrofit; the one current-state line moves into TOPIC:loo | Corpse ids unique via `{new_id}` (retrofit history) |
| b:524 | 1901 | 399 | N | CAP | plan: `done/monster_corpse_loot.md` | Prefer `SetDespawnTimer` over `EmitEventAfterDelay`+`De |
| b:545 | 682 | 299 | N | CAP | plan: `done/monster_corpse_loot.md` | `target_auto_clear_system` clears on despawn |
| b:554 | 648 | 329 | N | SM | plan: `done/monster_corpse_loot.md` | `Action::Despawn` closes an open container panel |
| b:563 | 1055 | 0 | N | (not written yet), TOPIC:lootable-corpse | topic doc | A dying entity can't catch its own respawn timer; globa |
| b:575 | 1026 | 0 | Y | (not written yet), TOPIC:lootable-corpse | topic doc | Respawn rules must be in `global_on`, not state-scoped  |
| b:587 | 944 | 630 | N | SM | plan: `done/monster_corpse_loot.md` | `OpenContainer` must not double-count `panels_open` |
| b:598 | 800 | 485 | Y | SM | plan: `done/monster_corpse_loot.md` | No `trigger_zone` on a Dynamic rigid-body prefab |
| b:608 | 1141 | 0 | N | (not written yet) | cut: the surviving reason lives in the 524-543 rule | Superseded same-entity corpse design (cautionary) |
| b:622 | 1179 | 0 | N | (not written yet), TOPIC:animation-pipeline | topic doc | Animation resolver/playback pipeline, field-ownership t |
| b:638 | 706 | 0 | N | (not written yet), TOPIC:animation-pipeline | topic doc | `pending_seek` purpose |
| b:647 | 846 | 321 | N | CAP | plan: `done/dynamic_animation_control.md` | Paused clip must be resumed before next play |
| b:658 | 882 | 289 | N | CAP | plan: `done/dynamic_animation_control.md` | `set_seek_time` not `seek_to`; duration via the live gr |
| b:670 | 539 | 281 | Y | CAP | plan: `done/dynamic_animation_control.md` | `ActiveOverride.seek_fraction`/`frozen` must be durable |
| b:677 | 639 | 0 | N | (not written yet), TOPIC:animation-pipeline | topic doc | Spawn-already-posed needs a minimal `AnimationPolicy` |
| b:686 | 1381 | 789 | N | CAP | plan: `done/dialogue_system.md` | Dialogue system |
| b:759 | 567 | 330 | N | CAP | plan: `deterministic_fixed_timestep.md` | "Confirmed fine, deliberately unconverted" list |
| b:765 | 216 | 141 | N | CAP | plan: `deterministic_fixed_timestep.md` | Faster-than-64Hz displays step motion (accepted) |
| b:769 | 797 | 462 | N | SCH | plan: `deterministic_fixed_timestep.md` | `ProjectConfig.max_frame_delta_secs` caps catch-up tick |
| b:779 | 3000 | 0 | N | (not written yet), TOPIC:player-ground-detection | topic doc | Jump reset can't rely on a ground-check edge; slope wal |
| b:815 | 3344 | 0 | N | (not written yet), TOPIC:player-ground-detection | topic doc | `jumps_used` reset: grace ticks, velocity, liftoff heig |
| b:856 | 2308 | 487 | Y | CAP, TOPIC:player-ground-detection | topic doc | Reset must read `raw_grounded`, never coyote-buffered ` |
| b:883 | 2375 | 0 | N | (not written yet), TOPIC:player-ground-detection | topic doc | Coyote time: debounced grounding |
| b:912 | 1473 | 397 | Y | CAP, TOPIC:player-ground-detection | topic doc | `can_jump`: coyote buffer unlocks only the first jump |
| b:929 | 1857 | 598 | Y | CAP, TOPIC:player-ground-detection | topic doc | Ground shape-cast must `.exclude_sensors()` and `normal |
| b:950 | 4605 | 0 | N | (not written yet), TOPIC:player-ground-detection | topic doc | `ground_cast` re-query loop, "underfoot", `is_walkable_ |
| b:1005 | 1411 | 0 | N | (not written yet), TOPIC:player-ground-detection | topic doc | `jump_air_grace_ticks` NaN clamp; accepted pogo consequ |
| b:1027 | 172 | 142 | Y | CAP | kept | Terrain generation is async |
| b:1053 | 725 | 430 | N | SM | plan: `done/audio_mute_toggle.md` | Audio preloading: `preload_audio_system`, `LoadedAudioH |
| b:1060 | 910 | 796 | N | SM | plan: `done/audio_volume_readout.md` | `audio_volume_var_system` mirrors `AudioState` to a `Ga |
| b:1063 | 602 | 0 | N | (not written yet) | cut: already at docs/20:1928; add the decoder start-up-on-first-play (WASM) | SFX: no leading silence, WAV only |
| b:1067 | 1209 | 921 | Y | SM | plan: `done/spawn_site_consolidation.md` | `tag_spawned_entity` is the single spawn-metadata sourc |
| b:1121 | 6516 | 0 | N | (not written yet), TOPIC:player-spawn-sites | topic doc | Player-construction sites inventory (1-5) + post-dispat |
| b:1201 | 957 | 536 | N | SM | plan: `done/player_stat_widgets.md` | Player `stat_label`/`world_stat_bar` routing; 16-param  |
| b:1213 | 946 | 901 | N | SCH, SM | plan: `player_model_source_unification.md` | `PrefabDef.material` not auto-applied on the player pat |
| b:1224 | 3144 | 1215 | Y | CAP, SM, TOPIC:player-ground-detection | topic doc | Collider divergence; Friction 0.15 idle-only after the  |
| b:1261 | 6085 | 0 | N | (not written yet), TOPIC:split-screen-cameras-and-widgets | topic doc | Local co-op: Party, split, Grid, dynamic split, fallbac |
| b:1331 | 2371 | 575 | N | CAP | plan: `done/camera_modes.md` | Split prefabs need `zoom_speed:0`, `orbit_button:"None" |
| b:1359 | 1347 | 517 | N | CAP | plan: `done/per_player_camera_look_controls.md` | Keyboard camera look |
| b:1374 | 719 | 314 | N | CAP | plan: `done/camera_modes.md` | `CameraShake` query must exclude Fixed/FirstPerson/Flyc |
| b:1383 | 1948 | 1151 | Y | CAP | plan: `done/camera_modes.md` | `SetCameraMode`; Authored vs Active camera mode; `camer |
| b:1406 | 4956 | 706 | N | CAP, TOPIC:split-screen-cameras-and-widgets | topic doc | `WorldLabelRank` viewport-aware labels; per-rank widget |
| b:1463 | 1072 | 0 | N | (not written yet), TOPIC:split-screen-cameras-and-widgets | topic doc | `particle_renderer` billboard viewport-aware (Phase 1) |
| b:1476 | 1028 | 213 | N | CAP, TOPIC:split-screen-cameras-and-widgets | topic doc | Targeting `click_select` viewport-aware (Phase 2) |
| b:1488 | 2057 | 273 | N | CAP, TOPIC:split-screen-cameras-and-widgets | topic doc | Nameplate distance culling via stored camera distance ( |
| b:1511 | 1688 | 0 | N | (not written yet), TOPIC:split-screen-cameras-and-widgets | topic doc | Split-screen player HUD labels (P{n}) |
| b:1530 | 4152 | 2254 | Y | CAP, RT, TOPIC:gamepad-routing | topic doc | Gamepad routing: `BoundGamepad`, `claimed` invariant |
| b:1576 | 1082 | 0 | N | (not written yet), TOPIC:gamepad-routing | topic doc | Gamepad button/axis RON mapping, parity gap |
| b:1589 | 1396 | 743 | N | SCH | plan: `done/local_coop_foundation.md` | Duplicate `gamepad_index`/`player_index` validation sco |
| b:1606 | 2864 | 1447 | Y | RT, SM, TOPIC:gamepad-routing | topic doc | Gamepad-triggered hot join: `PendingJoinGamepad` |
| b:1638 | 623 | 399 | N | CAP | plan: `done/local_coop_4way_split.md` | View-box clamp |
| b:1646 | 936 | 607 | Y | SM | plan: `done/dynamic_spawn_components.md` | Spawn queue: `SPAWNS_PER_FRAME = 2` |
| b:1657 | 3944 | 1870 | N | SM | plan: `done/dynamic_spawn_components.md` | Component parity: dynamic vs scene-placed (nameplate ga |
| b:1668 | 3844 | 1524 | N | SM | plan: `done/label_depth_scale_validation.md` | Label depth scale + validation coverage |
| b:1695 | 1566 | 0 | N | (not written yet) | cut: someone may "fix" the asymmetry | CLI vs runtime camera-set asymmetry for label depth sca |
| b:1731 | 2012 | 666 | N | CAP | plan: `particle_system_v2.md` | Particle pipeline warmup + `ParticleBudget` footgun |

Totals (blocks not in the parent): was 140476 chars, now 38691 chars in folder files (topic-doc halves are not counted until Phase 2).
