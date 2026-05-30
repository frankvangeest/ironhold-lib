# Backlog

> **How this works**
> - Items progress: `Icebox → Queued → Active → Done`
> - Simple items live here as bullet points. Anything needing design lives in `features/`.
> - This file tracks *what to build next*, not *how* — keep it skimmable.
> - Roadmap and milestone gates: see `docs/50_roadmap_and_milestones.md`
> - Implementation status: see `docs/STATUS.md`

---

## Active

---

## Bugs

- [ ] **uphill jump lock** — when jumping against an uphill slope, the player can land in a state where `jump` never re-triggers: the character controller reports ground contact but the slope normal keeps the jump cooldown active. Suspected cause: Rapier's ground-contact normal threshold in the character controller or the jump cooldown not resetting when sliding contact ends. Reproduce: 3rd_person_game_demo, run toward any hill and spam jump while ascending.
- [x] **`PrefabComponents` silently drops unknown fields** — added `#[serde(deny_unknown_fields)]`; RON parse now fails with a clear field name on typos or unknown fields; two regression tests added.

---

## Queued

### Tooling

- [ ] **Ironhold CLI** — `ironhold validate / schema / query / inspect` commands; new `crates/ironhold_cli`; shared validation module from `ironhold_core`; `--json` flag for AI agent use; `inspect glb` lists animations/meshes/materials, `inspect texture` reports dimensions/format, `inspect audio` reports duration/sample-rate. See `planning/features/ironhold_cli.md`

### Particle System v2

- [ ] **Bloom / post-processing in scene RON** — BLOCKED: Bevy's `Bloom` requires `#[require(Hdr)]`; HDR breaks the WASM build. Needs native-only guard or custom SDR pass before this can ship. See `planning/features/particle_bloom.md`
- [x] **Dynamic effect lights** — `light` block on EffectDef spawns a temporary fading PointLight. See `planning/features/particle_dynamic_lights.md`
- [x] **Extended particle behaviours** — rotation over lifetime, non-uniform scale, Ring/Sphere/Line/Arc emitters, velocity curves. See `planning/features/done/particle_extended_behaviours.md`
- [x] **Ground decals / AoE projections** — `ProjectDecal` action for AoE circles, impact splats, cast indicators. See `planning/features/done/particle_ground_decals.md`
- [x] **Flipbook / sprite sheet animation** — UV sub-rect baked per-frame in CPU pool renderer; `explosion_4x4.png` sheet; Flipbook Pad station in particles_demo. See `planning/features/done/particle_flipbook.md`
- [ ] **Shared effect library** — `assets/shared/effects/` with reusable effects and per-project overrides. See `planning/features/particle_shared_library.md`

### Beta 0.5 — Deterministic Tick + Replay
- [ ] Fixed-tick schedule for gameplay systems (separate from render tick)
- [ ] Deterministic RNG resource (seeded, replaces any `rand` usage in gameplay)
- [ ] `InputAction` stream capture to file (native)
- [ ] Replay playback from captured stream
- [ ] Snapshot/restore stub for core gameplay state
- [ ] Determinism constraints doc

### Beta 0.6 — Networking Prototype
- [ ] Server-authoritative input relay (client sends inputs, server simulates)
- [ ] Client interpolation pass
- [ ] Connect / disconnect flow (minimal, no lobby)
- [ ] Network message protocol doc
- [ ] Latency/jitter test harness
- [ ] Multiplayer demo scene

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
- [ ] **`ChildOf` hierarchy migration** — migrate from `Children`/`Parent` (Bevy pre-0.16 API) to the `ChildOf` relationship component (Bevy 0.16+); the animation system queries `&Children` to walk GLB hierarchies and all spawners use `with_children()` — these need updating to the forward-looking API before a future Bevy upgrade removes the compat shim
- [ ] **Required components on project-defined components** — adopt `#[require(...)]` (Bevy 0.15+) on project-defined marker components (e.g. `TriggerZone`, `FadingLight`, `LevelEntity`) so that inserting the primary component automatically inserts its mandatory companions; reduces manual bundle construction in spawners and makes component contracts explicit at the type level
- [ ] **Typed primitive shape field** — split the `model:` field on `kind: "primitive"` prefabs into a separate typed `shape:` field (e.g. `shape: Cuboid`) so it is clearly distinct from the asset catalog key used by `kind: "actor"` and `kind: "prop"`; requires schema version bump; breaking change — needs design doc
- [ ] **Consistent RON enum casing** — unify quoted magic strings (`kind: "actor"`) and bare enum variants (`kind: Standard(...)`) to a single convention across the schema; requires schema version bump; breaking change — needs design doc
- [ ] **Consistent `assets.ron` entry shapes** — `models` entries use `(path: "...")`, `textures` are bare strings, `audio` uses `(path: "...", volume: ...)`; unifying the shapes reduces copy-paste errors and parse confusion; requires schema version bump
- [x] `Action::SetVariable` / `Action::IncrementVariable` — write to named runtime variables from RON rules
- [ ] `Condition` expressions in rules (`score >= 10`, `variable == "value"`) — currently only event matching
- [ ] Hot-reload for `.scene.ron` and `rules.ron` in native debug builds

### Gameplay Capabilities
- [x] **Game stats — Phase 2a: stat templates** — `stat_templates` on `PrefabDef`; `StatMap` component (IndexMap, Clone); dot-routing `ModifyStat`/`SetStat`; threshold/regen for instance stats; `{self}` in stat keys; goblin guard moves to behavior file; composite primitive `behavior` field fixed; integration tests + docs; design: `planning/features/stat_templates.md`
- [x] **Game stats — Phase 1: core stat model** — `StatDef` (base/min/max/regen/thresholds), `LoadedStats` resource, `ModifyStat`/`SetStat` actions, threshold events into existing pipeline; design: `planning/features/game_stats_core.md`
- [x] **Game stats — Phase 2: buffs and modifiers** — named modifier templates, additive/multiplicative/override kinds, stacking rules, soft_max, `ApplyModifier`/`RemoveModifier` actions; design: `planning/features/game_stats_buffs.md` _(depends on Phase 1)_
- [x] **Stat display — health bars and stat spreads** — `StatBar` and `StatSpread` UI node types in scene RON, colour bands, change-detection update; design: `planning/features/game_stats_display.md` _(depends on Phase 1)_
- [x] **Stat display — radar chart** — `StatRadar` UI node (3–12 axes), WGSL polar-coordinate shader via `UiMaterial`, straight-edged polygon grid (no circles), `stat_radar_update_system`; `primitive_world` demo: 5-stat pentagon (health/mana/stamina/strength/speed) on Key C overlay
- [ ] **Stat radar labels** — render stat-key labels at each axis tip of `StatRadar`; blocked by UI text on `UiMaterial` nodes; low priority
- [x] **Stat display — per-entity stat routing** — `resolve_stat(key, &LoadedStats, &Query<(&SpawnId, &StatMap)>)` shared helper; dotted keys route to entity `StatMap`, plain keys to global `LoadedStats`; all three update systems (`StatBar`, `StatSpread`, `StatRadar`) + new `stat_label_update_system` use it; `StatLabelMarker` component enables floating world-space health labels; `primitive_world` attack dummies demonstrate the feature
- [x] **World-space stat bar — Pixel style** — see `planning/features/world_pixel_stat_bar.md` _(design done, ready to implement)_
- [ ] **World-space icon stat bar** — row of per-cell sprites (hearts, shields, or any catalog icon) above entities, `WorldIconBarDef` schema field on `PrefabDef`; full cells show filled icon, empty cells show depleted icon; requires sprite-sheet or paired asset catalog entries; design needed (asset reference format, partial-cell handling)
- [ ] Dialogue system — multi-step NPC conversation trees in RON; emits `dialogue.ended:{id}` trigger
- [ ] Inventory / item system — `AddItem`, `RemoveItem`, `HasItem` condition
- [ ] Quest / objective tracker — RON-defined objectives, `CompleteObjective`, `FailObjective` actions
- [x] Particle effect spawning via `Action::SpawnEffect` — see `planning/features/particle_effects.md`
- [ ] Camera shake action (`Action::CameraShake { duration, intensity }`)
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
- [ ] **Toon / cel shading (3-tone, 4-tone, 5-tone)** — WGSL-only `CustomMaterial` shaders for stylized discrete light bands; 3- and 4-tone fit current uniform budget; 5-tone uses a ramp texture; design: `planning/features/toon_shading.md`
- [ ] LOD (level of detail) for terrain and models — distance-based mesh swap.
    - Auto generated LODs of assets (only for web builds using IndexedDB). 
    - A flag per asset and/or per scene? 
    - Web workers or something similar will likely be needed for web builds, because it must not block the main thread. 
    - Seamless switching between LODs?
    - Use bevy meshlets?
- [ ] Decal system — project a texture onto geometry without modifying meshes
- [ ] Animated texture support in `CustomMaterial` (frame index via time uniform)
- [ ] Water / reflective plane primitive with animated normal map
- [ ] Post-process pass authoring — expose WGSL post-process shader slot per scene

### Performance
- [ ] **Extend pipeline warmup to Text2d and UI pipelines** — spawn hidden warmup entities for `Text2d` and UI `Node` at scene load to pre-compile the 2D/UI GPU pipelines, eliminating WASM frame spikes on first text/UI render; design: `planning/features/pipeline_warmup_2d_ui.md`
- [ ] **Discrete LOD steps for depth-scaled label font sizes** — snap `base_font_size * scale` to a small fixed set (e.g. 100 %, 75 %, 50 %, 25 %) instead of rounding to every integer; bounds atlas slot count to ~4 per label regardless of depth range, at the cost of a slight stepping artefact on slow zoom. Integer rounding + 0.5-threshold guard already fix the per-frame atlas upload problem; this is a further atlas-memory micro-optimisation.
- [x] Staggered entity spawning — `PendingEntitySpawns` queue drains at `SPAWNS_PER_FRAME = 2`/frame via `drain_spawn_queue_system`; spreads WebGPU pipeline compilations across frames for wave spawns

### Profiling & Diagnostics
- [ ] Diagnostics HUD — F3 overlay: FPS, frame time, entity count, draw calls, triangles, CPU/RAM (native); design: `planning/features/diagnostics_hud.md`
- [ ] Tracy integration — `--features trace_tracy` on native runner; per-system CPU timeline; design: `planning/features/tracy_integration.md`

### Designer Experience
- [ ] **Extend `entity_logic_demo`** — add one clearly labeled station per behavior concept: multi-state FSM behavior file with `EmitEventAfterDelay` loop (goblin-guard pattern), a timed-door sequence (`EmitEventAfterDelay` chain), side-by-side trigger zone enter vs. interactable [F] comparison, and a `global_on` example showing project-wide vs. entity-local events; modeled on the station-per-concept layout of `particles_demo`
- [ ] **`stats_demo` project** — standalone demo project showcasing the full stats system in one place: health/mana bars, stat spread widget, radar chart, world-space pixel bars, buffs and modifiers, damage popups, and per-entity stat routing; the current `primitive_world` mixes all of this with geometry and AI work making it hard to use as a reference
- [ ] **`ui_demo` project** — standalone demo project for every UI capability: buttons, data-bound labels with `SetVariable`/`IncrementVariable`, overlays, pause-menu pattern, and all stat display widget types; gives designers a single project to copy patterns from
- [ ] **`audio_demo` project** — standalone demo project focused on audio authoring: `PlaySound`, `PlayMusicLoop`, `SetVolume`, stop/loop patterns, and how audio is triggered from RON rules; no existing project makes audio its primary focus
- [ ] **`scene_transitions_demo` project** — standalone demo project with 3–4 scenes wired through a state machine, demonstrating portal navigation, scene overlays, preloading, and multi-scene project config; distinct from `particles_demo` where portal navigation is a side feature
- [ ] **Blank starter project template** — a minimal `blank_project` under `assets/projects/` containing only the required files (project config, one empty scene, empty prefab catalog, empty asset catalog, empty rules), no terrain, no models, and no dummy fields; the canonical copy-and-rename starting point so new projects do not inherit `quick_scene` noise
- [ ] **Schema version v2→v3 migration guide** — add a "Migrating from v2 to v3" section in `docs/20_data_formats.md` covering: rename `rules_path` → `state_machine_path`, bump `schema_version` to `3`, convert `rules.ron` to the FSM format, and the warning to expect if both files coexist
- [ ] **Magic-string event/action validator** — `tools/ron_validator/` CLI that cross-checks event names used in `rules.ron` / `state_machine.ron` against the set emitted by capabilities and reports unknown event keys before runtime; eliminates silent no-ops from typos in event names

### Tools
- [ ] `tools/ron_formatter/` — auto-format `.ron` files (indentation, trailing commas)
- [ ] Live reload server — watch `assets/` and push scene reload to running native build via IPC
- [ ] GLB batch inspector — produce a markdown table of node names, animations, and materials for a whole folder

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
