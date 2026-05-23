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

Example projects: `quick_scene`, `3rd_person_game_demo`, `terrain_demo`, `custom_materials`, `primitive_world`, `entity_logic_demo`, `particles_demo`. Test data lives in `assets/projects/integration_tests/`.

## Tools

Python CLI tools live in `tools/`. Always run them from the repo root.

| Tool | When to use |
|---|---|
| `tools/asset_checker/check.py` | After editing any `assets.ron` or moving/renaming asset files — verifies all referenced paths resolve on disk |
| `tools/texture_gen/generate.py` | Generate seamless noise textures or per-project terrain heightmaps |
| `tools/avif2png/convert.py` | Batch-convert AVIF preview images to PNG |
| `tools/glb_inspector/inspect_glb.py` | Inspect a GLB for exact node names, animation clips, and materials before authoring RON |
| `tools/glb_preview/preview.py` | Render a 3/4-view preview PNG for GLB models using Blender headless |
| `tools/build_asset_manifest.py` | After adding, removing, or renaming any asset files — regenerates `assets_manifest.json` for the `assets.html` browser |

Each tool has its own `CLAUDE.md` with full usage examples. Run `python <tool> --help` for a quick reference.

```bash
# Always run after changing any assets.ron or moving asset files
python tools/asset_checker/check.py

# Also check for unreferenced files in assets/shared/
python tools/asset_checker/check.py --orphans

# Regenerate the asset browser manifest after adding/removing asset files
python tools/build_asset_manifest.py
```

## Planning

All work items live in `planning/`. See `planning/CLAUDE.md` for the full folder reference.

### Backlog (`planning/backlog.md`)
The canonical priority queue — features and bugs in one place. Items flow: **Icebox → Queued → Active → Done**. Do not duplicate items into GitHub issues or `docs/`.

### Bugs
Log known bugs in the `## Bugs` section of `planning/backlog.md` as a one-liner with reproduction and suspected cause. If the bug needs investigation before it can be fixed, also create `planning/investigations/{name}.md` and link to it from the backlog entry.

### Feature files (`planning/features/`)
Create `planning/features/{name}.md` (copy `_template.md`) when a feature needs design discussion before coding: new schema fields, new event/action types, cross-capability changes, or anything where the approach is unclear. Always fill in `Planned at: <hash> (<YYYY-MM-DD>)` at the top — run `git rev-parse --short HEAD` to get the hash.

### Claude suggestions (`planning/claude_suggestions.md`)
While implementing features, if you notice something worth revisiting — a latent bug, a pattern that could be improved, a follow-up optimisation — add a brief entry. Format:
```
- **Title** _(observed at `<hash>` <YYYY-MM-DD>)_
  What (one sentence) + Why (one sentence, concrete basis).
```
Only add things with a concrete technical basis. Frank reviews these periodically and promotes good ones to the backlog.

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

### Code change workflow
Every code change must follow this order before committing:

1. **Verify feature plan** — Check if the plan for the feature is:
  - planned out enough
  - project goal aligned
  - follows proper UX design 
2. **Code changes** — implement the feature or fix
3. **Tests pass** — `cargo test -p ironhold_core --test integration_tests --test ron_validation`
4. **Docs updated** — `docs/20_data_formats.md` and any relevant `CLAUDE.md` files
5. **WASM build** — `wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg`
6. **Provide a play-test checklist** — A checklist on how to check the changes and with what project.
7. **User play-tests** — Frank runs `python serve.py` and confirms the feature works in the browser
8. **Commit** — only after Frank confirms; include a summary in git commit message format

Do not commit before step 6. Do not skip the WASM build — new Rust code can compile natively but fail in WASM.

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
