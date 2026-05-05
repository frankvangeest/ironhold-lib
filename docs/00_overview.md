# Ironhold-lib: Overview

> **Doc type:** Design + Overview
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet
>
## What is Ironhold-lib?

Ironhold-lib is a cross-platform (native + web/WASM) game runtime built on **Bevy**. The core goal is to enable **data-defined games**: creators can build and iterate on projects and scenes using data files (RON) and assets, without recompiling the engine for most content changes. 

## Goals (vision)

1. **Data-driven gameplay** 🧭  
   Most game behavior should be declared in data files (RON), not hard-coded.
2. **Single runtime model** 🧭  
   A shared, cross-platform logic layer that behaves the same on native and web.
3. **Composable capabilities** 🧭  
   Reusable “capability blocks” (player controller, cameras, UI flows, triggers, etc.) that can be enabled/configured by data.
4. **Schema evolution** 🧭  
   Versioned data formats with validation and migration paths.
5. **Determinism-ready foundation** 🧭  
   A path toward deterministic simulation for multiplayer/rollback, without requiring determinism everywhere from day one.
6. **Support for multiple 3D game formats** 🧭  
   Support for multiple 3D game formats, such as 3rd Person, First Person, Platformer and Strategy games.

## Big ideas

### 1) Data-defined games (RON)

**Target design (planned):**
- A project file defines global configuration (initial scene, input mappings, UI roots, etc.). 🧭
- Scene files define entities to spawn, their capabilities, and their bindings to events/actions. 🧭
- Assets (models/textures/audio) are referenced by paths/handles from scene data. 🧭

**Implementation snapshot (today):**
- ✅ RON asset loading is wired up for `ProjectConfig` and `GameLevel` via `RonAssetPlugin`. 
- ✅ The project config currently includes at least `initial_scene` (see `ProjectConfig`). 

### 2) Capability blocks

**Target design (planned):**
- A capability is a reusable feature module with:
  - declared inputs/events it consumes 🧭
  - actions it emits/executes 🧭
  - validation rules for its configuration 🧭
- Capabilities are activated/configured by scene data.

**Implementation snapshot (today):**
- 🧪 Capability systems exist and are registered (player movement, orbit camera, animation playback), but the formal “capability registry + declarative bindings” is not fully implemented yet. 

### 3) Messages → Actions → Execution

**Target design (planned):**
- Standardized runtime messages/events (UI, input, scene lifecycle, triggers, animation markers, etc.). 🧭
- An interpreter maps messages to actions using data-defined rules. ✅
- An action executor applies actions in a controlled, testable way. ✅

**Implementation snapshot (today):**
- ✅ Standardized event/message types: `UiEvent`, `GameEvent`, `SceneEvent`, `InputActionMessage`.
- ✅ Project-level logic rules map events to actions via `logic/rules.ron` or `logic/state_machine.ron`.
- ✅ An action executor handles `LoadScene`, `Quit`, `Log`, `Spawn`, `PlayAnimation`, `PlaySound`, `SetVariable`, `IncrementVariable`, and more.
- ✅ UI buttons emit `UiEvent::ButtonPressed(String)`; gameplay sensors emit `GameEvent::Trigger(String)` — both flow into the rules pipeline.

## Repository layout

- `crates/ironhold_core` — core runtime plugin(s), schemas, runtime systems, and capability systems. ✅ 
- `crates/ironhold_native` — desktop runner calling `ironhold_core::start_app()`. ✅ 
- `crates/ironhold_web` — WASM runner exporting `start()` via `wasm-bindgen`. ✅ 
- `assets/` — example project + scenes + models. ✅ 
- `docs/` — design docs, data format spec drafts, roadmap, contributing guide. ✅ 

## Current implementation snapshot (today)

This section is intentionally brief and factual.

- ✅ The Bevy app is built by `start_app()` and adds `GamePlugin`. 
- ✅ Project configuration is loaded as an asset and transitions into a loading state. 
- ✅ UI buttons can trigger scene load requests. 
- ✅ Action queue infrastructure exists with a minimal `LoadScene` action. 
- ✅ Event catalog expansion started: `InputAction` and `SceneEvent` are implemented.

## Planned next steps (high level)

- 🧭 Expand and formalize the runtime **event schema** (input abstraction, scene lifecycle events, triggers/collisions, animation markers).
- 🧭 Move from ad-hoc wiring to **data-defined bindings** (strings → events/actions) with validation.
- ✅ Add **schema_version** to top-level data formats (project + scenes).
- 🧭 Add **schema_version migration notes**.
- 🧭 Introduce a **fixed-tick simulation loop** suitable for deterministic gameplay where needed.

## Getting started in 5 minutes

You never need to recompile the runtime to create or edit a project. The engine binary reads RON data files from `assets/projects/{name}/` at startup — editing RON files and refreshing the browser (web) or restarting the binary (native) is the entire iteration loop.

### 1 — Copy an existing project

```bash
cp -r assets/projects/quick_scene assets/projects/my_game
mv assets/projects/my_game/quick_scene.project.ron assets/projects/my_game/my_game.project.ron
```

### 2 — Update the project config

Edit `my_game/my_game.project.ron` — change `project_id` and `display_name`:

```ron
(
    schema_version: 2,
    project_id: "my_game",
    display_name: "My Game",
    initial_scene: "scenes/main.scene.ron",
    asset_catalog: "assets.ron",
    prefab_catalog: "prefabs/prefabs.ron",
    rules_path: "logic/rules.ron",
    model_fixes_path: "overrides/model_fixes.ron",
)
```

Use `schema_version: 3` with `state_machine_path` instead of `rules_path` when you need multiple scenes with a pause/menu flow. See `docs/20_data_formats.md` for the v3 example.

### 3 — Run it

**Native:**
```bash
cargo run -p ironhold_native -- --project my_game
```
**Web** (one-time build, then live editing):
```bash
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg
python serve.py
# Open play.html?project=my_game in the browser
```
After the initial build, RON edits are live — just press F5 to reload without rebuilding Rust.

### 4 — Edit the scene

`scenes/main.scene.ron` controls everything visible: lighting, entities, UI buttons, terrain. Change lighting colours, move entities, add UI buttons — all by editing the file and refreshing.

### 5 — Wire up logic

`logic/rules.ron` maps events to actions. Button presses, collisions, and scene lifecycle events all flow through here:

```ron
(
    schema_version: 2,
    rules: [
        ( on: "scene.ready:main",          do_actions: [ Log("Loaded!") ] ),
        ( on: "ui.button_pressed:start",   do_actions: [ LoadScene("scenes/game.scene.ron") ] ),
        ( on: "ui.button_pressed:quit",    do_actions: [ Quit ] ),
    ],
)
```

### Key reference files

| What you want to change | File to edit |
|---|---|
| Entities, lighting, UI in a scene | `scenes/{name}.scene.ron` |
| Character stats, model, colliders | `prefabs/prefabs.ron` |
| Which assets are available | `assets.ron` |
| What happens when events fire | `logic/rules.ron` or `logic/state_machine.ron` |
| Fix a model's pivot or rotation | `overrides/model_fixes.ron` |

Full field reference: `docs/20_data_formats.md`.

---

## Where to read next

- `docs/10_architecture.md` — current state + target architecture
- `docs/20_data_formats.md` — full field reference for all RON file types
- `docs/30_runtime_events_and_logic.md` — event/action model and FSM details
- `docs/50_roadmap_and_milestones.md` — milestones and feature gates

