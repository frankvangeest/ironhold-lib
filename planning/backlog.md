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

## Queued

### Beta 0.4 — Entity Logic (FSM v1)
- [ ] `Behavior` component referencing a `StateMachineAsset` per entity
- [ ] Trigger zone messages: `entity.entered:{id}` / `entity.exited:{id}` (Rapier sensor)
- [ ] Interaction message: `entity.interacted:{id}` (player in range + action key)
- [ ] Example: door open/close driven by entity FSM
- [ ] Example: NPC idle-wander with simple pickup interaction
- [ ] Docs + integration tests

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
- [ ] `Action::SetVariable` / `Action::IncrementVariable` — write to named runtime variables from RON rules
- [ ] `Condition` expressions in rules (`score >= 10`, `variable == "value"`) — currently only event matching
- [ ] Hot-reload for `.scene.ron` and `rules.ron` in native debug builds

### Gameplay Capabilities
- [ ] Dialogue system — multi-step NPC conversation trees in RON; emits `dialogue.ended:{id}` trigger
- [ ] Inventory / item system — `AddItem`, `RemoveItem`, `HasItem` condition
- [ ] Quest / objective tracker — RON-defined objectives, `CompleteObjective`, `FailObjective` actions
- [ ] Particle effect spawning via `Action::SpawnEffect(String)` — catalog-keyed effect prefab
- [ ] Camera shake action (`Action::CameraShake { duration, intensity }`)
- [ ] Timeline / sequencer — scripted cutscene playback from a RON timeline asset

### UI
- [ ] UI element types beyond `Button`: `Label`, `Image`, `ProgressBar`, `Panel`
- [ ] Data-bound UI — bind label text or bar fill to a named variable
- [ ] UI layout — stack/flex layout or anchor-based positioning replacing raw pixel coords
- [ ] Font + theme config per project

### Terrain
- [ ] Terrain snap — `snap_to_terrain: true` on entity def makes Y an offset above terrain surface; design: `planning/features/terrain_snap.md`
- [ ] Terrain chunked streaming — generate and load only chunks within a player radius; unload distant chunks; requires chunk-aware terrain capability rewrite

### Rendering & Assets
- [ ] LOD (level of detail) for terrain and models — distance-based mesh swap
- [ ] Decal system — project a texture onto geometry without modifying meshes
- [ ] Animated texture support in `CustomMaterial` (frame index via time uniform)
- [ ] Water / reflective plane primitive with animated normal map
- [ ] Post-process pass authoring — expose WGSL post-process shader slot per scene

### Performance
- [ ] Staggered entity spawning — drain a `PendingEntitySpawns` queue at N entities/frame instead of spawning all in one frame; spreads WebGPU pipeline compilations across frames, reducing peak WASM frame time (fixes 1400ms+ INP on scenes with many unique custom shaders)

### Profiling & Diagnostics
- [ ] Diagnostics HUD — F3 overlay: FPS, frame time, entity count, draw calls, triangles, CPU/RAM (native); design: `planning/features/diagnostics_hud.md`
- [ ] Tracy integration — `--features trace_tracy` on native runner; per-system CPU timeline; design: `planning/features/tracy_integration.md`

### Tools
- [ ] `tools/ron_formatter/` — auto-format `.ron` files (indentation, trailing commas)
- [ ] Live reload server — watch `assets/` and push scene reload to running native build via IPC
- [ ] GLB batch inspector — produce a markdown table of node names, animations, and materials for a whole folder

---

## Done (reference)

- [x] Beta 0.1 — Baseline Runtime
- [x] Beta 0.2 — Event/Action Bus refactor
- [x] Beta 0.3 — Global Logic (FSM v1)
- [x] Three-point warm lighting defaults for GLB preview tool (`--light-strength 0.3`)
