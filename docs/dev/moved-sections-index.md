# Moved sections: where the old `crates/ironhold_core/src/CLAUDE.md` went

On 2026-10-08 the 1,752-line `crates/ironhold_core/src/CLAUDE.md` was split by directory and topic (`planning/features/core_claude_md_split.md`). Old plans, agent memory and commit messages cite its sections by name or line number; this page keeps them resolvable.

- **By line number (old plans):** use the second table. Its line numbers are those of `git show ba4088d:crates/ironhold_core/src/CLAUDE.md`, the file as it stood when those plans were written; block ids (`b:N`) are named after the first of those lines.
- **By section name:** use the first table (headings and line numbers of `git show d222d91:crates/ironhold_core/src/CLAUDE.md`, the file after the small drift fixes).
- Per-block sizes and the narrative archive: `planning/investigations/core_claude_md_split_dispositions.md`; condensed narrative is kept verbatim in the owning feature plan under "Notes moved from `crates/ironhold_core/src/CLAUDE.md`".

## By old section heading

| Old section | Lines | Now at |
|---|---|---|
| # ironhold_core — Rust Source Rules | 1-2 | parent `src/CLAUDE.md` |
| ## The Message → Interpreter → Action → Executor pipeline | 3-16 | parent `src/CLAUDE.md` |
| ### Rules for new capabilities | 17-34 | parent `src/CLAUDE.md` |
| ### Adding new actions | 35-53 | `schema/CLAUDE.md` |
| ### RON action syntax — struct vs tuple variants | 54-69 | `schema/CLAUDE.md` |
| ### Conditions on rules | 70-82 | `runtime/scene_manager/CLAUDE.md`; parent `src/CLAUDE.md` |
| ## Before coding | 83-87 | parent `src/CLAUDE.md` |
| ### Color field convention | 88-93 | parent `src/CLAUDE.md`; `runtime/scene_manager/CLAUDE.md` |
| ## Composite and nested prefab spawning | 94-112 | `runtime/scene_manager/CLAUDE.md`; parent `src/CLAUDE.md`; `schema/CLAUDE.md` |
| ## Entity FSM (per-entity behavior) | 113-621 | `runtime/scene_manager/CLAUDE.md`; cut (derivable or duplicate; text archived in the owning plan); [`docs/dev/action-bar-input-routing.md`](action-bar-input-routing.md); `capabilities/CLAUDE.md`; [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md); [`docs/dev/lootable-corpse.md`](lootable-corpse.md); parent `src/CLAUDE.md` |
| ### Animation resolver/playback pipeline (`capabilities/animation_resolver.rs` + `capabilities/animation.rs`) | 622-685 | [`docs/dev/animation-pipeline.md`](animation-pipeline.md); `capabilities/CLAUDE.md` |
| ### Dialogue system (`capabilities/dialogue.rs`) | 686-705 | `capabilities/CLAUDE.md` |
| ## WebGPU 16-byte alignment | 706-710 | parent `src/CLAUDE.md` |
| ## WGSL is the first-class shader language | 711-730 | `assets/CLAUDE.md`; parent `src/CLAUDE.md` |
| ## Physics & movement must use `FixedUpdate` | 731-778 | parent `src/CLAUDE.md`; `capabilities/CLAUDE.md`; `schema/CLAUDE.md` |
| ### Jump reset cannot rely on a ground-check edge (`planning/features/uphill_jump_lock.md`) | 779-882 | [`docs/dev/player-ground-detection.md`](player-ground-detection.md); `capabilities/CLAUDE.md` |
| ### Coyote time — debounced grounding for uneven terrain (`planning/features/uphill_jump_lock.md`) | 883-1022 | [`docs/dev/player-ground-detection.md`](player-ground-detection.md); `capabilities/CLAUDE.md` |
| ## Deterministic iteration order on gameplay paths | 1023-1026 | parent `src/CLAUDE.md` |
| ## Terrain generation is async | 1027-1029 | `capabilities/CLAUDE.md` |
| ## Inspector isolation | 1030-1032 | parent `src/CLAUDE.md` |
| ## Frame pacing and performance | 1033-1052 | parent `src/CLAUDE.md` |
| ## Audio | 1053-1054 | `runtime/scene_manager/CLAUDE.md` |
| ### Preloading | 1055-1059 | `runtime/scene_manager/CLAUDE.md` |
| ### Live volume variable (`audio_volume_var_system`) | 1060-1062 | `runtime/scene_manager/CLAUDE.md` |
| ### Audio file authoring | 1063-1067 | cut (derivable or duplicate; text archived in the owning plan) |
| ## Spawning: standard entity metadata | 1068-1084 | `runtime/scene_manager/CLAUDE.md` |
| ## Despawning: prefer `try_despawn()` when an entity may already be gone | 1085-1121 | parent `src/CLAUDE.md` |
| ### Player-construction sites | 1122-1261 | `runtime/scene_manager/CLAUDE.md`; [`docs/dev/player-spawn-sites.md`](player-spawn-sites.md); `schema/CLAUDE.md`; `capabilities/CLAUDE.md`; [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| ### Local co-op: shared camera, split-screen, gamepad routing, view-box clamp | 1262-1646 | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md); `capabilities/CLAUDE.md`; [`docs/dev/gamepad-routing.md`](gamepad-routing.md); `runtime/CLAUDE.md`; `schema/CLAUDE.md`; `runtime/scene_manager/CLAUDE.md` |
| ## Dynamic spawning | 1647-1648 | `runtime/scene_manager/CLAUDE.md` |
| ### Spawn queue | 1649-1657 | `runtime/scene_manager/CLAUDE.md` |
| ### Component parity with scene-placed entities | 1658-1713 | `runtime/scene_manager/CLAUDE.md`; cut (derivable or duplicate; text archived in the owning plan) |
| ### GLB preloading | 1714-1731 | `runtime/scene_manager/CLAUDE.md`; a `docs/` page |
| ### Particle pipeline warmup | 1732-1753 | `capabilities/CLAUDE.md` |

## By old line number

| Block | Old lines | What | Now at |
|---|---|---|---|
| `b:1` | 1-34 | Message→Interpreter→Action→Executor pipeline; sensors never push actio | parent `src/CLAUDE.md` |
| `b:35` | 35-53 | Adding new actions + `deny_unknown_fields` consequences | `schema/CLAUDE.md` |
| `b:54` | 54-69 | RON struct vs tuple variant syntax | `schema/CLAUDE.md` |
| `b:70` | 70-80 | Conditions on rules: only `LogicState`; `EnterState` removed | `runtime/scene_manager/CLAUDE.md` |
| `b:81` | 81-91 | No hardcoded assets; sRGB colours only | parent `src/CLAUDE.md` |
| `b:92` | 92-107, 109-110 | Composite/nested prefab spawning via `spawn_primitive_children` | `runtime/scene_manager/CLAUDE.md` |
| `b:108` | 108 | `scene.ui` must be walked via `walk_ui_nodes` | parent `src/CLAUDE.md`; `schema/CLAUDE.md` |
| `b:111` | 111-126 | Entity FSM overview, interpreter chain order, never bypass pipeline | `runtime/scene_manager/CLAUDE.md` |
| `b:127` | 127-143 | Supported `{self}` targets (15 examples) | cut (derivable or duplicate; text archived in the owning plan) |
| `b:144` | 144-187 | `{new_id}` semantics, collisions, resolved at executor | `runtime/scene_manager/CLAUDE.md` |
| `b:188` | 188-189 | `{target}` substitution via `CurrentTarget` | `runtime/scene_manager/CLAUDE.md` |
| `b:190` | 190-205 | Per-player targeting Phase 1: `PlayerTarget` vs `CurrentTarget` | [`docs/dev/action-bar-input-routing.md`](action-bar-input-routing.md) |
| `b:206` | 206-246 | Per-player action bars Phase 2: `owner_player`, `owns_slot`, cost pool | [`docs/dev/action-bar-input-routing.md`](action-bar-input-routing.md) |
| `b:247` | 247-255 | `action_bar_input_system` requires `PlayerTarget` on every `CharacterC | `capabilities/CLAUDE.md`; `runtime/scene_manager/CLAUDE.md` |
| `b:256` | 256-283 | Gamepad-routed action-bar slots (`gamepad_key`, `BoundGamepad`) | [`docs/dev/action-bar-input-routing.md`](action-bar-input-routing.md) |
| `b:284` | 284 | Mouse-click action-bar slots (`Option<Ref<Interaction>>`, `InspectorEn | `capabilities/CLAUDE.md`; [`docs/dev/action-bar-input-routing.md`](action-bar-input-routing.md) |
| `b:286` | 286-314 | Targeting chain ordering `.before(action_bar_input_system)`; transitiv | `capabilities/CLAUDE.md` |
| `b:315` | 315-322 | `Targetable` test fixtures must register in `SpawnRegistry` | cut (derivable or duplicate; text archived in the owning plan) |
| `b:323` | 323-351 | `target.*` events, screen-space selection, `select_aim_height`, `targe | `capabilities/CLAUDE.md` |
| `b:352` | 352-382 | Target indicator ring, per-player, colour precedence | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md); `capabilities/CLAUDE.md` |
| `b:383` | 383-395 | Per-viewport ring visibility: RenderLayers intro | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md) |
| `b:396` | 396-405 | Orbit camera / ring layer assignment | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md) |
| `b:406` | 406-413 | Party camera must carry `all_ring_layers()` | `capabilities/CLAUDE.md` |
| `b:414` | 414-423 | `TargetRingVisibilityMode` lifecycle | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md) |
| `b:424` | 424-431 | Warnings: `player_index` collision, non-hot-join Spawn | cut (derivable or duplicate; text archived in the owning plan) |
| `b:433` | 433-437 | `pipeline_warmup_system` doesn't touch RenderLayers | `capabilities/CLAUDE.md` |
| `b:439` | 439-444 | Light visibility intersects light RenderLayers (shadowless lit mesh) | `capabilities/CLAUDE.md` |
| `b:446` | 446-450 | "Reader-facing entry points" for the ring feature | cut (derivable or duplicate; text archived in the owning plan) |
| `b:452` | 452-462 | Per-viewport target HUD readout | `capabilities/CLAUDE.md`; [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md) |
| `b:464` | 464-466 | `behavior` works on composite primitive prefabs | `runtime/scene_manager/CLAUDE.md` |
| `b:468` | 468-471 | `TriggerZone` description | cut (derivable or duplicate; text archived in the owning plan) |
| `b:472` | 472-476 | `Interactable` fires for every in-range interactable | `capabilities/CLAUDE.md` |
| `b:478` | 478-488 | Lootable corpse design summary | [`docs/dev/lootable-corpse.md`](lootable-corpse.md) |
| `b:489` | 489-505 | `Action::Spawn.at_entity` copies full transform; skip with warning, ne | `runtime/scene_manager/CLAUDE.md` |
| `b:507` | 507-522 | Corpse ids unique via `{new_id}` (retrofit history) | cut (derivable or duplicate; text archived in the owning plan) |
| `b:524` | 524-543 | Prefer `SetDespawnTimer` over `EmitEventAfterDelay`+`Despawn` | `capabilities/CLAUDE.md` |
| `b:545` | 545-552 | `target_auto_clear_system` clears on despawn | `capabilities/CLAUDE.md` |
| `b:554` | 554-561 | `Action::Despawn` closes an open container panel | `runtime/scene_manager/CLAUDE.md` |
| `b:563` | 563-573 | A dying entity can't catch its own respawn timer; global rule needed | [`docs/dev/lootable-corpse.md`](lootable-corpse.md) |
| `b:575` | 575-585 | Respawn rules must be in `global_on`, not state-scoped `on:` | [`docs/dev/lootable-corpse.md`](lootable-corpse.md); parent `src/CLAUDE.md` |
| `b:587` | 587-596 | `OpenContainer` must not double-count `panels_open` | `runtime/scene_manager/CLAUDE.md` |
| `b:598` | 598-606 | No `trigger_zone` on a Dynamic rigid-body prefab | `runtime/scene_manager/CLAUDE.md` |
| `b:608` | 608-620 | Superseded same-entity corpse design (cautionary) | cut (derivable or duplicate; text archived in the owning plan) |
| `b:622` | 622-637 | Animation resolver/playback pipeline, field-ownership table | [`docs/dev/animation-pipeline.md`](animation-pipeline.md) |
| `b:638` | 638-646 | `pending_seek` purpose | [`docs/dev/animation-pipeline.md`](animation-pipeline.md) |
| `b:647` | 647-657 | Paused clip must be resumed before next play | `capabilities/CLAUDE.md` |
| `b:658` | 658-669 | `set_seek_time` not `seek_to`; duration via the live graph | `capabilities/CLAUDE.md` |
| `b:670` | 670-675 | `ActiveOverride.seek_fraction`/`frozen` must be durable | `capabilities/CLAUDE.md` |
| `b:677` | 677-684 | Spawn-already-posed needs a minimal `AnimationPolicy` | [`docs/dev/animation-pipeline.md`](animation-pipeline.md) |
| `b:686` | 686-702 | Dialogue system | `capabilities/CLAUDE.md` |
| `b:706` | 706-709 | WebGPU 16-byte alignment for GPU structs | parent `src/CLAUDE.md` |
| `b:711` | 711-720 | WGSL authoring rules for designer-facing shaders | `assets/CLAUDE.md` |
| `b:722` | 722-728 | Engine-internal shaders embedded via `include_str!`/`uuid_handle!`; no | parent `src/CLAUDE.md` |
| `b:729` | 729-730 | Pointer to `docs/25_custom_shaders.md` | parent `src/CLAUDE.md` |
| `b:731` | 731-733 | Physics/movement must run in `FixedUpdate` (camera follow/blend and an | parent `src/CLAUDE.md` |
| `b:734` | 734-749 | Rapier steps in `FixedUpdate` at 64Hz; multi-tick frames; `mark_dirty_ | parent `src/CLAUDE.md` |
| `b:750` | 750-758 | Update systems must not read `&GlobalTransform`; use `utils::fresh_glo | parent `src/CLAUDE.md` |
| `b:759` | 759-764 | "Confirmed fine, deliberately unconverted" list | `capabilities/CLAUDE.md` |
| `b:765` | 765-768 | Faster-than-64Hz displays step motion (accepted) | `capabilities/CLAUDE.md` |
| `b:769` | 769-777 | `ProjectConfig.max_frame_delta_secs` caps catch-up ticks | `schema/CLAUDE.md` |
| `b:779` | 779-814 | Jump reset can't rely on a ground-check edge; slope walkability gate | [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| `b:815` | 815-855 | `jumps_used` reset: grace ticks, velocity, liftoff height; dual-clock  | [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| `b:856` | 856-882 | Reset must read `raw_grounded`, never coyote-buffered `is_grounded` | `capabilities/CLAUDE.md`; [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| `b:883` | 883-911 | Coyote time: debounced grounding | [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| `b:912` | 912-928 | `can_jump`: coyote buffer unlocks only the first jump | `capabilities/CLAUDE.md`; [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| `b:929` | 929-949 | Ground shape-cast must `.exclude_sensors()` and `normalize_or_zero()` | `capabilities/CLAUDE.md`; [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| `b:950` | 950-1004 | `ground_cast` re-query loop, "underfoot", `is_walkable_contact`, known | [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| `b:1005` | 1005-1022 | `jump_air_grace_ticks` NaN clamp; accepted pogo consequences | [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| `b:1023` | 1023-1026 | Deterministic iteration order: BTreeMap/IndexMap; `determinism_lint` m | parent `src/CLAUDE.md` |
| `b:1027` | 1027-1029 | Terrain generation is async | `capabilities/CLAUDE.md` |
| `b:1030` | 1030-1032 | Inspector isolation (`cfg_attr(feature = "inspector")`) | parent `src/CLAUDE.md` |
| `b:1033` | 1033-1040 | Native frame cap, unfocused throttle, pipeline warmup | parent `src/CLAUDE.md` |
| `b:1041` | 1041-1052 | Change-detection discipline for render-affecting components | parent `src/CLAUDE.md` |
| `b:1053` | 1053-1059 | Audio preloading: `preload_audio_system`, `LoadedAudioHandles` | `runtime/scene_manager/CLAUDE.md` |
| `b:1060` | 1060-1062 | `audio_volume_var_system` mirrors `AudioState` to a `GameVariable` | `runtime/scene_manager/CLAUDE.md` |
| `b:1063` | 1063-1066 | SFX: no leading silence, WAV only | cut (derivable or duplicate; text archived in the owning plan) |
| `b:1067` | 1067-1083 | `tag_spawned_entity` is the single spawn-metadata source | `runtime/scene_manager/CLAUDE.md` |
| `b:1084` | 1084-1110 | Prefer `try_despawn()` when an entity may already be gone | parent `src/CLAUDE.md` |
| `b:1111` | 1111-1120 | Child `LevelEntity`/`OverlayEntity` self-tagging is deliberate | parent `src/CLAUDE.md` |
| `b:1121` | 1121-1200 | Player-construction sites inventory (1-5) + post-dispatch rule | `runtime/scene_manager/CLAUDE.md`; [`docs/dev/player-spawn-sites.md`](player-spawn-sites.md) |
| `b:1201` | 1201-1212 | Player `stat_label`/`world_stat_bar` routing; 16-param ceiling | `runtime/scene_manager/CLAUDE.md` |
| `b:1213` | 1213-1223 | `PrefabDef.material` not auto-applied on the player path | `schema/CLAUDE.md`; `runtime/scene_manager/CLAUDE.md` |
| `b:1224` | 1224-1260 | Collider divergence; Friction 0.15 idle-only after the wall-friction f | `capabilities/CLAUDE.md`; `runtime/scene_manager/CLAUDE.md`; [`docs/dev/player-ground-detection.md`](player-ground-detection.md) |
| `b:1261` | 1261-1330 | Local co-op: Party, split, Grid, dynamic split, fallback | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md) |
| `b:1331` | 1331-1358 | Split prefabs need `zoom_speed:0`, `orbit_button:"None"`, `character_r | `capabilities/CLAUDE.md` |
| `b:1359` | 1359-1373 | Keyboard camera look | `capabilities/CLAUDE.md` |
| `b:1374` | 1374-1382 | `CameraShake` query must exclude Fixed/FirstPerson/Flycam | `capabilities/CLAUDE.md` |
| `b:1383` | 1383-1405 | `SetCameraMode`; Authored vs Active camera mode; `camera_blend_system` | `capabilities/CLAUDE.md` |
| `b:1406` | 1406-1462 | `WorldLabelRank` viewport-aware labels; per-rank widget duplication | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md); `capabilities/CLAUDE.md` |
| `b:1463` | 1463-1474 | `particle_renderer` billboard viewport-aware (Phase 1) | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md) |
| `b:1476` | 1476-1486 | Targeting `click_select` viewport-aware (Phase 2) | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md); `capabilities/CLAUDE.md` |
| `b:1488` | 1488-1509 | Nameplate distance culling via stored camera distance (Phase 3) | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md); `capabilities/CLAUDE.md` |
| `b:1511` | 1511-1528 | Split-screen player HUD labels (P{n}) | [`docs/dev/split-screen-cameras-and-widgets.md`](split-screen-cameras-and-widgets.md) |
| `b:1530` | 1530-1574 | Gamepad routing: `BoundGamepad`, `claimed` invariant | [`docs/dev/gamepad-routing.md`](gamepad-routing.md); `capabilities/CLAUDE.md`; `runtime/CLAUDE.md` |
| `b:1576` | 1576-1587 | Gamepad button/axis RON mapping, parity gap | [`docs/dev/gamepad-routing.md`](gamepad-routing.md) |
| `b:1589` | 1589-1604 | Duplicate `gamepad_index`/`player_index` validation scope | `schema/CLAUDE.md` |
| `b:1606` | 1606-1636 | Gamepad-triggered hot join: `PendingJoinGamepad` | [`docs/dev/gamepad-routing.md`](gamepad-routing.md); `runtime/CLAUDE.md`; `runtime/scene_manager/CLAUDE.md` |
| `b:1638` | 1638-1644 | View-box clamp | `capabilities/CLAUDE.md` |
| `b:1646` | 1646-1655 | Spawn queue: `SPAWNS_PER_FRAME = 2` | `runtime/scene_manager/CLAUDE.md` |
| `b:1657` | 1657-1667 | Component parity: dynamic vs scene-placed (nameplate gating) | `runtime/scene_manager/CLAUDE.md` |
| `b:1668` | 1668-1693 | Label depth scale + validation coverage | `runtime/scene_manager/CLAUDE.md` |
| `b:1695` | 1695-1711 | CLI vs runtime camera-set asymmetry for label depth scale | cut (derivable or duplicate; text archived in the owning plan) |
| `b:1713` | 1713-1729 | GLB preloading `PreloadPrefab`/`PreloadGlb` | `runtime/scene_manager/CLAUDE.md`; a `docs/` page |
| `b:1731` | 1731-1752 | Particle pipeline warmup + `ParticleBudget` footgun | `capabilities/CLAUDE.md` |
