# Backlog

> **How this works**
> - Items progress: `Icebox → Queued → Active → Done`
> - Simple items live here as bullet points. Anything needing design lives in `features/`.
> - This file tracks *what to build next*, not *how* — keep it skimmable.
> - Roadmap and milestone gates: see `docs/50_roadmap_and_milestones.md`
> - Implementation status: see `docs/STATUS.md`

---

## Active

- [x] **Stylized foliage (anime / Ghibli-style trees)** — `kind: Foliage` prefab type; procedural leaf card clusters with camera-facing billboard vertex shader; sphere-mapped normals for unbroken toon shading volumes; alpha-clip brush-stroke texture; `FoliageMaterial` WGSL shader; `height_bias` + `seed` for crown shape control. See `planning/features/done/stylized_foliage.md`
- [x] **Skill action bar (1–9)** — `ActionBar` UI node; `ActionSlotUi` + `CooldownOverlay` components; `CooldownMap` + `CurrentTarget` resources; keys 1–9 fire `do_actions`, check cooldown, deduct stat cost; `action_bar.*` pipeline events; alpha-fade cooldown overlay; `ShowFloatingText` action; demo in `primitive_world`. See `planning/features/done/skill_action_bar.md`
- [ ] **Foliage demo — visual tuning** — refine cluster placement, leaf scale, and per-prefab parameters in `foliage_demo` to match the reference image quality. Blocked on live RON reload or live editor — parameter tuning without a tight feedback loop is impractical. Pick up after either ships.
- [ ] **Mouse click-to-select + Tab targeting** — `click_selectable: true` / `targetable: true` on `PrefabDef`; shared `CurrentTarget` resource; `bevy::picking` mesh raycast for click; Tab/Shift-Tab cycles nearest-first; `{target}` substitution in behavior files, rules, and skill bar slots; `ProjectDecal` selection ring; `target.*` events into pipeline. See `planning/features/targeting_system.md`

---

## Bugs

- [x] **Foliage square shadows** — `FoliageMaterial` cluster entities cast square shadows because Bevy's shadow depth pass ignores the material's alpha discard, rendering the full quad silhouette. Reproduce: `foliage_demo`, look at tree shadows on the ground. **Cheap fix:** insert `NotShadowCaster` on each cluster entity in `foliage_setup_system` — removes shadows entirely, which is the standard stylised-foliage approach. **Proper fix:** override the depth prepass shader on `FoliageMaterial` to also sample the leaf texture and discard on alpha < 0.5; shadow map resolution will still blur the result.
- [ ] **uphill jump lock** — when jumping against an uphill slope, the player can land in a state where `jump` never re-triggers: the character controller reports ground contact but the slope normal keeps the jump cooldown active. Suspected cause: Rapier's ground-contact normal threshold in the character controller or the jump cooldown not resetting when sliding contact ends. Reproduce: 3rd_person_game_demo, run toward any hill and spam jump while ascending.
- [x] **`PrefabComponents` silently drops unknown fields** — added `#[serde(deny_unknown_fields)]`; RON parse now fails with a clear field name on typos or unknown fields; two regression tests added.

---

## Queued

### Camera
- [ ] **Camera modes** — unified data-driven camera system: `Orbit`, `Follow`, `FirstPerson`, `Fixed`, `Flycam` modes all tunable from RON; `SetCameraMode` action for runtime switching with optional eased transitions; FOV interpolation; backwards-compatible with existing `camera:` / `flycam:` prefab fields. See `planning/features/camera_modes.md`

### Gameplay & Environment

- [ ] **Status effect icon display** — HUD and/or above-entity icon strip showing active buffs and debuffs; icons are asset catalog texture keys declared on modifier templates; strip updates via change detection on `ActiveModifiers`; designer controls position (HUD panel vs. world-space above entity) and max visible icons in scene RON. See `planning/features/status_effect_icons.md`
- [ ] **Layered icon UI node** — new `LayeredIcon` UI node type; each layer declares a texture key, tint color (r,g,b,a), and opacity; layers are alpha-composited in declaration order (bottom → top); v1 alpha-stack only — additive blend mode deferred to a future `blend:` field per layer; feeds action bar slot icons and status effect icon strips directly.
- [ ] **AoE ground targeting** — `TargetingMode: GroundAoE(radius)` on skill action bar slots; pressing the slot enters a placement mode showing a circle decal under the cursor; confirming fires the slot's `do_actions` with `{aoe_position}` substitution; cancelling (right-click / Escape) exits without firing. See `planning/features/aoe_ground_targeting.md` _Hard dep: Skill action bar._
- [ ] **Nameplate system** — floating name + health bar above entities, scene-wide opt-in (`show_nameplates: true` in scene RON) with per-prefab override; visibility filtered by faction stance (hostile / friendly / all) and optional max distance; distinct from per-entity world-space stat bars — nameplates are managed by a single system scanning all tagged entities. See `planning/features/nameplate_system.md`
- [ ] **Spawn wave / encounter system** — `WaveDef` in RON: an ordered sequence of spawn steps each with a prefab key, count, delay, and optional position list; fires on an event (`StartWave("wave_01")`), emits `wave.complete:{id}` when all spawned entities are dead; supports looping waves and inter-wave delays. Designer-friendly alternative to scripting individual `Spawn` + `EmitEventAfterDelay` chains. See `planning/features/spawn_wave_encounter.md`
- [ ] **Day/night cycle** — `DayNightCycleDef` in scene RON: cycle duration, sun color/intensity keyframes at dawn/noon/dusk/midnight; `TimeOfDay` resource drives directional light + ambient each frame; `SetTimeOfDay(hour)` and `SetDaySpeed(multiplier)` actions; emits `time.dawn` / `time.noon` / `time.dusk` / `time.midnight` events designers can hook; WASM compatible (pure CPU, no post-process). See `planning/features/day_night_cycle.md`
- [ ] **Sound zones** — ambient audio driven by player location; a new `kind: SoundZone` trigger zone variant with `audio_key`, `volume`, and `fade_distance` fields; entering the zone fades in the audio, leaving fades it out; defined entirely in scene RON using the existing trigger zone + `PlayMusicLoop`/`StopMusic` actions, no new systems needed beyond the fade envelope.
- [ ] **Camera shake** — `Action::CameraShake { duration_secs, intensity }` applies a procedural position shake to the active camera; designer fires it from any rule or behavior file.

### Particle System v2

- [ ] **Bloom / post-processing in scene RON** — WASM-BLOCKED: Bevy's `Bloom` requires `#[require(Hdr)]`; HDR breaks the WASM build. Parked until performant HDR/post-process support is available in Bevy's WebGPU backend. Do not implement a native-only workaround — that splits the runtime model. See `planning/features/particle_bloom.md`
- [x] **Dynamic effect lights** — `light` block on EffectDef spawns a temporary fading PointLight. See `planning/features/particle_dynamic_lights.md`
- [x] **Extended particle behaviours** — rotation over lifetime, non-uniform scale, Ring/Sphere/Line/Arc emitters, velocity curves. See `planning/features/done/particle_extended_behaviours.md`
- [x] **Ground decals / AoE projections** — `ProjectDecal` action for AoE circles, impact splats, cast indicators. See `planning/features/done/particle_ground_decals.md`
- [x] **Flipbook / sprite sheet animation** — UV sub-rect baked per-frame in CPU pool renderer; `explosion_4x4.png` sheet; Flipbook Pad station in particles_demo. See `planning/features/done/particle_flipbook.md`
- [ ] **Shared effect library** — `assets/shared/effects/` with reusable effects and per-project overrides. See `planning/features/particle_shared_library.md`

### Rendering & Assets

- [ ] **LOD — pre-baked mesh swap** — distance-based LOD switching using offline-generated LOD GLB files; `lod_levels: [(distance, model)]` on `PrefabDef` declares swap thresholds; a system watches camera distance and swaps `SceneRoot` handle; LOD meshes generated offline (Blender / `meshopt`) and referenced in `assets.ron`; no runtime compute required — fully WASM-compatible. See `planning/features/lod_prebaked_mesh_swap.md`
- [ ] **Channel-packed ORM texture shader** — new WGSL shader variant (`custom_texture_triplanar_pbr_packed.wgsl`) that reads a single packed ORM texture (R=occlusion, G=roughness, B=metallic) instead of three separate textures; matches the default export format from Substance Painter; frees two sampler slots and halves texture bandwidth for PBR properties; no schema changes — designers swap the shader key and point one texture at `texture_1` instead of three.

### Beta 0.5 — Deterministic Tick + Replay
- [ ] Fixed-tick schedule for gameplay systems (separate from render tick)
- [ ] Deterministic RNG resource (seeded, replaces any `rand` usage in gameplay)
- [ ] `InputAction` stream capture to file (native)
- [ ] Replay playback from captured stream
- [ ] Snapshot/restore stub for core gameplay state
- [ ] Determinism constraints doc

### Beta 0.6 — Multiplayer Form 1: LAN Co-op
See `planning/features/networking_multiplayer.md`. Gate: Beta 0.5 (deterministic tick) must ship first.
- [ ] Library spike (Bevy Lightyear vs alternatives — see feature file pre-checks)
- [ ] `HostGame` / `JoinGame` / `DisconnectGame` actions
- [ ] Input replication: client sends `InputActionMessage` per tick; host authorises
- [ ] State replication: `Transform`, `AnimationState`, `StatMap`, `GameVariables`
- [ ] `multiplayer.*` pipeline events
- [ ] LAN co-op demo scene
- [ ] Network protocol doc + integration tests

### Beta 0.8 — Multiplayer Form 2: Internet Player-Hosted
See `planning/features/networking_multiplayer.md`. Gate: Beta 0.6 (LAN) must ship first.
- [ ] Relay/signaling service decision and deployment (Matchbox recommended — see feature file)
- [ ] WASM-compatible transport confirmed in browser
- [ ] Lobby system: `CreateLobby` / `JoinLobby` / `ListLobbies` actions
- [ ] `LobbyList` UI scene node
- [ ] Internet co-op demo (accessible from WASM build)
- [ ] Relay setup docs

### Beta 0.9+ — Multiplayer Form 3: Dedicated Server
See `planning/features/networking_multiplayer.md`. Gate: Beta 0.8 (internet listen server) must ship first. Requires a separate detailed feature file before coding.
- [ ] `ironhold_server` crate (headless Bevy, no render/window)
- [ ] `start_server()` entry point in `ironhold_core`
- [ ] Client-mode thin renderer (no local simulation)
- [ ] Server admin actions (kick, change_scene)
- [ ] Dedicated server demo + deployment guide

### Beta 0.7 — Loading & Preloading
- [ ] Loading screen overlay during `LoadingScene` / `LoadingProject` states
- [ ] `scene.loading_progress:{0-100}` milestone events from loader and terrain task
- [ ] `loading_scene` field in project config for custom splash scenes
- [ ] `preload_poll_system`: watch `PreloadedScenes` handles, emit `scene.preloaded:{name}`
- [ ] `LoadScene` fast-path when handle is already loaded in `PreloadedScenes`
- [ ] Docs + tests
- [ ] Design: `planning/features/loading_screen.md`, `planning/features/scene_preloading.md`

---

## Icebox

### Engine / Runtime
- [ ] Capability registry — declare events, actions, and validation rules per capability; replaces ad-hoc wiring
- [ ] Schema migrations — versioned upgrade paths with diagnostics on load failure
- [ ] **Gamepad / controller input** — wire Bevy's built-in gamepad input through the existing `InputAction` system and RON key bindings; map stick axes to movement/camera and face buttons to `InputAction` variants; designers declare gamepad bindings in the same input config block as keyboard; needed for web builds targeting controller users
- [ ] **Save / load game state** — `SaveGame` / `LoadGame` actions; serialize `GameVariables`, per-entity `StatMap`, and active modifier state to a JSON/RON file (native) or `localStorage` (WASM); `AutoSave` trigger on configurable events; scene transitions preserve state across loads. See `planning/features/save_load_game_state.md`
- [ ] **Input remapping** — let players rebind keyboard and gamepad actions at runtime via a settings UI; bindings persisted to a per-player config file (native) or `localStorage` (WASM); designer declares remappable actions and default bindings in project RON; depends on gamepad input feature for full coverage
- [ ] **`ChildOf` hierarchy migration** — migrate from `Children`/`Parent` (Bevy pre-0.16 API) to the `ChildOf` relationship component (Bevy 0.16+); the animation system queries `&Children` to walk GLB hierarchies and all spawners use `with_children()` — these need updating to the forward-looking API before a future Bevy upgrade removes the compat shim
- [ ] **Required components on project-defined components** — adopt `#[require(...)]` (Bevy 0.15+) on project-defined marker components (e.g. `TriggerZone`, `FadingLight`, `LevelEntity`) so that inserting the primary component automatically inserts its mandatory companions; reduces manual bundle construction in spawners and makes component contracts explicit at the type level
- [>] **Typed primitive shape field** — add `shape: PrimitiveShapeKind` (typed enum) to `PrefabDef` and promote `ChildPrimitiveDef.shape` from `String` to the same enum; `model:` must be empty for primitives; `PREFAB_CATALOG_SCHEMA_VERSION` → 2; ship with enum casing change. See `planning/features/typed_primitive_shape_field.md`
- [>] **Consistent RON enum casing** — change `PrefabDef.kind: String` → `PrefabKind` enum and `ColliderDef.shape: String` → `ColliderShapeKind` enum; all other categorical fields already use bare variants; `PREFAB_CATALOG_SCHEMA_VERSION` → 2; ship with typed shape change. See `planning/features/consistent_ron_enum_casing.md`
- [ ] **Consistent `assets.ron` entry shapes** — `models` entries use `(path: "...")`, `textures` are bare strings, `audio` uses `(path: "...", volume: ...)`; unifying the shapes reduces copy-paste errors and parse confusion; requires schema version bump
- [x] `Action::SetVariable` / `Action::IncrementVariable` — write to named runtime variables from RON rules
- [ ] `Condition` expressions in rules (`score >= 10`, `variable == "value"`) — currently only event matching
- [ ] Hot-reload for `.scene.ron` and `rules.ron` in native debug builds

### Groups & Membership
- [ ] **Group system — Tier 1 (factions, teams, parties)** — generic RON-defined `GroupDef` (kind, max_members, default_stance); `LoadedGroups` global resource mapping group-id → member set + `GroupMembership` component on entities; `AddToGroup` / `RemoveFromGroup` / `DisbandGroup` actions; `group.joined:{id}:{entity}` / `group.left:{id}:{entity}` / `group.full:{id}` events into the existing pipeline; faction stance rules (Hostile/Neutral/Friendly) for AI targeting; useful standalone in single-player for factions, arena teams, and NPC parties. Tier 2 (guild, chat, raid hierarchy) deferred to Beta 0.6 networking milestone. See `planning/features/group_system_tier1.md`

### Gameplay Capabilities
- [ ] **Grid system** — square, hexagonal (flat-top / pointy-top), and triangular cell layouts; `grid: GridDef` on scene RON; `(col, row)` addressing for all types (axial for hex); `PlaceOnGrid` / `StartGridMove` / `SetCellPassable` / `FindPath` actions; A* with node cap; `GridPosition` component; `grid.cell_entered` / `grid.move_complete` / `grid.path_blocked` events; Gizmos debug overlay; WASM-compatible. See `planning/features/grid_system.md`
- [x] **Game stats — Phase 2a: stat templates** — `stat_templates` on `PrefabDef`; `StatMap` component (IndexMap, Clone); dot-routing `ModifyStat`/`SetStat`; threshold/regen for instance stats; `{self}` in stat keys; goblin guard moves to behavior file; composite primitive `behavior` field fixed; integration tests + docs; design: `planning/features/stat_templates.md`
- [x] **Game stats — Phase 1: core stat model** — `StatDef` (base/min/max/regen/thresholds), `LoadedStats` resource, `ModifyStat`/`SetStat` actions, threshold events into existing pipeline; design: `planning/features/game_stats_core.md`
- [x] **Game stats — Phase 2: buffs and modifiers** — named modifier templates, additive/multiplicative/override kinds, stacking rules, soft_max, `ApplyModifier`/`RemoveModifier` actions; design: `planning/features/game_stats_buffs.md` _(depends on Phase 1)_
- [x] **Stat display — health bars and stat spreads** — `StatBar` and `StatSpread` UI node types in scene RON, colour bands, change-detection update; design: `planning/features/game_stats_display.md` _(depends on Phase 1)_
- [x] **Stat display — radar chart** — `StatRadar` UI node (3–12 axes), WGSL polar-coordinate shader via `UiMaterial`, straight-edged polygon grid (no circles), `stat_radar_update_system`; `primitive_world` demo: 5-stat pentagon (health/mana/stamina/strength/speed) on Key C overlay
- [ ] **Stat radar labels** — render stat-key labels at each axis tip of `StatRadar`; blocked by UI text on `UiMaterial` nodes; low priority
- [x] **Stat display — per-entity stat routing** — `resolve_stat(key, &LoadedStats, &Query<(&SpawnId, &StatMap)>)` shared helper; dotted keys route to entity `StatMap`, plain keys to global `LoadedStats`; all three update systems (`StatBar`, `StatSpread`, `StatRadar`) + new `stat_label_update_system` use it; `StatLabelMarker` component enables floating world-space health labels; `primitive_world` attack dummies demonstrate the feature
- [x] **World-space stat bar — Pixel style** — see `planning/features/world_pixel_stat_bar.md` _(design done, ready to implement)_
- [ ] **World-space icon stat bar** — row of per-cell sprites (hearts, shields, or any catalog icon) above entities, `WorldIconBarDef` schema field on `PrefabDef`; full cells show filled icon, empty cells show depleted icon; requires sprite-sheet or paired asset catalog entries; design needed (asset reference format, partial-cell handling)
- [ ] **Skill action bar (1–9)** — a configurable 9-slot action bar defined in scene RON as a new `ActionBar` UI node; each slot declares a keybind (1–9), icon (asset catalog texture key), one or more `do_actions` fired through the existing pipeline, optional `cooldown_secs`, and optional `cost` stat deduction; slot actions support `{target}` and `{self}` substitution; cooldown state tracked in a new `CooldownMap` resource; greyed-out visual state when on cooldown or when a cost stat is insufficient; slot state events `action_bar.activated:{slot}` / `action_bar.on_cooldown:{slot}` into the pipeline. See `planning/features/skill_action_bar.md`
- [ ] **Dialogue system** — RON-defined conversation trees between the player and NPCs; standalone `.dialogue.ron` asset files referenced by `PrefabDef.dialogue`; `DialoguePanel` UI node in scene RON; `StartDialogue` / `EndDialogue` actions; `{self}` / `{target}` substitution in text; branching via `jump_to: node_id` on choices; `do_actions` on choices fire through the existing pipeline; events `dialogue.started`, `dialogue.node`, `dialogue.choice`, `dialogue.ended`; auto-wired to `entity.interacted` when `dialogue` is set on prefab. See `planning/features/dialogue_system.md` _Soft dep: Quest system (for quest.state conditions), Targeting (for {target} in text)._
- [ ] **Inventory & item system** — `items/items.ron` catalog; `PlayerInventory` resource (persists across scenes); `Inventory` component for containers; `AddItem`/`RemoveItem`/`TransferItem`/`OpenInventory`/`CloseInventory`/`OpenShop` actions; `InventoryPanel`+`ShopPanel` UI nodes; currency via existing stat system; `MerchantDef` inline on `PrefabDef`; `PrefabKey` component added at spawn time (used by quest + loot). See `planning/features/inventory_item_system.md`
- [ ] **Equipment system** — string-key slot system (`EquipmentSlotsDef` on `PrefabDef`); `equippable`+`slot`+`stat_bonuses` on `ItemDef`; `EquipmentMap` component + `PlayerEquipment` resource; `Equip`/`Unequip`/`UnequipAll` actions; stat delta snapshot for reversal on unequip; two-handed exclusion; visual mesh attachment deferred to v2. See `planning/features/equipment_system.md` _Deps: Inventory (hard); Stat templates (soft)._
- [ ] **Quest system** — `quests/quests.ron` catalog; `QuestLog` resource (persists across scenes); objectives: `KillCount` (via `PrefabKey`+`entity.died`), `Collect`, `ReachLocation`, `TalkTo`, `Custom`; `auto_complete` flag; reward types: `GiveItem`, `GiveStat`, `UnlockQuest`, `RunActions`; `quest_giver` on `PrefabDef`; `QuestTracker` UI node; nameplate indicator patch. See `planning/features/quest_system.md` _Deps: Inventory, Dialogue, Save/load (soft); Stat templates (shipped)._
- [ ] **Loot system** — `loot/loot_tables.ron` catalog; `RollEach`/`RollOne` strategies; `loot_table` on `PrefabDef`; `LootTableRef` component; `RollLootTable(entity)`/`PickupLoot`/`ClearLootBag` actions; designer-wired via behavior file; `auto_loot` on scene RON; `ItemQuality` for icon border tinting; nested tables deferred. See `planning/features/loot_system.md` _Deps: Inventory (hard); Quest, Equipment (soft)._
- [x] Particle effect spawning via `Action::SpawnEffect` — see `planning/features/particle_effects.md`
- [ ] Timeline / sequencer — scripted cutscene playback from a RON timeline asset

### UI
- [ ] UI element types beyond `Button`: `Label`, `Image`, `ProgressBar`, `Panel`
- [x] Data-bound UI labels — `bind`/`format` fields on labels + `GameVariables` resource; `Action::SetVariable` / `Action::IncrementVariable` let designers write arbitrary variables from RON; `DebugState.score` derived from `GameVariables["score"]`
- [ ] UI layout — stack/flex layout or anchor-based positioning replacing raw pixel coords
- [ ] Font + theme config per project
- [ ] Drop shadow support for UI text
- [ ] Drop shadow support for world entity text labels

### Terrain
- [ ] Terrain snap — `snap_to_terrain: true` on entity def makes Y an offset above terrain surface; design: `planning/features/terrain_snap.md`
- [ ] Terrain chunked streaming — generate and load only chunks within a player radius; unload distant chunks; requires chunk-aware terrain capability rewrite
- [x] **Terrain path consolidation** — `TerrainConfigV2` is now the single struct (schema + runtime `Component`); `TerrainConfig` removed. Scene loader spawns `terrain_v2.clone()` directly. Fixed **scale.z bug**: `generate_terrain_mesh_raw` now takes separate `scale_x`/`scale_z` so asymmetric terrain is no longer distorted.

### Rendering & Assets
- [ ] **Deferred rendering** — replace clustered forward with Bevy's deferred pipeline to remove the `MAX_FADING_LIGHTS = 16` cap and efficiently handle large numbers of dynamic lights (torches, particle lights, explosions); transparent/additive materials (particles, decals) stay on the forward path automatically. Investigation complete — WASM builds clean, GL degrades gracefully; one remaining step: manual Chrome WebGPU console check. See `planning/features/deferred_rendering.md`
- [>] **Stylized foliage (anime / Ghibli-style trees)** — `kind: Foliage` prefab type; procedural leaf card clusters with camera-facing billboard vertex shader; sphere-mapped normals for unbroken toon shading volumes; alpha-clip brush-stroke texture; `FoliageMaterial` WGSL shader; v2 adds GPU wind sway and particle leaf drop. See `planning/features/stylized_foliage.md`
- [ ] **Toon / cel shading (3-tone, 4-tone, 5-tone)** — WGSL-only `CustomMaterial` shaders for stylized discrete light bands; 3- and 4-tone fit current uniform budget; 5-tone uses a ramp texture; design: `planning/features/toon_shading.md`
- [ ] **LOD — runtime generation + caching** — WASM-BLOCKED: generating simplified meshes at runtime requires offthread compute (web workers + `SharedArrayBuffer`); Bevy's WASM build does not support this today. Also covers IndexedDB caching of generated LODs and Bevy meshlets (GPU-driven micro-mesh rendering — not WASM-stable). Parked until Bevy's WASM offthread compute support matures.
- [ ] Decal system — project a texture onto geometry without modifying meshes
- [ ] Animated texture support in `CustomMaterial` (frame index via time uniform)
- [ ] Water / reflective plane primitive with animated normal map — WASM-BLOCKED: reflection passes require multi-pass rendering or screen-space sampling; not performantly supported in Bevy's WebGPU backend yet. Parked until WASM support matures.
- [ ] Post-process pass authoring — expose WGSL post-process shader slot per scene — WASM-BLOCKED: same root cause as Bloom and water; HDR and multi-pass post-process break or perform poorly on WebGPU. Parked until Bevy's WebGPU backend matures.

### Performance
- [ ] **Extend pipeline warmup to Text2d and UI pipelines** — spawn hidden warmup entities for `Text2d` and UI `Node` at scene load to pre-compile the 2D/UI GPU pipelines, eliminating WASM frame spikes on first text/UI render; design: `planning/features/pipeline_warmup_2d_ui.md`
- [ ] **Discrete LOD steps for depth-scaled label font sizes** — snap `base_font_size * scale` to a small fixed set (e.g. 100 %, 75 %, 50 %, 25 %) instead of rounding to every integer; bounds atlas slot count to ~4 per label regardless of depth range, at the cost of a slight stepping artefact on slow zoom. Integer rounding + 0.5-threshold guard already fix the per-frame atlas upload problem; this is a further atlas-memory micro-optimisation.
- [x] Staggered entity spawning — `PendingEntitySpawns` queue drains at `SPAWNS_PER_FRAME = 2`/frame via `drain_spawn_queue_system`; spreads WebGPU pipeline compilations across frames for wave spawns
- [ ] **Scene transition material cache** — `scene_loader` rebuilds all materials in the asset catalog on every `LoadScene`, including materials already built for the previous scene; cache built handles per-project in a persistent resource and only rebuild when the catalog key set changes; estimated 50–200 ms saving per transition on large projects (`scene_loader.rs` material rebuild block)
- [ ] **WASM terrain generation first-frame stall** — `AsyncComputeTaskPool` degrades to `block_on` on the main thread in WASM, causing 100–500 ms jank on first frame for large heightmaps; fix by splitting mesh generation across multiple frames (progressive tile-by-tile build) or pre-baking terrain meshes as GLB assets; requires `#[cfg(target_arch = "wasm32")]` code path (`terrain.rs` poll system)
- [x] **Particle mesh buffer recreation** — the pool renderer rebuilds full `Vec<[f32;3]>` vertex buffers and re-inserts mesh attributes every frame for every active particle group; replace with in-place attribute mutation (`Mesh::attribute_mut`) to avoid per-frame allocations and reduce GPU upload overhead; particularly impactful on WASM where buffer uploads block the main thread (`particle_renderer.rs`)
- [x] **Animation player entity lookup cache** — `animation_playback_system` recurses through the entity hierarchy every frame to locate the `AnimationPlayer` child; `AnimationController` already has a `last_player_entity` field — use it as a fast-path cache and only re-walk the tree on cache miss; reduces O(tree_depth) lookups to O(1) for the common case (`animation.rs`)
- [ ] **Per-frame collection allocations in hot systems** — `stat_modifier_system` (key clone Vec), `message_interpreter_system` (event Vec rebuilt each frame), and `player_movement_system` (input HashMap) each allocate new collections every frame; refactor to reuse pre-allocated collections via `.clear()` on a `Local` resource or avoid the intermediate collection entirely; WASM GC pressure is higher than native so the gain is larger there

### Profiling & Diagnostics
- [ ] Diagnostics HUD — F3 overlay: FPS, frame time, entity count, draw calls, triangles, CPU/RAM (native); design: `planning/features/diagnostics_hud.md`
- [ ] Tracy integration — `--features trace_tracy` on native runner; per-system CPU timeline; design: `planning/features/tracy_integration.md`

### Designer Experience
- [ ] **Extend `entity_logic_demo`** — add one clearly labeled station per behavior concept: multi-state FSM behavior file with `EmitEventAfterDelay` loop (goblin-guard pattern), a timed-door sequence (`EmitEventAfterDelay` chain), side-by-side trigger zone enter vs. interactable [F] comparison, and a `global_on` example showing project-wide vs. entity-local events; modeled on the station-per-concept layout of `particles_demo`
- [x] **`stats_demo` project** — standalone demo project showcasing the full stats system in one place: health/mana bars, stat spread widget, radar chart, world-space pixel bars, buffs and modifiers, damage popups, and per-entity stat routing; the current `primitive_world` mixes all of this with geometry and AI work making it hard to use as a reference
- [ ] **`ui_demo` project** — standalone demo project for every UI capability: buttons, data-bound labels with `SetVariable`/`IncrementVariable`, overlays, pause-menu pattern, and all stat display widget types; gives designers a single project to copy patterns from
- [ ] **`audio_demo` project** — standalone demo project focused on audio authoring: `PlaySound`, `PlayMusicLoop`, `SetVolume`, stop/loop patterns, and how audio is triggered from RON rules; no existing project makes audio its primary focus
- [ ] **`scene_transitions_demo` project** — standalone demo project with 3–4 scenes wired through a state machine, demonstrating portal navigation, scene overlays, preloading, and multi-scene project config; distinct from `particles_demo` where portal navigation is a side feature
- [x] **Blank starter project template** — a minimal `blank_project` under `assets/projects/` containing only the required files (project config, one empty scene, empty prefab catalog, empty asset catalog, empty rules), no terrain, no models, and no dummy fields; the canonical copy-and-rename starting point so new projects do not inherit `quick_scene` noise
- [ ] **Schema version v2→v3 migration guide** — add a "Migrating from v2 to v3" section in `docs/20_data_formats.md` covering: rename `rules_path` → `state_machine_path`, bump `schema_version` to `3`, convert `rules.ron` to the FSM format, and the warning to expect if both files coexist
- [ ] **Magic-string event/action validator** — `tools/ron_validator/` CLI that cross-checks event names used in `rules.ron` / `state_machine.ron` against the set emitted by capabilities and reports unknown event keys before runtime; eliminates silent no-ops from typos in event names

### Tools
- [ ] `tools/ron_formatter/` — auto-format `.ron` files (indentation, trailing commas)
- [ ] Live reload server — watch `assets/` and push scene reload to running native build via IPC
- [ ] GLB batch inspector — produce a markdown table of node names, animations, and materials for a whole folder
- [ ] **Live project editor** — `crates/ironhold_editor`; axum server on port 3001 serving React frontend + WASM game preview + REST API; `schemars`-derived JSON Schema → RJSF forms; RON ↔ JSON bridge with validation gate; WebSocket-triggered iframe reload on save; v1 edit-only, v2 create/delete. See `planning/features/live_project_editor.md`

---

## Done (reference)

- [x] **no diagnostic when a rule event is never matched** — `match_rules` emits `debug!` when rules are loaded but none fire; silent on FSM projects where `LoadedRules` is empty.
- [x] **`rules.ron` silently ignored when `state_machine_path` is set** — `project_loader` warns at Phase 1 if both `rules_path` and `state_machine_path` are set.
- [x] **`Spawn` `position`/`spawn_point` conflict is silent** — `action_executor` warns with the spawn ID when both fields are set; `position` wins.
- [x] **quick_scene web spawn hang** — `Action::PreloadPrefab("enemy_orc_melee")` fires on `scene.ready:main` so the orc GLB is decoded during scene load before the button is reachable; `PreloadedGlbHandles` resource keeps the handle alive. A `PendingEntitySpawns` queue (drained at 2/frame) was added simultaneously — it doesn't eliminate the remaining ~300 ms WebGPU pipeline-compile stall on first render, but caps per-frame stalls to 2 entities for wave spawns.
- [x] **animation T-pose on landing** — Root cause: Bevy's `SceneSpawner` re-spawns the GLTF hierarchy mid-session. `animation_playback_system` now detects when the `AnimationPlayer` entity changes and resets `graph_initialized`. See `planning/investigations/resolved/animation_tpose.md`.
- [x] **`implicit_some` RON extension** — `ImplicitRonPlugin` in `schema/ron_loader.rs` enables `implicit_some` globally via `ron::Options`; 671 `Some()` wrappers removed from all project `.ron` files; `tools/migrate_implicit_some.py` one-shot migration script included; no per-file directives needed
- [x] **Nested prefabs — mesh support** — `spawn_primitive_children` dispatches on `kind`: actor/prop loads GLB via `spawn_prefab_instance`, single-shape primitive builds one mesh; `rock_deco` GLB prop nested in `village` demo; design: `planning/features/nested_prefabs_mesh_support.md`
- [x] **Nested prefabs** — `children` entries reference named prefabs by key; multiplicative Bevy hierarchy; cycle detection; `village` prefab demo in `primitive_world`; design: `planning/features/nested_prefabs.md`
- [x] Beta 0.1 — Baseline Runtime
- [x] Beta 0.2 — Event/Action Bus refactor
- [x] Beta 0.3 — Global Logic (FSM v1)
- [x] Beta 0.4 — Entity Logic (FSM v1): per-entity `.behavior.ron`, `{self}` substitution, `TriggerZone`, `Interactable`, `PlayAnimationOn`/`EmitEvent`, `entity_logic_demo` project
- [x] Three-point warm lighting defaults for GLB preview tool (`--light-strength 0.3`)
- [x] **World-space pixel stat bar** — see `planning/features/done/world_pixel_stat_bar.md`
- [x] **Particle effect spawning v1** — campfire, torch, explosions, triggers, UV distort/scroll complete. See `planning/features/done/particle_effects.md`
- [x] **Particle System v2 — Pool Renderer** — CPU pool + one mesh entity per (blend_mode, texture) group; O(distinct textures) draw calls; `PoolFlameMaterial` for UV-animated flames. See `planning/features/done/particle_instanced_renderer.md`
- [x] **Multi-layer EffectDef** — compose effects from multiple emitter layers in one RON key. See `planning/features/done/particle_multi_layer.md`
- [x] **Quality tiers & particle budget** — `SetParticleQuality` action, per-effect priority (`Player`/`Npc`/`Ambient`), live-count cap, multi-layer running counter, portal navigation, Arcane Observatory demo scene. See `planning/features/done/particle_quality_budget.md`
- [x] **Ironhold CLI** — `validate`, `inspect glb/texture/audio`, `query prefabs/effects/scenes/rules`; `--json` flag throughout; all example projects pass validate. See `planning/features/ironhold_cli.md`
- [x] **Ironhold CLI enhancements** — `watch`, `stats`, `query actions/events`, `validate --strict` (orphan detection), `after_help` examples across all 14 help pages, exit-code docs, `--json` placement note, missing `--filter` values.
