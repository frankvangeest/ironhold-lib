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

## Tools

Python CLI tools live in `tools/`. Always run them from the repo root.

| Tool | When to use |
|---|---|
| `tools/asset_checker/check.py` | After editing any `assets.ron` or moving/renaming asset files — verifies all referenced paths resolve on disk |
| `tools/texture_gen/generate.py` | Generate seamless noise textures or per-project terrain heightmaps |
| `tools/avif2png/convert.py` | Batch-convert AVIF preview images to PNG |
| `tools/glb_inspector/inspect_glb.py` | Inspect a GLB for exact node names, animation clips, and materials before authoring RON |
| `tools/glb_preview/preview.py` | Render a 3/4-view preview PNG for GLB models using Blender headless |

Each tool has its own `CLAUDE.md` with full usage examples. Run `python <tool> --help` for a quick reference.

```bash
# Always run after changing any assets.ron or moving asset files
python tools/asset_checker/check.py

# Also check for unreferenced files in assets/shared/
python tools/asset_checker/check.py --orphans
```

## Planning

Feature planning lives in `planning/`. The canonical priority queue is `planning/backlog.md`. Design specs for non-trivial features live in `planning/features/`.

### Backlog workflow
- Items flow: **Icebox → Queued → Active → Done**
- Move an item to **Active** when work starts; to **Done** when merged.
- Keep `backlog.md` as the single source of priority — do not duplicate it into GitHub issues.

### When to create a feature file
Create `planning/features/{name}.md` (copy `_template.md`) when a feature needs design discussion before coding: new schema fields, new event/action types, cross-capability changes, or anything where the approach is unclear. Skip the file for simple, self-contained additions.

### Claude suggestions
While implementing features, if you notice something worth revisiting later — a pattern that could be improved, a latent bug, a follow-up optimisation — add a brief entry to `planning/claude_suggestions.md`. Only add things with a concrete technical basis observed during the current work, not general speculation. Each entry format:

```
- **Title** _(observed at `<hash>` <YYYY-MM-DD>)_
  What (one sentence) + Why (one sentence, concrete basis).
```

Run `git rev-parse --short HEAD` to get the hash. Frank reviews these periodically and promotes good ones to the backlog.

### Recording context in feature files
When writing a new feature file, always fill in the `Planned at` metadata at the top:
```
Planned at: <short commit hash> (<YYYY-MM-DD>)
```
Run `git rev-parse --short HEAD` to get the hash. This creates a stable reference — use `git log <hash>..HEAD` later to see what changed between design and implementation.

## Adding a new asset project

When a new project is added under `assets/projects/{name}/`, three registration steps are required:

1. **`test_web.py`** — append the project name to the `PROJECTS` list at the top of the file.

2. **Baseline screenshot** — generate the project's scene screenshot so it can be used in the gallery:
   ```bash
   python test_web.py --project {name} --update-baselines --skip-build
   ```
   This writes `screenshot_baselines/scenes/{name}_main.png` (and one file per scene if the project has multiple scenes).

3. **`index.html`** — add a card to the project grid. Copy an existing `<a class="project-card">` block and update:
   - `id` attribute (`card-{name}`)
   - `href` → `play.html?project={name}`
   - `data-keywords` → space-separated search terms
   - `img src` → `screenshot_baselines/scenes/{name}_main.png`
   - `img alt`, card title, description, and tags

---

## Critical Rules

### After changes
When ever you make changes in the code, give the summery of the changes in a nice git commit message format.

### Web Performance
When making new features, performance and compatibility with WASM web builds must be considered. Avoid using features not supported in web builds. Test web builds frequently (`python test_web.py`).

### Updating documentation
When asked to update or audit documentation, check **all** of the following — not just CLAUDE.md files:
- `CLAUDE.md` (root)
- `crates/ironhold_core/src/CLAUDE.md`
- `crates/ironhold_core/tests/CLAUDE.md`
- Every `.md` file in `docs/` (`00_overview.md`, `10_architecture.md`, `20_data_formats.md`, `25_custom_shaders.md`, `30_runtime_events_and_logic.md`, `40_determinism_and_networking.md`, `50_roadmap_and_milestones.md`, `60_contributing.md`, `70_profiling.md`, `browser_tests.md`, `STATUS.md`)

> Rust-specific rules (GPU/WGSL alignment, physics, terrain, inspector) live in
> `crates/ironhold_core/src/CLAUDE.md`.
> Integration test setup rules live in `crates/ironhold_core/tests/CLAUDE.md`.
> Browser test suite documentation lives in `docs/browser_tests.md`.
