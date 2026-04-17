# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```bash
# Run native (desktop) build
cargo run -p ironhold_native

# Run with inspector UI (debug overlay)
cargo run -p ironhold_native --all-features

# Run a specific project by name
cargo run -p ironhold_native -- --project 3rd_person_game_demo

# Run all tests
cargo test -p ironhold_core --test '*' -- --nocapture

# Run a single test file
cargo test -p ironhold_core --test integration_tests
cargo test -p ironhold_core --test ron_validation

# Run a single test by name
cargo test -p ironhold_core --test integration_tests test_ui_button_to_load_scene_action

# Build for WASM (requires wasm-pack)
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg

# Serve WASM locally (no-cache, port 8000)
python serve.py

# Full browser test suite (builds WASM, starts server, runs headless Chromium)
python test_web.py

# Skip wasm-pack build and test against existing pkg/
python test_web.py --skip-build

# Overwrite stored screenshot baselines after intentional visual changes
python test_web.py --update-baselines

# Overwrite baseline for a single project (or 'pause_nav' for navigation steps)
python test_web.py --update-baseline quick_scene
python test_web.py --update-baseline pause_nav
```

## Architecture Overview

Three-crate workspace:
- **`ironhold_core`** — platform-agnostic game library; contains all logic, rendering, physics, and the scene pipeline. Must never have platform-specific code.
- **`ironhold_native`** — thin desktop runner; parses `--project` CLI arg, calls `ironhold_core::start_app()`.
- **`ironhold_web`** — thin WASM runner; `#[wasm_bindgen(start)]` calls `ironhold_core::start_app(None)`.

### Core internal structure (`ironhold_core/src/`)

- **`schema/`** — RON-serializable data types (`ProjectConfig`, `GameSceneV2`, `AssetCatalog`, `PrefabCatalog`, `Action`, etc.). These are the source of truth for all data-driven content.
- **`runtime/`** — systems that run at engine boot/update: scene loading (`scene_manager`), model spawning, material creation, input translation, message interpreter, action executor.
- **`capabilities/`** — modular gameplay systems: player controller, orbit camera, flycam, animation, animation resolver, NPC AI, collectible triggers, motion (rotate/bob), custom material, terrain mesh generation, terrain material, physics (Rapier3D).

### Data-driven game loop

The engine uses a **Message → Interpreter → Action → Executor** pipeline:

1. Capabilities emit `UiEvent`, `GameEvent`, `InputActionMessage`, or `SceneEvent` events.
2. `message_interpreter_system` reads those events plus the data-defined `LogicRules` (from `logic/rules.ron`) to produce `Action` values placed on the `ActionQueue` resource.
3. `action_executor_system` dispatches each `Action` (e.g., `LoadScene`, `Spawn`, `PlayAnimation`) to the appropriate capability systems.

This means game behavior can be authored entirely in RON without recompiling the engine.

### Asset & project layout

```
assets/projects/{name}/
  {name}.project.ron          ← ProjectConfig (entry point, initial scene ref)
  scenes/*.scene.ron          ← GameSceneV2 files (models, UI, lighting, player); projects can have multiple scenes
  logic/rules.ron             ← event → action rules (simple projects)
  logic/state_machine.ron     ← FSM-based logic (used by projects with multiple states/scenes)
  overrides/model_fixes.ron   ← per-model transform corrections
  prefabs/prefabs.ron         ← reusable component definitions
  prefabs/animation/*.ron     ← AnimationPolicy per character
  assets.ron                  ← AssetCatalog
```

Note: projects may have `rules.ron`, `state_machine.ron`, or both. Simple projects use only `rules.ron`; projects with multiple scenes/states use `state_machine.ron` (sometimes alongside `rules.ron`). See the interpreter notes in `crates/ironhold_core/src/CLAUDE.md`.

Example projects: `quick_scene`, `3rd_person_game_demo`, `terrain_demo`, `custom_materials`, `primitive_world`. Test data lives in `assets/projects/integration_tests/`.

## Critical Rules

### After changes
When ever you make changes in the code, give the summery of the changes in a nice git commit message format.

### Web Performance
When making new features, performance and compatibility with WASM web builds must be considered. Avoid using features not supported in web builds. Test web builds frequently (`python test_web.py`).

> Rust-specific rules (GPU/WGSL alignment, physics, terrain, inspector) live in
> `crates/ironhold_core/src/CLAUDE.md`.
> Integration test setup rules live in `crates/ironhold_core/tests/CLAUDE.md`.
> Browser test suite documentation lives in `docs/browser_tests.md`.
