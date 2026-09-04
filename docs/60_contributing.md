# Contributing

> **Doc type:** Contribution Guide (process + design intent)
>
> **Status legend:**
> - ✅ **Implemented** — enforced by code/tooling today
> - 🧪 **Prototype / Partial** — partly enforced; conventions exist
> - 🧭 **Planned** — target process; not fully implemented yet

## Status
🧪 Partially implemented

This guide describes **how we want to build Ironhold**. Some parts (notably the capability registry, event/action catalogs, and schema-version enforcement) are **planned** and may not be fully implemented yet.

---

## Ground rules

### 1) Prefer data-driven behavior (RON) 🧪
- If something can be configured as data, it should be.
- Hard-code only what must be hard-coded (platform integration, low-level engine wiring).

### 2) Keep responsibilities separated 🧪
- **Messages/events**: observations (“what happened”). 🧭
- **Actions**: explicit intent (“do this”). 🧭
- **Execution**: a controlled place where side effects happen. 🧭

> The full Messages → Actions → Execution model is the target design; parts exist today but are not complete.

### 3) Make changes testable ✅
- Add unit tests for pure logic.
- Add integration tests for data loading/validation and runtime flows.

---

## Adding or modifying a capability

A **capability** is a reusable feature module (player control, camera, UI flow, triggers, etc.).

### Target capability contract (planned) 🧭
New capabilities should register:
- **events they emit**
- **actions they execute**
- **validation rules** for their configuration

This contract enables:
- tooling that lists supported events/actions
- schema validation for scenes/projects
- clear documentation and stable behavior

### What to do today (current process) 🧪
Until the capability registry is fully implemented:
1. Add the capability module under `crates/ironhold_core/src/capabilities/`.
2. Wire the systems into the core plugin.
3. Add/extend schema types under `crates/ironhold_core/src/schema/` as needed.
4. Add tests:
   - RON parsing/validation tests for new schema
   - integration tests for the runtime behavior

### Documentation requirements ✅
For any capability change:
- Update the relevant design docs under `docs/`.
- If you introduce a new planned concept, label it 🧭.
- If you ship an implemented subset, document it as ✅/🧪.

---

## Data formats and schema changes

### Target requirements (planned) 🧭
- Every top-level data file includes `schema_version`.
- We keep **backward compatibility** where feasible.
- Breaking changes must include migration notes.

### What to do today (current process) 🧪
- If you add a field to a schema struct, add/adjust:
  - example RON in `assets/`
  - tests that load and validate those assets
  - documentation in `docs/20_data_formats.md`

---

## Testing expectations

### Required for PRs ✅
- `cargo test` passes.
- New behavior is covered by at least one of:
  - **Unit test** — `#[cfg(test)]` module in the same source file (e.g. `scene_loader.rs`). Best for pure functions and private helpers with no Bevy setup required.
  - **Integration test** — file in `crates/ironhold_core/tests/`. Best for Bevy systems, multi-module flows, or anything that needs a real `App` via `setup_test_app()`.
  - **Data validation test** — in `tests/ron_validation.rs`. Best for RON schema compliance and asset loading regression.

**Test placement at a glance:**

| What you're testing | Where it lives |
|---|---|
| Pure function, no Bevy | `#[cfg(test)]` block in the `.rs` file |
| Bevy system / ECS behavior | `tests/{domain}_tests.rs` — e.g. `fsm_tests.rs`, `spawn_tests.rs`, `ui_tests.rs` (see `crates/ironhold_core/tests/CLAUDE.md` for the full file layout) |
| RON file loads correctly | `tests/ron_validation.rs` |
| CLI output and exit codes | `crates/ironhold_cli/tests/` |

### CLI tests (`crates/ironhold_cli/tests/`) ✅

These tests build and invoke the `ironhold` binary directly using `env!("CARGO_BIN_EXE_ironhold")`. No GitHub Actions dependency — they run anywhere `cargo test` runs.

```bash
cargo test -p ironhold_cli                             # run all CLI tests
cargo test -p ironhold_cli --test validate_projects    # smoke: all example projects pass validate
cargo test -p ironhold_cli --test validate_cross_file  # cross-file reference errors are caught
```

**`validate_projects.rs`** — one test per example project under `assets/projects/`. Verifies `ironhold validate` exits `0` for every shipped project. Add a new test here whenever a new project is added.

**`validate_cross_file.rs`** — targeted tests for each cross-file reference check: missing effect key, missing audio key, missing prefab in scene, missing prefab in `Spawn` action, missing behavior file, and parse error. Each test asserts both the exit code (`1`) and that the offending key name appears in stdout.

Fixtures live in `crates/ironhold_cli/tests/fixtures/`. Each fixture contains only the minimum files to trigger its specific error. Do not pad them — lean fixtures stay readable and fail fast when the validate logic changes.

### Strongly recommended 🧪
- Add a “golden” RON file under `assets/` for new schema features.
- Add a regression test that loads it.

### Browser tests ✅
`test_web.py` runs a headless Chromium suite against the WASM build. Run it before submitting changes that touch rendering, scene loading, UI, or the action pipeline:

```bash
python test_web.py --skip-build   # fast: reuses existing pkg/
python test_web.py                # full: rebuilds WASM first
```

If a rendering change is intentional, regenerate baselines:
```bash
python test_web.py --update-baselines
```

If you add a new UI button that should be testable, note its canvas coordinates (derived from `position` + `size / 2` in the scene file) — Bevy UI renders inside the WebGPU canvas, not as DOM elements, so clicks must use `page.mouse.click(x, y)`.

---

## CLI tooling (`ironhold`) ✅

The `ironhold` CLI (`crates/ironhold_cli`) inspects asset files without starting the engine.
Build it once with `cargo build -p ironhold_cli` or run ad-hoc with `cargo run -p ironhold_cli -- <args>`.

### `inspect glb <path.glb>`

Lists everything you need to author RON for a model: animation clip names and durations,
mesh names with vertex/triangle counts, materials, and root scene nodes.

```bash
ironhold inspect glb assets/shared/models/creatures/orc-enemy.glb
ironhold --json inspect glb assets/shared/models/creatures/dragon.glb
```

Use this instead of `tools/glb_inspector/inspect_glb.py` for day-to-day authoring
(the Python tool is still needed for `--preview` renders that require Blender).

### `inspect texture <path>`

Reports image dimensions, format (PNG/JPEG/WebP/etc.), channel layout (RGB/RGBA/Grayscale),
and file size. Useful for catching oversized textures before they land in WASM builds.
Supports PNG, JPEG, WebP, GIF, BMP, TIFF. AVIF requires a native C decoder and is not supported.

```bash
ironhold inspect texture assets/shared/textures/decals/circle_filled.png
ironhold --json inspect texture assets/shared/textures/Cobblestone_001_SD/Cobblestone_001_COLOR.jpg
```

### `inspect audio <path>`

Reports audio format, duration, sample rate, channel count, and file size.
Duration is the key output — use it to set correct `delay_secs` values in `EmitEventAfterDelay`
after a sound plays. Supports WAV and MP3.

```bash
ironhold inspect audio assets/shared/audio/boulder/boulder-push1.wav
ironhold --json inspect audio assets/shared/audio/bg-music-balance.mp3
```

### `watch <project_dir>`

Re-runs `validate` automatically every time a `.ron` file in the project directory changes.
Press Ctrl+C to stop. Useful for an edit-validate loop without starting the engine.

```bash
ironhold watch assets/projects/quick_scene/
ironhold watch assets/projects/particles_demo/
```

Each save prints the changed file path and a compact result line:

```
Watching C:\git\rust\ironhold-lib\assets\projects\quick_scene\ — Ctrl+C to stop

[14:23:01] initial check  →  OK (6 files)

[14:24:15] scenes\main.scene.ron
           →  ERROR (1 issue)
             scenes/main.scene.ron: line 12, col 5: unknown field `directioonal`

[14:24:32] scenes\main.scene.ron
           →  OK (6 files)
```

The `--json` flag has no effect on `watch` — output is always human-readable.

### `stats <project_dir>`

Prints a compact summary of a project without starting the engine. Useful for quick AI context
before authoring new RON files, and for spotting projects that have grown unexpectedly large.

```bash
ironhold stats assets/projects/particles_demo/
ironhold stats assets/projects/3rd_person_game_demo/
ironhold --json stats assets/projects/quick_scene/
```

Example output:

```
particles_demo
  Scenes:    2
  Prefabs:   16
  Effects:   18
  Logic:     21 rules  5 behaviors
  Catalog:   120 entries  (models:0  textures:97  audio:0  effects:18  decals:5)
  Project:   11 RON files, 87.6 KB on disk
```

The `--json` flag emits a structured object with all the same fields plus `total_bytes`.

### `validate <project_dir>`

Parses every RON file in a project directory using the same schema types as the engine runtime,
then runs cross-file consistency checks. Use this before committing to catch typos and broken
references without starting the engine.

**Checks performed:**
- Per-file RON parse errors with line and column numbers
- Effect keys in `SpawnEffect` / decal keys in `ProjectDecal` exist in `assets.ron`
- Audio keys in `PlaySound` / `PlayMusicLoop` exist in `assets.ron`
- Prefab keys in scene entity defs and `Spawn` / `PreloadPrefab` actions exist in `prefabs.ron`
- Modifier keys in `ApplyModifier` / `RemoveModifier` exist in `stats.ron` (when present)
- Behavior file paths on `PrefabDef` exist on disk
- Scene paths in `LoadScene` / `LoadSceneOverlay` / `PreloadScene` / `ToggleOverlay` actions, and the project's own `initial_scene`, exist on disk (`missing_file`)
- A merchant prefab's `currency_stat` exists in `stats.ron`, and every `stock[].item_key` exists in `items.ron` (when the project sets `items_path`) — see "MerchantDef fields" in `docs/20_data_formats.md`
- Two players instantiated in the same scene — or reachable via that scene's `join_prefab_keys` hot-join slots — author the same `InputMap.gamepad_index` (`duplicate_gamepad_index`) — see "How a controller gets assigned to a player" in `docs/20_data_formats.md`. The `join_prefab_keys` half of this check has no scene-load `warn!` counterpart (the runtime warning only scans scene-instantiated players at load time, before any hot-join can happen) — this is the only design-time signal for that case.
- A scene's `label_depth_scale.min_scale` is outside `[0.0, 1.0]` (`label_depth_scale_min_scale_out_of_range`) — see "Label depth scaling" in `docs/20_data_formats.md`

```bash
ironhold validate assets/projects/particles_demo/
ironhold validate assets/projects/primitive_world/
ironhold --json validate assets/projects/quick_scene/
```

**`--strict` flag** adds reverse / orphan detection on top of the normal checks:
- Prefab keys in `prefabs.ron` never referenced in any scene entity or `Spawn` / `PreloadPrefab` action
- Effect keys in `assets.ron` never used in any `SpawnEffect` action
- Audio keys in `assets.ron` never used in any `PlaySound` or `PlayMusicLoop` action
- Decal keys in `assets.ron` never used in any `ProjectDecal` action
- A player prefab's `jump`/`double_jump_height` apex does not clear `collider_radius + ground_cast_length` (`jump_cannot_clear_ground_sensor`) — see the `MovementConfig` note in `docs/20_data_formats.md`
- A player prefab's `max_walkable_slope_deg` is outside the valid `(0, 90]` range (`invalid_walkable_slope_limit`) — see the `MovementConfig` note in `docs/20_data_formats.md`
- A player prefab's `coyote_time_secs` is negative (`negative_coyote_time_secs`) — silently disables the coyote-time buffer (same as `0.0`), most likely a typo — see the `MovementConfig` note in `docs/20_data_formats.md`
- A scene's `label_depth_scale.reference_distance` falls far outside its reachable player camera(s)' radius range (`label_depth_scale_reference_distance_outside_camera_range`) — depth scaling may never visibly engage; see "Label depth scaling" in `docs/20_data_formats.md`

```bash
ironhold validate --strict assets/projects/particles_demo/
ironhold --json validate --strict assets/projects/quick_scene/
```

Strict warnings appear in a separate `Strict checks` section and cause exit code `1`. Use in CI to enforce no dead data; omit for day-to-day editing where catalog entries accumulate ahead of usage.

**Exit codes:** `0` = all valid, `1` = validation errors or strict warnings found, `2` = tool / IO error.

### `query <subcommand> <project_dir>`

Lists data parsed from a project directory. Useful for AI agents and scripts that need to know what keys are defined before authoring new RON files.

```bash
ironhold query prefabs assets/projects/particles_demo/
ironhold query prefabs assets/projects/particles_demo/ --keys-only
ironhold query prefabs assets/projects/particles_demo/ --filter kind=actor
ironhold query prefabs assets/projects/particles_demo/ --filter tag=player
ironhold query prefabs assets/projects/particles_demo/ --filter behavior=true

ironhold query effects assets/projects/particles_demo/
ironhold query effects assets/projects/particles_demo/ --keys-only
ironhold query effects assets/projects/particles_demo/ --filter additive=true
ironhold query effects assets/projects/particles_demo/ --filter priority=Ambient
ironhold query effects assets/projects/particles_demo/ --filter layers=true

ironhold query scenes   assets/projects/3rd_person_game_demo/
ironhold query rules    assets/projects/3rd_person_game_demo/
ironhold query actions  assets/projects/3rd_person_game_demo/
ironhold query events   assets/projects/3rd_person_game_demo/

ironhold --json query prefabs  assets/projects/particles_demo/ --keys-only
ironhold --json query effects  assets/projects/particles_demo/
ironhold --json query actions  assets/projects/3rd_person_game_demo/
ironhold --json query events   assets/projects/particles_demo/
```

**`query prefabs`** — lists all entries from `prefabs/prefabs.ron`. Human output shows kind, model, tags, npc/trigger_zone/interactable flags, and behavior path. Supports `--filter kind=actor|prop|primitive`, `--filter tag=<value>`, `--filter behavior=true|false`, `--filter npc=true`. Use `--keys-only` to get one key per line for piping.

**`query effects`** — lists all entries from `assets.ron → effects`. Human output shows particle count or layer count, lifetime, additive flag, sprite flag, light flag, and non-default priority. Supports `--filter additive=true`, `--filter priority=Player|Npc|Ambient`, `--filter layers=true`, `--filter sprite=true`.

**`query scenes`** — lists all `*.scene.ron` files. Output shows name, entity count, UI element count, `player:true` (if any entity's prefab has the `player` tag), and `overlay` (scenes with only UI and no world entities or terrain).

**`query rules`** — shows `logic/rules.ron` (each rule's event trigger, optional `when:` guard, and action count) and/or `logic/state_machine.ron` (initial state, states with entry/exit/on counts and outgoing transitions).

**`query actions`** — lists every action type used across `rules.ron`, `state_machine.ron`, and all `*.behavior.ron` files. Shows variant name, total count, and which source files use it. Sorted by count descending. Useful for auditing what a project actually does at a glance.

**`query events`** — lists every event trigger string used across the same logic files. Shows the event name, how many bindings use it, which action types it directly fires, and `[transition]` when it also drives an FSM state change. Sorted alphabetically.

### `--json` flag

Any command accepts `--json` (before the subcommand name) for machine-readable output.

---

## Branching model ✅

Ironhold uses a three-tier branch model — `main` (deployable, serves GitHub Pages) → `integration` (batches finished features for combined testing + the release WASM build) → `feature/{slug}` (one per backlog item, its own git worktree). This lets several features be developed in parallel without any GitHub Actions or platform automation; enforcement is via local git hooks (`.githooks/`), which are plain git and carry over to Forgejo unchanged.

Full branch tiers, workflow-step mapping, and the one-time machine setup (`git config core.hooksPath .githooks`, shared `CARGO_TARGET_DIR`) live in root `CLAUDE.md` under **Branching Model** — that's the canonical reference; this section just flags that it exists.

A PR (via `gh pr create`) into `integration` is optional, not required, for a solo-dev flow — a plain `git merge` is fine. Use a PR when you want a review record before merging.

---

## Pull request checklist

Applies whether a feature lands via a PR or a direct merge into `integration`:

- [ ] Documentation updated (use ✅/🧪/🧭 labeling)
- [ ] Example project updated or a new example added
- [ ] Tests added/updated (unit/integration as appropriate)
- [ ] Browser tests pass (`python test_web.py --skip-build`); baselines updated if rendering changed
- [ ] Schema compatibility considered (version bump + migration notes if needed)
- [ ] No accidental platform-specific behavior in core logic
- [ ] `pkg/` is untouched on the feature branch (release builds only happen on `integration`)

---

## Style and code quality

### Rust style 🧪
- Prefer clear imports and avoid long single-line `use` lists.
- Keep modules small and focused.

### Observability ✅
- Prefer structured logging for important runtime transitions.
- Use `bevy::log::info!`, `warn!`, or `error!` instead of `println!` or `eprintln!`.
  - This ensures logs appear correctly on all platforms (including WebAssembly browser console).
- Avoid noisy logs in hot loops.

---

## Where to discuss design changes

If your change affects the runtime model (events/actions/determinism) or data formats:
- Update the relevant design docs first.
- Reference the roadmap milestone you’re targeting.

Recommended starting points:
- `docs/10_architecture.md`
- `docs/30_runtime_events_and_logic.md`
- `docs/40_determinism_and_networking.md`
- `docs/50_roadmap_and_milestones.md`

## Documentation requirements for Messages/Actions

If you add or change a **Message** or **Action** (engine ABI):

- Update `docs/STATUS.md` (Engine ABI section).
- Update `docs/30_runtime_events_and_logic.md` (lists + semantics).
- Update `docs/20_data_formats.md` with an authoring example if the change is user-facing.

This keeps the ABI and docs consistent and is required for Beta 0.2.

