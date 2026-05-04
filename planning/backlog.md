# Backlog

> **How this works**
> - Items progress: `Icebox → Queued → Active → Done`
> - Simple items live here as bullet points. Anything needing design lives in `features/`.
> - This file tracks *what to build next*, not *how* — keep it skimmable.
> - Roadmap and milestone gates: see `docs/50_roadmap_and_milestones.md`
> - Implementation status: see `docs/STATUS.md`

---

## Active

_(nothing in flight right now — pick from Queued)_

---

## Bugs

- [ ] **uphill jump lock** — when jumping against an uphill slope, the player can land in a state where `jump` never re-triggers: the character controller reports ground contact but the slope normal keeps the jump cooldown active. Suspected cause: Rapier's ground-contact normal threshold in the character controller or the jump cooldown not resetting when sliding contact ends. Reproduce: 3rd_person_game_demo, run toward any hill and spam jump while ascending.
- [x] **quick_scene web spawn hang** — `Action::PreloadPrefab("enemy_orc_melee")` fires on `scene.ready:main` so the orc GLB is decoded during scene load before the button is reachable; `PreloadedGlbHandles` resource keeps the handle alive. A `PendingEntitySpawns` queue (drained at 2/frame) was added simultaneously — it doesn't eliminate the remaining ~300 ms WebGPU pipeline-compile stall on first render, but caps per-frame stalls to 2 entities for wave spawns. A further phantom-entity warmup approach could eliminate the pipeline-compile stall if that residual freeze proves unacceptable.
- [x] **animation T-pose on landing** — Root cause confirmed: Bevy's `SceneSpawner` re-spawns the GLTF hierarchy mid-session (sub-assets complete loading slightly after first spawn; more frequent on WASM/cold builds). `animation_playback_system` now detects when the `AnimationPlayer` entity changes and resets `graph_initialized` so step 1 re-initializes on the new entity within 2-3 frames. See `planning/investigations/resolved/animation_tpose.md`.

---

## Queued

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
- [x] `Action::SetVariable` / `Action::IncrementVariable` — write to named runtime variables from RON rules
- [ ] `Condition` expressions in rules (`score >= 10`, `variable == "value"`) — currently only event matching
- [ ] Hot-reload for `.scene.ron` and `rules.ron` in native debug builds

### Gameplay Capabilities
- [ ] **Game stats — Phase 1: core stat model** — `StatDef` (base/min/max/regen/thresholds), `LoadedStats` resource, `ModifyStat`/`SetStat` actions, threshold events into existing pipeline; design: `planning/features/game_stats_core.md`
- [ ] **Game stats — Phase 2: buffs and modifiers** — named modifier templates, additive/multiplicative/override kinds, stacking rules, soft_max, `ApplyModifier`/`RemoveModifier` actions; design: `planning/features/game_stats_buffs.md` _(depends on Phase 1)_
- [ ] **Stat display — health bars and stat spreads** — `StatBar` and `StatSpread` UI node types in scene RON, colour bands, change-detection update; design: `planning/features/game_stats_display.md` _(depends on Phase 1)_
- [ ] Dialogue system — multi-step NPC conversation trees in RON; emits `dialogue.ended:{id}` trigger
- [ ] Inventory / item system — `AddItem`, `RemoveItem`, `HasItem` condition
- [ ] Quest / objective tracker — RON-defined objectives, `CompleteObjective`, `FailObjective` actions
- [ ] Particle effect spawning via `Action::SpawnEffect(String)` — catalog-keyed effect prefab
- [ ] Camera shake action (`Action::CameraShake { duration, intensity }`)
- [ ] Timeline / sequencer — scripted cutscene playback from a RON timeline asset

### UI
- [ ] UI element types beyond `Button`: `Label`, `Image`, `ProgressBar`, `Panel`
- [x] Data-bound UI labels — `bind`/`format` fields on labels + `GameVariables` resource; `Action::SetVariable` / `Action::IncrementVariable` let designers write arbitrary variables from RON; `DebugState.score` derived from `GameVariables["score"]`
- [ ] UI layout — stack/flex layout or anchor-based positioning replacing raw pixel coords
- [ ] Font + theme config per project

### Terrain
- [ ] Terrain snap — `snap_to_terrain: true` on entity def makes Y an offset above terrain surface; design: `planning/features/terrain_snap.md`
- [ ] Terrain chunked streaming — generate and load only chunks within a player radius; unload distant chunks; requires chunk-aware terrain capability rewrite
- [x] **Terrain path consolidation** — `TerrainConfigV2` is now the single struct (schema + runtime `Component`); `TerrainConfig` removed. Scene loader spawns `terrain_v2.clone()` directly. Fixed **scale.z bug**: `generate_terrain_mesh_raw` now takes separate `scale_x`/`scale_z` so asymmetric terrain is no longer distorted.

### Rendering & Assets
- [ ] **Toon / cel shading (3-tone, 4-tone, 5-tone)** — WGSL-only `CustomMaterial` shaders for stylized discrete light bands; 3- and 4-tone fit current uniform budget; 5-tone uses a ramp texture; design: `planning/features/toon_shading.md`
- [ ] LOD (level of detail) for terrain and models — distance-based mesh swap
- [ ] Decal system — project a texture onto geometry without modifying meshes
- [ ] Animated texture support in `CustomMaterial` (frame index via time uniform)
- [ ] Water / reflective plane primitive with animated normal map
- [ ] Post-process pass authoring — expose WGSL post-process shader slot per scene

### Performance
- [x] Staggered entity spawning — `PendingEntitySpawns` queue drains at `SPAWNS_PER_FRAME = 2`/frame via `drain_spawn_queue_system`; spreads WebGPU pipeline compilations across frames for wave spawns

### Profiling & Diagnostics
- [ ] Diagnostics HUD — F3 overlay: FPS, frame time, entity count, draw calls, triangles, CPU/RAM (native); design: `planning/features/diagnostics_hud.md`
- [ ] Tracy integration — `--features trace_tracy` on native runner; per-system CPU timeline; design: `planning/features/tracy_integration.md`

### Designer Experience

### Tools
- [ ] `tools/ron_formatter/` — auto-format `.ron` files (indentation, trailing commas)
- [ ] Live reload server — watch `assets/` and push scene reload to running native build via IPC
- [ ] GLB batch inspector — produce a markdown table of node names, animations, and materials for a whole folder

---

## Done (reference)

- [x] **`implicit_some` RON extension** — `ImplicitRonPlugin` in `schema/ron_loader.rs` enables `implicit_some` globally via `ron::Options`; 671 `Some()` wrappers removed from all project `.ron` files; `tools/migrate_implicit_some.py` one-shot migration script included; no per-file directives needed
- [x] **Nested prefabs — mesh support** — `spawn_primitive_children` dispatches on `kind`: actor/prop loads GLB via `spawn_prefab_instance`, single-shape primitive builds one mesh; `rock_deco` GLB prop nested in `village` demo; design: `planning/features/nested_prefabs_mesh_support.md`
- [x] **Nested prefabs** — `children` entries reference named prefabs by key; multiplicative Bevy hierarchy; cycle detection; `village` prefab demo in `primitive_world`; design: `planning/features/nested_prefabs.md`
- [x] Beta 0.1 — Baseline Runtime
- [x] Beta 0.2 — Event/Action Bus refactor
- [x] Beta 0.3 — Global Logic (FSM v1)
- [x] Beta 0.4 — Entity Logic (FSM v1): per-entity `.behavior.ron`, `{self}` substitution, `TriggerZone`, `Interactable`, `PlayAnimationOn`/`EmitEvent`, `entity_logic_demo` project
- [x] Three-point warm lighting defaults for GLB preview tool (`--light-strength 0.3`)
