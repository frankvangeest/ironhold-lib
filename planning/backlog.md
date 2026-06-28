# Backlog

> **How this works**
> - Items progress: `Icebox → Queued → Active → Done`
> - Simple items live here as bullet points. Anything needing design lives in `features/`.
> - This file tracks *what to build next*, not *how* — keep it skimmable.
> - Roadmap and milestone gates: see `docs/50_roadmap_and_milestones.md`
> - Implementation status: see `docs/STATUS.md`

---

## Active

- [x] **Nameplate system** — floating name + health bar above entities, scene-wide opt-in (`show_nameplates: true` in scene RON) with per-prefab override; visibility filtered by faction stance (hostile / friendly / all) and optional max distance; distinct from per-entity world-space stat bars — nameplates are managed by a single system scanning all tagged entities. _(Quest-giver `!`/`?` indicator belongs to Quest system v2, not here.)_ See `planning/features/done/nameplate_system.md`
- [x] **Intent event layer** — emit `intent.slot.{n}:{entity}` from `action_bar.rs` before committing, route through the interpreter; designers can then cancel/redirect ability slots from RON rules using `when:` gates. Fixes the one capability that bypasses the Message→Interpreter→Action→Executor pipeline. See `planning/features/done/intent_event_layer.md`
- [ ] **Consolidate conditional prefab-feature application (sibling divergence)** — `interactable` / `trigger_zone` / `behavior` / `stat_templates` are still applied per-path in `scene_loader.rs` for single-mesh and composite primitive branches but only centrally in `spawn_prefab_instance` for GLB prefabs; introduce a `apply_prefab_features(ec, &PrefabDef)` helper (parallel to `tag_spawned_entity`) called at all primitive branches; closes the same "works for one kind, silently missing for another" bug class one level down. See `claude_suggestions.md`.

---

## Bugs

- [x] **`npc_revive` stop-action sentinel leaks into clip pipeline** — `PlayAnimationOn(clip: "npc_revive")` fired by enemy behavior on `alive` entry reached the raw-clip-name branch of `animation_resolver.rs` on fresh spawns (no active override → stop-action check was skipped), setting `controller.current = "npc_revive"` and triggering two fallback paths per enemy per spawn. Fixed: sentinels are now intercepted before the raw-clip branch and always dropped.
- [ ] **Stale `EmitEventAfterDelay` fires after entity state exit** — `enemy_spider` 'dead' state schedules `spider.hide:{self}` at 15s; if the spider respawns before 15s elapses the pending delay fires and hides the now-alive spider. Root cause: delay system has no cancel/guard on state transition. Reproduce: kill a spider in `3rd_person_game_demo` and wait for respawn within 15s.
- [ ] **uphill jump lock** — when jumping against an uphill slope, the player can land in a state where `jump` never re-triggers: the character controller reports ground contact but the slope normal keeps the jump cooldown active. Suspected cause: Rapier's ground-contact normal threshold in the character controller or the jump cooldown not resetting when sliding contact ends. Reproduce: 3rd_person_game_demo, run toward any hill and spam jump while ascending.
- [x] **composite prefab child positions and physics wrong for nested Actor/Prop** — Root cause: Rapier reads `GlobalTransform` before `TransformPropagate` runs; a Bevy-parented child entity's `GlobalTransform` at that moment equals its local offset, not the world position — Rapier locked the `Fixed` body there permanently. Fix: `spawn_primitive_children` computes `world_child_tf = parent_world_tf.mul_transform(child_tf)` and spawns nested Actor/Prop entities as root-level entities (no `add_child`) at the composed world position; root entities satisfy `GlobalTransform == Transform` from frame 1. `TriggerZone` sensor child entities had `Visibility::default()` removed to eliminate visibility propagation overhead (was causing frame stutter).
- [ ] **Frame/audio stutter worsens on camera movement (WASM release)** — periodic stall visible in `primitive_world` release build; intensifies when moving the camera. Suspected cause: WebGPU synchronous pipeline compilation stalls when new mesh+material combinations enter the frustum for the first time. `pipeline_warmup_system` (4-frame `NoFrustumCulling` pass) covers scene-loaded entities but may not cover all variants or dynamically entering geometry. Reproduce: `primitive_world`, walk around for 10–20 seconds and observe frame hitches.

---

## Queued

### Engine / Runtime

- [ ] **Static scene mode (`?static=1`)** — freeze all time-driven systems (animations, NPC AI, motion, particles) immediately after `SceneEvent::Ready` so browser screenshot baselines are pixel-identical across runs. Mechanism: parse `?static=1` URL param in the WASM runner → `StaticMode(bool)` resource → pause `Time<Virtual>` + seek all `AnimationPlayer`s to t=0 on scene ready. Requires `start_app` signature change (all three crates) and a one-line change to `test_web.py`. See `planning/features/static_scene_mode.md`.
- [ ] **Overlay modal backdrop (click-blocking)** — when `LoadSceneOverlay` loads a scene, spawn a full-screen transparent UI rect (z-index above all base-scene UI) that absorbs pointer events; despawn it on `UnloadOverlay`; prevents base-scene buttons from remaining clickable through overlay panels. Currently overlays allow click-through — acceptable for `paused` (base-scene events are silently dropped) but broken for overlays where base-scene buttons are actively harmful (e.g. start menu showing through options panel).
- [ ] **Inventory / shop / container click-blocking backdrop** — when `OpenInventory`, `OpenShop`, or `OpenContainer` shows a panel, spawn a full-screen absorbing rect beneath the panel (same technique as overlay modal backdrop but triggered by the panel open/close actions); prevents base-scene world-space interactions (collectibles, NPC interactables) firing through the UI while a window is open. _Dep: overlay modal backdrop (click-blocking) — reuse the same backdrop spawning utility._
- [ ] **Promote magic `tags` to typed prefab fields** — add `collectable: bool`, `player: bool`, and `flycam: bool` as `#[serde(default)]` fields on `PrefabDef`; `tags` remains for free-form designer labels but control-flow semantics move to typed fields; consistent with the `PrefabKind` enum casing work that cleaned up `kind`. Additive, no migration required.
- [ ] **Per-prefab `depth_scale` honoured on dynamic spawns** — `StatLabelDef`/`WorldStatBarDef.depth_scale` overrides are silently ignored for `Action::Spawn` entities; fix by storing scene-level label depth config in a `LoadedLabelDepthScale` resource at scene load and reading it in `drain_dynamic_stat_ui_system`. See `planning/features/depth_scale_dynamic_spawn.md`
- [ ] **Page visibility / focus-loss handling** — freeze delta time, pause audio, and drop render to zero when the browser tab loses focus; resume cleanly on tab restore without physics or audio desync; wire Bevy's `WindowFocused` / `ApplicationLifetime` events behind a `pause_on_focus_loss: bool` field on `ProjectConfig` (default `true`); opt-out lets streaming / spectator scenes keep running. Sourced from Phaser's focus-loss model.
- [ ] **Optional `physics` Cargo feature** — gate Rapier3D behind a `physics` feature on `ironhold_core` so projects that don't use colliders skip the ~15 MB of Rapier symbols in the WASM binary; `ColliderDef` in RON becomes a validated-but-no-op field when the feature is absent; `PhysicsPlugin` conditionally compiled; `ironhold_web` enables `physics` by default but a future stripped build could omit it. Sourced from Phaser's Arcade vs Matter modular physics model.

### Camera
- [ ] **Camera mode unification (v1)** — unify `OrbitCamera` and `FlyCamera` under a single `ActiveCameraMode` resource; backward-compat mapping for existing `camera:`/`flycam:` prefab fields; no new designer-facing surface, but de-risks `CameraShake` re-homing and the persistent-camera/black-frame issue. See `planning/features/camera_modes.md`
- [ ] **Camera modes — new modes + switching (v2)** — `Follow`, `FirstPerson`, `Fixed` modes in RON; `SetCameraMode` action with optional eased transitions; FOV interpolation. _Dep: camera mode unification (v1)._ See `planning/features/camera_modes.md`

### Gameplay & Environment

- [ ] **Status effect icons — HUD bar (v1)** — `StatusEffectBar` UI node in scene RON; shows active player buffs/debuffs as a strip of icons; icons are asset catalog texture keys on modifier templates; updates via change detection on `ActiveModifiers`. See `planning/features/status_effect_icons.md`
- [ ] **Status effect icons — world-space strip (v2)** — icon strip above entities (not just the player); shares `collect_visible_modifiers` logic; separate spawn/despawn path per entity. _Dep: HUD bar (v1)._ See `planning/features/status_effect_icons.md`
- [ ] **Layered icon UI node** — new `LayeredIcon` UI node type; each layer declares a texture key, tint color (r,g,b,a), and opacity; layers are alpha-composited in declaration order (bottom → top); v1 alpha-stack only — additive blend mode deferred to a future `blend:` field per layer; feeds action bar slot icons and status effect icon strips directly.
- [ ] **AoE ground targeting** — `TargetingMode: GroundAoE(radius)` on skill action bar slots; pressing the slot enters a placement mode showing a circle decal under the cursor; confirming fires the slot's `do_actions` with `{aoe_position}` substitution; cancelling (right-click / Escape) exits without firing. See `planning/features/aoe_ground_targeting.md` _Hard dep: Skill action bar._
- [ ] **Action bar custom hotkeys** — bind any `parse_key`-recognised key name (`"KeyQ"`, `"KeyE"`, `"F2"`) to action bar slots in RON; removes the hardcoded `DIGIT_KEYS` table; optional `key_label` field overrides the on-screen corner hint; fully backward-compatible with existing `"1"`–`"9"` layouts. See `planning/features/action_bar_custom_hotkeys.md`
- [ ] **Spawn wave / encounter system** — `WaveDef` in RON: an ordered sequence of spawn steps each with a prefab key, count, delay, and optional position list; fires on an event (`StartWave("wave_01")`), emits `wave.complete:{id}` when all spawned entities are dead; supports looping waves and inter-wave delays. Designer-friendly alternative to scripting individual `Spawn` + `EmitEventAfterDelay` chains. See `planning/features/spawn_wave_encounter.md`
- [ ] **Day/night cycle** — `DayNightCycleDef` in scene RON: cycle duration, sun color/intensity keyframes at dawn/noon/dusk/midnight; `TimeOfDay` resource drives directional light + ambient each frame; `SetTimeOfDay(hour)` and `SetDaySpeed(multiplier)` actions; emits `time.dawn` / `time.noon` / `time.dusk` / `time.midnight` events designers can hook; WASM compatible (pure CPU, no post-process). See `planning/features/day_night_cycle.md`
- [ ] **Audio channels (volume buses)** — `channels: HashMap<String, f32>` on `AudioConfig`; each audio entry in `assets.ron` declares a `channel` key; `SetChannelVolume(channel, f32)` action scales that category within the master ceiling; enables independent music/sfx/ambient balance without touching source files. _Depends on mute toggle + master volume._
- [ ] **Item-gated interactable** — `requires_item: "key_id"` field on `PrefabDef.interactable`; fires `entity.interact_blocked:{id}` when the player lacks the item, `entity.interacted:{id}` when they have it; enables key-locked doors and quest-gated triggers without a GameVariable workaround. _Dep: Inventory._
- [ ] **Conditional dialogue choices** — `condition_var`/`condition_value` fields on a `DialogueChoiceDef`; hides the choice when the named `GameVariable` doesn't match; enables quest-gated reward branches and post-event dialogue without duplicating nodes. _Dep: Dialogue system._
- [ ] **Sound zones** — ambient audio driven by player location; a new `kind: SoundZone` trigger zone variant with `audio_key`, `volume`, and `fade_distance` fields; entering the zone fades in the audio, leaving fades it out; defined entirely in scene RON using the existing trigger zone + `PlayMusicLoop`/`StopMusic` actions, no new systems needed beyond the fade envelope.
- [ ] **World-space icon stat bar** — row of per-cell sprites (hearts, shields, or any catalog icon) above entities, `WorldIconBarDef` schema field on `PrefabDef`; full cells show filled icon, empty cells show depleted icon; requires sprite-sheet or paired asset catalog entries; design needed (asset reference format, partial-cell handling)
- [ ] **Stat radar labels** — render stat-key labels at each axis tip of `StatRadar`; blocked by UI text on `UiMaterial` nodes; low priority
- [ ] **Equipment system (v1)** — string-key slot system (`EquipmentSlotsDef` on `PrefabDef`); `equippable`+`slot`+`stat_bonuses` on `ItemDef`; `EquipmentMap` component + `PlayerEquipment` resource; `Equip`/`Unequip`/`UnequipAll` actions; stat delta snapshot for reversal on unequip; two-handed exclusion. See `planning/features/equipment_system.md` _Deps: Inventory (hard); Stat templates (soft)._
- [ ] **Quest system — core loop (v1)** — `quests/quests.ron` catalog; `QuestLog` resource (persists across scenes); `AcceptQuest`/`CompleteQuest`/`FailQuest` actions; objectives: `KillCount`, `Collect`, `ReachLocation`, `TalkTo`, `Custom`; `auto_complete` flag; reward types: `GiveItem`, `GiveStat`, `UnlockQuest`, `RunActions`. Fully testable via events alone. See `planning/features/quest_system.md` _Deps: Inventory, Dialogue (soft); Stat templates (shipped)._
- [ ] **Quest system — presentation layer (v2)** — `QuestTracker` UI node; quest-giver `!`/`?` nameplate indicator; `DialogueCondition::QuestState` patch for dialogue branching on quest state. _Deps: Quest core (v1), Nameplate system._ See `planning/features/quest_system.md`
- [ ] **Monster drop pickups** — enemies spawn interactable drops (health potion, coins) at their death position via a new `at_entity` field on `Action::Spawn`; behavior files handle collect and auto-expire logic entirely in RON; exercises the full dynamic-spawn pipeline with a meaningful reward loop. Lands the `at_entity` primitive that Loot v1 reuses. See `planning/features/monster_drop_pickups.md`
- [ ] **Loot system — roll + auto-loot (v1)** — `loot/loot_tables.ron` catalog; `RollEach`/`RollOne` strategies; `loot_table` on `PrefabDef`; `RollLootTable(entity)` action with `auto_loot: true` delivering directly to inventory; designer-wired via behavior file; closes Quest `Collect` objective dep. See `planning/features/loot_system.md` _Deps: Inventory (hard); `at_entity` Spawn field (soft — reuse from Monster drop pickups)._
- [ ] **Loot system — physical loot bags (v2)** — `LootBag` entity with pickup UI; `PickupLoot`/`ClearLootBag` actions; `ItemQuality` for icon border tinting; nested tables. _Dep: Loot v1._ See `planning/features/loot_system.md`
- [ ] Timeline / sequencer — scripted cutscene playback from a RON timeline asset

### Particle System v2

- [ ] **Bloom / post-processing in scene RON** — WASM-BLOCKED: Bevy's `Bloom` requires `#[require(Hdr)]`; HDR breaks the WASM build. Parked until performant HDR/post-process support is available in Bevy's WebGPU backend. Do not implement a native-only workaround — that splits the runtime model. See `planning/features/particle_bloom.md`
- [ ] **Shared effect library** — `assets/shared/effects/` with reusable effects and per-project overrides. See `planning/features/particle_shared_library.md`

### Rendering & Assets

- [ ] **LOD — pre-baked mesh swap** — distance-based LOD switching using offline-generated LOD GLB files; `lod_levels: [(distance, model)]` on `PrefabDef` declares swap thresholds; a system watches camera distance and swaps `SceneRoot` handle; LOD meshes generated offline (Blender / `meshopt`) and referenced in `assets.ron`; no runtime compute required — fully WASM-compatible. See `planning/features/lod_prebaked_mesh_swap.md`
- [ ] **Channel-packed ORM texture shader** — new WGSL shader variant (`custom_texture_triplanar_pbr_packed.wgsl`) that reads a single packed ORM texture (R=occlusion, G=roughness, B=metallic) instead of three separate textures; matches the default export format from Substance Painter; frees two sampler slots and halves texture bandwidth for PBR properties; no schema changes — designers swap the shader key and point one texture at `texture_1` instead of three.
- [ ] **Deferred rendering** — replace clustered forward with Bevy's deferred pipeline to remove the `MAX_FADING_LIGHTS = 16` cap and efficiently handle large numbers of dynamic lights (torches, particle lights, explosions); transparent/additive materials (particles, decals) stay on the forward path automatically. Investigation complete — WASM builds clean, GL degrades gracefully; one remaining step: manual Chrome WebGPU console check. See `planning/features/deferred_rendering.md`
- [ ] **Toon / cel shading (3-tone, 4-tone, 5-tone)** — WGSL-only `CustomMaterial` shaders for stylized discrete light bands; 3- and 4-tone fit current uniform budget; 5-tone uses a ramp texture; design: `planning/features/toon_shading.md`
- [ ] **LOD — runtime generation + caching** — WASM-BLOCKED: generating simplified meshes at runtime requires offthread compute (web workers + `SharedArrayBuffer`); Bevy's WASM build does not support this today. Also covers IndexedDB caching of generated LODs and Bevy meshlets (GPU-driven micro-mesh rendering — not WASM-stable). Parked until Bevy's WASM offthread compute support matures.
- [ ] Decal system — project a texture onto geometry without modifying meshes
- [ ] Animated texture support in `CustomMaterial` (frame index via time uniform)
- [ ] Water / reflective plane primitive with animated normal map — WASM-BLOCKED: reflection passes require multi-pass rendering or screen-space sampling; not performantly supported in Bevy's WebGPU backend yet. Parked until WASM support matures.
- [ ] Post-process pass authoring — expose WGSL post-process shader slot per scene — WASM-BLOCKED: same root cause as Bloom and water; HDR and multi-pass post-process break or perform poorly on WebGPU. Parked until Bevy's WebGPU backend matures.

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

### Beta 0.7a — Loading Screen
- [ ] Loading screen overlay during `LoadingScene` / `LoadingProject` states
- [ ] `scene.loading_progress:{0-100}` milestone events from loader and terrain task
- [ ] `loading_scene` field in project config for custom splash scenes
- [ ] Docs + tests
- [ ] Design: `planning/features/loading_screen.md`

### Beta 0.7b — Scene Preloading
- [ ] `preload_poll_system`: watch `PreloadedScenes` handles, emit `scene.preloaded:{name}`
- [ ] `LoadScene` fast-path when handle is already loaded in `PreloadedScenes`
- [ ] Docs + tests
- [ ] Design: `planning/features/scene_preloading.md`

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

### Terrain
- [ ] Terrain snap — `snap_to_terrain: true` on entity def makes Y an offset above terrain surface; design: `planning/features/terrain_snap.md`
- [ ] Terrain chunked streaming — generate and load only chunks within a player radius; unload distant chunks; requires chunk-aware terrain capability rewrite
- [ ] **Improved terrain rendering — Phases 1+2** — UV elimination + U16 indices (~25 % vertex memory); mesh chunking (per-chunk culling + incremental async generation, eliminates WASM first-frame stall); unblocks terrain snap and chunked streaming. See `planning/features/improved_terrain_rendering.md`
- [ ] **Improved terrain rendering — Phases 3+4** _(gated on Phase 0 WebGPU PoC)_ — GPU-derived XZ positions; compressed normals; CPU height-array shared between GPU and Rapier. Start only after Phase 0 investigation confirms feasibility. See `planning/features/improved_terrain_rendering.md`

### Performance
- [ ] **Off-thread texture decode in WASM** — use the browser's `ImageBitmap` API to decode textures off the main WASM thread, eliminating main-thread stalls during asset loads; requires a `wasm32`-specific code path in the asset loading pipeline or a Bevy plugin that wraps `createImageBitmap`; investigate whether Bevy's current WASM asset pipeline already defers texture decode before implementing. Sourced from Phaser's web-optimised asset loader model.
- [ ] **Extend pipeline warmup to Text2d and UI pipelines** — spawn hidden warmup entities for `Text2d` and UI `Node` at scene load to pre-compile the 2D/UI GPU pipelines, eliminating WASM frame spikes on first text/UI render; design: `planning/features/pipeline_warmup_2d_ui.md`
- [ ] **Discrete LOD steps for depth-scaled label font sizes** — snap `base_font_size * scale` to a small fixed set (e.g. 100 %, 75 %, 50 %, 25 %) instead of rounding to every integer; bounds atlas slot count to ~4 per label regardless of depth range, at the cost of a slight stepping artefact on slow zoom. Integer rounding + 0.5-threshold guard already fix the per-frame atlas upload problem; this is a further atlas-memory micro-optimisation.
- [ ] **Scene transition material cache** — `scene_loader` rebuilds all materials in the asset catalog on every `LoadScene`, including materials already built for the previous scene; cache built handles per-project in a persistent resource and only rebuild when the catalog key set changes; estimated 50–200 ms saving per transition on large projects (`scene_loader.rs` material rebuild block)
- [ ] **WASM terrain generation first-frame stall** — `AsyncComputeTaskPool` degrades to `block_on` on the main thread in WASM, causing 100–500 ms jank on first frame for large heightmaps; fix by splitting mesh generation across multiple frames (progressive tile-by-tile build) or pre-baking terrain meshes as GLB assets; requires `#[cfg(target_arch = "wasm32")]` code path (`terrain.rs` poll system)
- [ ] **Per-frame collection allocations in hot systems** — `stat_modifier_system` (key clone Vec), `message_interpreter_system` (event Vec rebuilt each frame), and `player_movement_system` (input HashMap) each allocate new collections every frame; refactor to reuse pre-allocated collections via `.clear()` on a `Local` resource or avoid the intermediate collection entirely; WASM GC pressure is higher than native so the gain is larger there

### Profiling & Diagnostics
- [ ] Diagnostics HUD — F3 overlay: FPS, frame time, entity count, draw calls, triangles, CPU/RAM (native); design: `planning/features/diagnostics_hud.md`
- [ ] Tracy integration — `--features trace_tracy` on native runner; per-system CPU timeline; design: `planning/features/tracy_integration.md`

### Designer Experience
- [ ] **Foliage demo — visual tuning** — refine cluster placement, leaf scale, and per-prefab parameters in `foliage_demo` to match the reference image quality. Blocked on live RON reload or live editor — parameter tuning without a tight feedback loop is impractical. Pick up after either ships.
- [ ] **Extend `entity_logic_demo`** — add one clearly labeled station per behavior concept: multi-state FSM behavior file with `EmitEventAfterDelay` loop (goblin-guard pattern), a timed-door sequence (`EmitEventAfterDelay` chain), side-by-side trigger zone enter vs. interactable [F] comparison, and a `global_on` example showing project-wide vs. entity-local events; modeled on the station-per-concept layout of `particles_demo`
- [ ] **`ui_demo` project** — standalone demo project for every UI capability: buttons, data-bound labels with `SetVariable`/`IncrementVariable`, overlays, pause-menu pattern, and all stat display widget types; gives designers a single project to copy patterns from
- [ ] **`audio_demo` project** — standalone demo project focused on audio authoring: `PlaySound`, `PlayMusicLoop`, `SetVolume`, stop/loop patterns, and how audio is triggered from RON rules; no existing project makes audio its primary focus
- [ ] **`scene_transitions_demo` project** — standalone demo project with 3–4 scenes wired through a state machine, demonstrating portal navigation, scene overlays, preloading, and multi-scene project config; distinct from `particles_demo` where portal navigation is a side feature
- [ ] **Schema version v2→v3 migration guide** — add a "Migrating from v2 to v3" section in `docs/20_data_formats.md` covering: rename `rules_path` → `state_machine_path`, bump `schema_version` to `3`, convert `rules.ron` to the FSM format, and the warning to expect if both files coexist
- [ ] **Magic-string event/action validator** — `tools/ron_validator/` CLI that cross-checks event names used in `rules.ron` / `state_machine.ron` against the set emitted by capabilities and reports unknown event keys before runtime; eliminates silent no-ops from typos in event names
- [ ] **CLI `--strict` merchant cross-validation** — cross-check merchant `currency_stat` against stat defs and `item_key` against `items.ron` at validate time; catches typos before any runtime run, mirroring existing scene→prefab checks.

### UI
- [ ] UI element types beyond `Button`: `Label`, `Image`, `ProgressBar`, `Panel`
- [ ] UI layout — stack/flex layout or anchor-based positioning replacing raw pixel coords
- [ ] Font + theme config per project
- [ ] Drop shadow support for UI text
- [ ] Drop shadow support for world entity text labels
- [ ] **Draggable UI windows** — players drag inventory, shop, and container panels by their title bars; pure positional `Node.left/top` mutation (no ActionQueue involvement); position resets to RON-authored origin on scene change. See `planning/features/draggable_windows.md`
- [ ] **Cursor grab icon for draggable windows** — grab-hand cursor on title-bar hover, grabbing-fist while dragging; cosmetic companion to draggable windows, pipeline-free. _Dep: draggable UI windows._ See `planning/features/cursor_grab_icon.md`
- [ ] **Hover tooltips for inventory and action bar** — floating card on inventory-slot and action-bar-slot hover; shows item name/description/stats or skill title/effect/cooldown footer; single shared overlay node repositioned on hover change, edge-clamped to viewport; pipeline-free; supersedes the never-rendered `ActionSlotDef.label` field. See `planning/features/tooltip_system.md`
- [ ] **Three-channel icon masking** — `icon_colors: [(r,g,b,a); 3]` on `ActionSlotDef` and `ItemDef`; `IconMaskMaterial` WGSL UiMaterial maps each R/G/B channel to an independent designer-specified color from one channel-packed sprite cell; existing `icon_color` tint path unchanged. See `planning/features/icon_three_channel_mask.md`

### Tools
- [ ] `tools/ron_formatter/` — auto-format `.ron` files (indentation, trailing commas)
- [ ] Live reload server — watch `assets/` and push scene reload to running native build via IPC
- [ ] GLB batch inspector — produce a markdown table of node names, animations, and materials for a whole folder
- [ ] **Icon sheet builder** — `tools/icon_sheet/build.py` stitches a folder of individual icon PNGs into a power-of-2 RGBA atlas + sidecar manifest mapping filename-stem to `icon_index`; no engine changes; eliminates hand-packing and magic cell counting for designers. See `planning/features/icon_sheet_builder.md`
- [ ] **Live project editor** — `crates/ironhold_editor`; axum server on port 3001 serving React frontend + WASM game preview + REST API; `schemars`-derived JSON Schema → RJSF forms; RON ↔ JSON bridge with validation gate; WebSocket-triggered iframe reload on save; v1 edit-only, v2 create/delete. See `planning/features/live_project_editor.md`

---

## Icebox

### Physics
- [ ] **wgrapier — GPU physics watch item** — `wgrapier3d` v0.2.0 (Dimforge, Nov 2024) is a WebGPU compute-shader dynamic-body simulator; not usable for terrain height queries (requires async GPU→CPU readback); no Bevy integration; Dimforge has announced a full rewrite on `rust-gpu`. Revisit when the rust-gpu rewrite ships with real releases, an official Bevy bridge exists, and a dense dynamic-body workload justifies it.

### Engine / Runtime
- [ ] **Bevy 0.19 upgrade** — gated on `bevy_rapier3d`, `bevy_framepace`, and `bevy_common_assets` all shipping 0.19-compatible releases (none confirmed as of 2026-06-23). Est. ~4–7 person-days once the dep tree resolves. See `planning/investigations/bevy_019_upgrade.md`.
- [ ] **Camera/input configuration → scene layer** — `orbit_camera`, `flycam`, and `player` blocks on `PrefabDef` are scene singletons consumed as such by the runtime; architecturally they belong on the scene (or project config); migration path: introduce optional scene-level camera/input fields first, deprecate-but-keep prefab fields, then clean up in a `PREFAB_CATALOG_SCHEMA_VERSION` bump; lower urgency once per-instance overrides ship since that reduces the pressure on prefab forks. From scene/prefab boundary analysis.
- [ ] **Scene layer compositing** — `layer: Overlay | Base` field on `GameSceneV2`; overlay scenes render on top of the base scene without unloading it, enabling persistent pause menus, HUD layers, and cutscene overlays authored entirely in RON; renderer approach (two active Bevy worlds vs. `RenderLayer` masking) needs design investigation before coding. Sourced from Phaser's layered-scene architecture.
- [ ] Capability registry — declare events, actions, and validation rules per capability; replaces ad-hoc wiring
- [ ] Schema migrations — versioned upgrade paths with diagnostics on load failure
- [ ] **Gamepad / controller input** — wire Bevy's built-in gamepad input through the existing `InputAction` system and RON key bindings; map stick axes to movement/camera and face buttons to `InputAction` variants; designers declare gamepad bindings in the same input config block as keyboard; needed for web builds targeting controller users
- [ ] **Save / load game state** — `SaveGame` / `LoadGame` actions; serialize `GameVariables`, per-entity `StatMap`, and active modifier state to a JSON/RON file (native) or `localStorage` (WASM); `AutoSave` trigger on configurable events; scene transitions preserve state across loads. See `planning/features/save_load_game_state.md`
- [ ] **Input remapping** — let players rebind keyboard and gamepad actions at runtime via a settings UI; bindings persisted to a per-player config file (native) or `localStorage` (WASM); designer declares remappable actions and default bindings in project RON; depends on gamepad input feature for full coverage
- [ ] **Per-instance stat overrides on `Action::Spawn` (v2)** — extend `stat_overrides` support to dynamic `Action::Spawn` so runtime-spawned entities can also start with non-default stats; requires threading the override map through `QueuedSpawn` and `drain_spawn_queue_system`; depends on per-instance overrides v1 (scene-placed) shipping first.
- [ ] **`ChildOf` hierarchy migration** — migrate from `Children`/`Parent` (Bevy pre-0.16 API) to the `ChildOf` relationship component (Bevy 0.16+); the animation system queries `&Children` to walk GLB hierarchies and all spawners use `with_children()` — these need updating to the forward-looking API before a future Bevy upgrade removes the compat shim
- [ ] **Required components on project-defined components** — adopt `#[require(...)]` (Bevy 0.15+) on project-defined marker components (e.g. `TriggerZone`, `FadingLight`, `LevelEntity`) so that inserting the primary component automatically inserts its mandatory companions; reduces manual bundle construction in spawners and makes component contracts explicit at the type level
- [ ] **Consistent `assets.ron` entry shapes** — `models` entries use `(path: "...")`, `textures` are bare strings, `audio` uses `(path: "...", volume: ...)`; unifying the shapes reduces copy-paste errors and parse confusion; requires schema version bump
- [ ] `Condition` expressions in rules (`score >= 10`, `variable == "value"`) — currently only event matching
- [ ] Hot-reload for `.scene.ron` and `rules.ron` in native debug builds

### Groups & Membership
- [ ] **Group system — Tier 1 (factions, teams, parties)** — generic RON-defined `GroupDef` (kind, max_members, default_stance); `LoadedGroups` global resource mapping group-id → member set + `GroupMembership` component on entities; `AddToGroup` / `RemoveFromGroup` / `DisbandGroup` actions; `group.joined:{id}:{entity}` / `group.left:{id}:{entity}` / `group.full:{id}` events into the existing pipeline; faction stance rules (Hostile/Neutral/Friendly) for AI targeting; useful standalone in single-player for factions, arena teams, and NPC parties. Tier 2 (guild, chat, raid hierarchy) deferred to Beta 0.6 networking milestone. See `planning/features/group_system_tier1.md`

### Gameplay Capabilities
- [ ] **Equipment system — v2 visual mesh attachment** — `model_attachment`/`AttachmentDef` for equipping visible mesh props (weapons, helmets) to named bone sockets; requires GLB skeleton socket authoring and runtime bone-query APIs. _Dep: Equipment system v1._
- [ ] **Grid system** — square, hexagonal (flat-top / pointy-top), and triangular cell layouts; `grid: GridDef` on scene RON; `(col, row)` addressing for all types (axial for hex); `PlaceOnGrid` / `StartGridMove` / `SetCellPassable` / `FindPath` actions; A* with node cap; `GridPosition` component; `grid.cell_entered` / `grid.move_complete` / `grid.path_blocked` events; Gizmos debug overlay; WASM-compatible. See `planning/features/grid_system.md`

---

## Done (reference)

### June 2026
- [x] **Icon washed-out fix** — sRGB established as universal RON color convention; `Color::linear_rgba` fixed in shop icon spawn and decal base color storage; 14 doc labels corrected. See `planning/features/done/icon_washed_out_fix.md`
- [x] **Inventory & item system** — `items/items.ron` catalog; `PlayerInventory` resource; `InventoryPanel`/`ShopPanel`/`ContainerPanel` UI nodes; `OpenShop`/`BuyItem`/`OpenContainer`/`TakeAllFromContainer` actions; `MerchantDef`; currency via stat system. See `planning/features/done/inventory_item_system.md`
- [x] **Dialogue system** — `.dialogue.ron` assets; `DialoguePanel` UI node; `StartDialogue`/`EndDialogue`/`AdvanceDialogue` actions; branching choices; `{self}`/`{target}` substitution; auto-wired to `entity.interacted`. See `planning/features/done/dialogue_system.md`
- [x] **NPC aggro-on-hit + Investigating state** — `NpcHitQueue` relay; `Investigating` FSM state walks to attacker's last-known position; `investigate_timeout_secs` per prefab; `npc.investigating`/`npc.investigation_failed` events. See `planning/features/done/npc_aggro_on_hit.md`
- [x] **Camera shake** — `Action::CameraShake { duration_secs, intensity }`; procedural position shake on orbit camera; wired to player-hit events in `3rd_person_game_demo`. See `planning/features/done/camera_shake.md`
- [x] **Spider NPC behavior polish** — death animation holds last frame (no `duration`, `stop_action: "npc_revive"`); respawn clears death pose via `PlayAnimationOn(clip: "npc_revive")` in "alive" entry_actions; spider stops instantly on death (direct `StatMap` health check in `npc_behavior_system` eliminates 2-frame relay delay); patrol walk/idle/waypoint-pause working; attack override fires only on player reach; corpse hide/respawn timing designer-configurable via `EmitEventAfterDelay` in behavior RON.
- [x] **Per-prefab `select_aim_height` for click targeting** — `select_aim_height: f32` (default 1.0) on `PrefabDef`; `SelectAimHeight(f32)` component; fixes snake/spider click hitboxes floating 0.6–1 m above the visible body; orc/zombie unaffected (no field set → default 1.0)
- [x] **Target indicator color by category and per-prefab override** — layered color resolution: prefab `indicator_color` (direct RGBA) > `indicator_category` key in scene `named_colors` > scene `color` fallback; material memo keyed by resolved colour bits — `d945ea9`
- [x] **Embed capability shaders & fix hardcoded shared asset paths** — `stat_radar`, `foliage` (×2), `flame_material`, `pool_flame` shaders embedded via `include_str!()`; foliage fabricated texture fallback removed; CLI validate now cross-checks foliage `leaf_texture` keys; fixture RON kind-field format fixed — `planned at b1ca9b6`
- [x] **Creature collider sizing — snake & spider** — `collider_height`/`collider_radius` tuned on `enemy_snake` (0.8/0.3) and `enemy_spider` (1.2/0.4); eliminates oversized humanoid capsule blocking player approach.
- [x] **NPC dead-state fix + `ResetToSpawn` action** — visibility guard in `npc_behavior_system` stops ghost hitboxes; new `ResetToSpawn(entity)` action teleports NPC to origin on respawn; wired in all three enemy behavior files.
- [x] **Hitbox debug toggle** — `debug_target_hitboxes` GameVariable draws 0.5 m yellow gizmo spheres at `ClickSelectable` aim points; toggled via two HUD buttons in `3rd_person_game_demo`.
- [x] **Multi-source animations (animation packs + shared-rig mesh variants)** — `animation_sources: [catalog_key, ...]` on `AnimationPolicy.ron`; animation graph merges clips from all listed GLBs plus the model GLB; enables splitting clips across domain packs (locomotion, magic, gun) and sharing one pack across mesh variants with identical bone names; backwards-compatible.
- [x] **Character select + runtime player spawn via `Action::Spawn`** — WoW-style character selection screen in `3rd_person_game_demo`; player prefabs spawned at runtime via `Action::Spawn` with camera + controller assembled from prefab RON; `ActiveTonemapping` threads scene tonemapping to the orbit camera; FSM states carry character choice across the scene load boundary; display-only `preview_*` prefabs prevent stray cameras.
- [x] **Character select idle animations + model consolidation** — all three preview characters play idle animations on the select screen; model orientation fixed via `model_fixes.ron` (180° Y); zombie GLB scene root renamed to match animation pack root for `animation_sources` retargeting; mesh-only duplicate GLB subfolders removed (character_female, zombie); catalog keys unified (`character_male_mesh` → `character_male` etc.); docs updated: minimum-1-animation GLB requirement and 180° Y rotation convention.
- [x] **3rd-person orbit camera — sky shows tan ground on steep pan** — reduced `max_pitch` from 1.5 → 0.9 rad; removed redundant fallback camera spawn when `spawn_points` present; docs updated.
- [x] **GLB Splitter tool** — `tools/glb_splitter/split.py`; splits a monolithic GLB into a mesh-only file (buffer-compacted) and named animation-group files; `--mesh-only`, `--one-per-clip`, `--by-prefix`, `--group` modes; preview tool + commit hook skip animation-only GLBs — `96326d7`
- [x] **100+ shared GLB models + AVIF previews** — props, characters, creatures; GLB preview tool fixes (fallback materials, mesh-only bounds, proportional clip planes, pixel-count blank detection) — `7486104`
- [x] **Snake and spider GLB enemies** — `kind: Actor` NPC support; `enemy_snake` + `enemy_spider` with AI, hit effects, patrol waypoints in `3rd_person_game_demo` — `df8c94b`
- [x] **NpcDef collider sizing + stat_overrides in spawn** — `collider_radius`/`collider_height` on `NpcDef`; `spawn_prefab_instance` validates and applies stat overrides; NPC integration test — `1112fc4`
- [x] **Dynamic spawn missing components** — `motion`, `stat_label`, `world_stat_bar` now attach on dynamic `Action::Spawn` entities via `DynamicStatUiQueue` — `8005606`
- [x] **Mute audio toggle + master volume** — `ToggleMute`/`SetVolume` actions; `AudioConfig` on `ProjectConfig`; `SyncAudioState` for label init — `5df25da`
- [x] **Per-instance stat overrides on SceneEntityDef** — `stat_overrides: HashMap<String, f32>` on placed scene entities; unknown keys warn at load time — `4416545`
- [x] **Mouse click-to-select + Tab targeting** — screen-space proximity selection; Tab/Shift-Tab nearest-first cycling; `{target}` substitution; `SetTarget`/`ClearTarget` actions; `target.*` events — `34bc77d`
- [x] **Consolidate entity-spawn component insertion** — `tag_spawned_entity` helper owns all spawn metadata; all 7 spawn sites route through it — `0afd6a0`
- [x] **Skill action bar (1–9)** — `ActionBar` UI node; cooldown overlay; `CooldownMap`; `ShowFloatingText` action; demo in `primitive_world` — `187463d`
- [x] **stats_demo + blank_project starter** — standalone stats showcase; minimal copy-and-rename starter project — `fb30a6d`
- [x] **Foliage square shadows fix** — `NotShadowCaster` on cluster entities; `cast_shadows` RON opt-in — `c4b1ef8`
- [x] **Stylized foliage** — `kind: Foliage` prefab type; billboard leaf cards; `FoliageMaterial` WGSL; `height_bias`/`seed` for crown shape — `a1e7ffd`
- [x] **PrefabCatalog schema v2** — `PrefabKind` enum; `PrimitiveShapeKind` typed enum; `PREFAB_CATALOG_SCHEMA_VERSION` → 2 — `fba367e`

### May 2026
- [x] **Particle mesh buffer recreation** — in-place `Mesh::attribute_mut` replaces per-frame Vec rebuild; eliminates per-frame allocations in pool renderer — `e03c945`
- [x] **Animation player entity lookup cache** — `last_player_entity` fast-path in `animation_playback_system`; O(1) common case — `c72c57b`
- [x] **Flipbook / sprite sheet animation** — UV sub-rect baked per-frame in CPU pool renderer; `explosion_4x4.png` sheet — `9d76d59`
- [x] **Quality tiers + particle budget** — `SetParticleQuality` action; priority (`Player`/`Npc`/`Ambient`); live-count cap; portal navigation; Arcane Observatory demo — `1bc97d9`
- [x] **Ground decals / AoE projections** — `ProjectDecal` action; AoE circles, impact splats, cast indicators — `786800f`
- [x] **Extended particle behaviours** — rotation over lifetime, non-uniform scale, Ring/Sphere/Line/Arc emitters, velocity curves — `d14d1b3`
- [x] **Dynamic effect lights** — `light` block on `EffectDef` spawns a temporary fading `PointLight` — `7a87684`
- [x] **Ironhold CLI enhancements** — `watch`, `stats`, `query actions/events`, `validate --strict` orphan detection; `--json` throughout — `a67c450`
- [x] **Ironhold CLI** — `validate`, `inspect glb/texture/audio`, `query prefabs/effects/scenes/rules`; cross-file checks; exit codes — `61bbc26`
- [x] **Staggered entity spawning + web spawn hang fix** — `PendingEntitySpawns` drains 2/frame; `PreloadPrefab` on `scene.ready` eliminates the quick_scene hang — `b812b26`
- [x] **Stat display — per-entity stat routing** — `resolve_stat` helper; dotted keys route to `StatMap`, plain to global `LoadedStats`; `StatLabelMarker` floating labels — `2015a1e`
- [x] **World-space pixel stat bar** — unified `WorldStatBar` with Ascii and Pixel styles — `98ca5d0`
- [x] **Game stats Phase 2: buffs and modifiers** — named modifier templates; additive/multiplicative/override kinds; stacking rules; `ApplyModifier`/`RemoveModifier` actions — `84b5d15`
- [x] **Stat display — StatRadar** — `StatRadar` UI node (3–12 axes); WGSL polar-coordinate shader via `UiMaterial`; polygon grid — `763ac72`
- [x] **Stat display — StatBar + StatSpread** — `StatBar`/`StatSpread` UI node types; colour bands; change-detection update — `57e1628`
- [x] **Game stats Phase 2a: stat templates** — `stat_templates` on `PrefabDef`; `StatMap` component; dot-routing `ModifyStat`/`SetStat`; threshold/regen — `5292961`
- [x] **Animation T-pose on landing** — `animation_playback_system` detects `AnimationPlayer` entity change and resets `graph_initialized`; investigation resolved — `9ed1917`
- [x] **PrefabComponents deny_unknown_fields** — `#[serde(deny_unknown_fields)]` on `PrefabComponents`; clear field-name error on typos — `7144420`
- [x] **Terrain path consolidation** — `TerrainConfigV2` is now single struct (schema + runtime `Component`); `TerrainConfig` removed; `scale.z` bug fixed — `ece80c1`
- [x] **`implicit_some` RON extension** — `ImplicitRonPlugin` enables `implicit_some` globally; 671 `Some()` wrappers removed — `4590d77`
- [x] **Nested prefabs — mesh support** — `spawn_primitive_children` dispatches on `kind`; GLB props nestable in composite prefabs — `d8d0ed5`
- [x] **Nested prefabs** — `children: [key, ...]` references; cycle detection; `village` demo in `primitive_world` — `6625775`
- [x] **Particle effect spawning v1** — campfire, torch, explosions, UV distort/scroll; pool renderer; multi-layer `EffectDef` — `27f097d`
- [x] **Silent failure diagnostics** — warn when `rules.ron` is skipped by `state_machine_path`; warn on `Spawn` position+spawn_point conflict; debug log when no rule matches an event — `762dfc1`
- [x] **Game stats Phase 1: core stat model** — `StatDef` (base/min/max/regen/thresholds); `LoadedStats` resource; `ModifyStat`/`SetStat` actions; threshold events — `270ff7e`

### April 2026
- [x] **Beta 0.4 — Entity Logic FSM** — per-entity `.behavior.ron`; `{self}` substitution; `TriggerZone`; `Interactable`; `SetVariable`/`IncrementVariable`; data-bound UI labels; `GameVariables` resource; `entity_logic_demo` project — `2235add`
- [x] **Beta 0.3 — Global Logic FSM v1** — `state_machine.ron`; state transitions; FSM-driven scene loading — `dca4fd0`

### January 2026
- [x] **Beta 0.2 — Event/Action Bus refactor** — `Action` enum moved to schema layer; rule-based bindings in RON; `message_interpreter_system` data-driven — `9ef2547`
- [x] **Beta 0.1 — Baseline Runtime** — declarative RON schemas for project config and scenes; `schema_version` enforcement; native + WASM builds stable — `434fb9a`
