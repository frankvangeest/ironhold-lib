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
- **`capabilities/`** — modular gameplay systems: player controller, orbit camera, animation, animation resolver, terrain mesh generation, terrain material, physics (Rapier3D).

### Data-driven game loop

The engine uses a **Message → Interpreter → Action → Executor** pipeline:

1. Capabilities emit `UiMessage`, `InputActionMessage`, or `SceneEvent` events.
2. `message_interpreter_system` reads those events plus the data-defined `LogicRules` (from `logic/rules.ron`) to produce `Action` values placed on the `ActionQueue` resource.
3. `action_executor_system` dispatches each `Action` (e.g., `LoadScene`, `Spawn`, `PlayAnimation`) to the appropriate capability systems.

This means game behavior can be authored entirely in RON without recompiling the engine.

### Asset & project layout

```
assets/projects/{name}/
  {name}.project.ron        ← ProjectConfig (entry point, initial scene ref)
  scenes/{name}.scene.ron   ← GameSceneV2 (models, UI, lighting, player)
  logic/rules.ron           ← event → action rules
  overrides/model_fixes.ron ← per-model transform corrections
  prefabs/prefabs.ron       ← reusable component definitions
  prefabs/animation/*.ron   ← AnimationPolicy per character
  assets.ron                ← AssetCatalog
```

Example projects: `quick_scene`, `3rd_person_game_demo`, `terrain_demo`. Test data lives in `assets/projects/integration_tests/`.

## Critical Rules (from AGENTS.md)

### WebGPU 16-byte alignment
Custom GPU-bound structs (e.g., `TerrainMaterial`) **must** use 16-byte aligned uniform buffer layouts. Violating this causes `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` panics in web builds. Verify `AsBindGroup` mappings distinguish Uniform vs. Storage buffers per Bevy 0.18 expectations.
- Use `Vec4` (16 bytes) for all uniform fields; never bind a bare `f32`, `Vec2`, or `Vec3`.
- `CustomMaterialUniforms` (4 × Vec4 = 64 bytes) and `TerrainMaterial.uv_scale` (Vec4 padded) already comply — keep them that way.

### WGSL is the first-class shader language
All shaders in this project are authored in WGSL. WGSL is the native language of WebGPU and runs identically on desktop (wgpu) and browser (WebGPU) — zero transpilation cost and consistent output on all platforms.

**When writing or reviewing shader code:**
- Shared (reusable) shaders → `assets/shared/shaders/`, named `custom_*.wgsl`.
- Project-specific shaders → `assets/projects/{name}/shaders/`.
- All custom fragment shaders must declare the full `CustomMaterial` binding contract (see `docs/25_custom_shaders.md`). Missing bindings cause WebGPU validation errors, not panics.
- `TonyMcMapface` and `BlenderFilmic` tonemapping are excluded because they require a LUT texture. Do not add LUT-dependent shaders.
- `CustomMaterial` currently overrides the **fragment shader only**. Vertex shader override is planned but not yet implemented — do not attempt to swap the vertex shader via `specialize()`.
- Always test WGSL changes in a web build (`python test_web.py`). WebGPU validates binding interfaces strictly; native wgpu is more permissive and will not catch all errors.

See `docs/25_custom_shaders.md` for the full shader authoring guide.

### Physics & movement must use `FixedUpdate`
All player movement, physics processing, and camera-follow logic must run in `FixedUpdate`. Using `Update` for physics-driven movement causes stuttering.

### Web Performance
When making new features performance and compatibility with WASM web builds must be considered. Avoid using features that are not supported in web builds. Test web builds frequently. See `test_web.py` for more information.

### Terrain generation is async
Terrain mesh generation is compute-heavy. Always use Bevy's `AsyncComputeTaskPool` and poll `Task` components — never block the main thread.

### Inspector isolation
`bevy_egui` inspector and game UI are strictly separated. The inspector renders on its own camera/layer; never mix it with the main game UI camera.
Data structs of objects in the scene should be visible in the inspector need to be conditionally expose to the inspector using the `#[inspectable]` attribute.
E.g.:
``` rust
#[derive(Component, Reflect)]
#[cfg_attr(feature = "inspector", derive(bevy_inspector_egui::prelude::ReflectInspectorOptions))]
#[cfg_attr(feature = "inspector", inspector_options::InspectorOptions)]
pub struct MyObject {
    pub name: String,
    pub health: u32,
}
```

### Integration test setup
Tests in `ironhold_core/tests/` must:
- Include `PhysicsPlugin` (missing it causes panics from unregistered physics resources).
- Initialize the `Message` framework (Writer/Reader resources) before running any messaging systems.

See `tests/support.rs` for the `setup_test_app()` helper.

## Browser Test Suite (`test_web.py`)

Runs 9 headless Chromium tests against the built WASM package. Requires `playwright install chromium` (one-time).

**Five test categories:**

| Category | Tests | What it checks |
|----------|-------|----------------|
| `smoke` | one per project | Page loads, `<canvas>` appears, `app_state` reaches `InGame`, no JS/Rust errors |
| `action` | `dance_button` | Clicking the Dance button (canvas coords) fires `PlayAnimation` via the rules pipeline |
| `transition` | `start_game` | Clicking Start Game transitions `start_menu.scene.ron` → `main.scene.ron` |
| `baseline` | one per project | Screenshot diff vs stored baseline stays under 2% changed pixels |
| `navigation` | `pause_menu_flow` | Full menu flow: start menu → main → Esc (pause) → Esc (close) → Esc (pause) → Resume; screenshot at each step |

**`DebugState` resource** — the test harness reads a hidden `<div id="debug-state">` that the WASM runtime updates every frame with JSON:
```json
{"frame": 42, "app_state": "InGame", "last_action": "PlayAnimation(\"dance\")", "scene": "projects/quick_scene/scenes/main.scene.ron"}
```
This is written by `sync_debug_state_to_dom` (WASM-only, `PostUpdate`) in `ironhold_core/src/lib.rs`.

**URL project selection** — the WASM build reads `?project=<name>` from the URL (e.g. `?project=terrain_demo`) and passes it to `start_app`. Implemented in `ironhold_web/src/lib.rs`.

**Canvas coordinate clicks** — Bevy UI renders inside the WebGPU canvas, not as DOM elements. Button clicks in tests must use `page.mouse.click(x, y)` with coordinates derived from the scene's `position` + `size/2` fields.

**Baseline screenshots** live in `screenshots/baselines/` (project baselines) and `screenshots/pause_nav/baselines/` (navigation step baselines), both gitignored. Run `--update-baselines` after any intentional rendering change.

## Technology Notes

- **Bevy 0.18** — always use 0.18 API; `AsBindGroup` behaviors and resource initialization changed significantly in this version.
- **RON** — all game data files use Rusty Object Notation (`.ron`). Schema versioning is enforced via `schema_version` fields; see `docs/20_data_formats.md`.
- **WGSL** — all shaders are WGSL (WebGPU Shading Language). Fragment shaders are loaded from `assets/shared/shaders/` and wired up via the `CustomMaterial` system in `assets.ron`. See `docs/25_custom_shaders.md`.
- **Bevy app states**: `LoadingProject → LoadingScene → InGame` (with optional Paused/Error).
