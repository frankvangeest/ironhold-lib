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

**`validate_cross_file.rs`** — targeted tests for each cross-file reference check: missing effect key, missing audio key, missing prefab in scene, missing prefab in `Spawn` action, missing behavior file, missing dialogue file (both `PrefabDef.dialogue` and `Action::StartDialogue`), a dialogue `do_actions` reference error, a dialogue parse error, and parse error. Each test asserts both the exit code (`1`) and that the offending key name appears in stdout.

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

## CLI tooling (`ironhold`) — developer notes ✅

**How designers use the tool** (`validate`, `validate --strict`, `watch`, `stats`, `query`, `inspect`, exit codes,
`--json`, and the canonical table of every check with its code) is documented in
`docs/15_authoring_tools.md`. This section keeps only what contributors need.

- **Build / run:** `cargo build -p ironhold_cli` once, or `cargo run -p ironhold_cli -- <args>` ad hoc. The repo
  keeps a gitignored release-binary cache at `tools/bin/ironhold.exe` (see the root `CLAUDE.md` for when to rebuild
  it). Use `cargo run`, not the cached binary, whenever a schema type just changed.
- **`inspect glb`** replaces `tools/glb_inspector/inspect_glb.py` for day-to-day authoring; the Python tool is still
  needed for `--preview` renders that require Blender.

### Adding a validate check
1. Implement it in `crates/ironhold_cli/src/commands/validate.rs` (an always-on error goes in `cross_file_checks`
   with a stable `error_type` string; an advisory finding goes in `strict_checks` and makes `--strict` exit `1`). Walk
   `scene.ui` through `walk_ui_nodes`, never flat (root `CLAUDE.md` / the core `CLAUDE.md`).
2. Add a fixture under `crates/ironhold_cli/tests/fixtures/<name>/` and a test in
   `crates/ironhold_cli/tests/validate_cross_file.rs` (exit code and the message text).
3. **Add a row to the matching table in `docs/15_authoring_tools.md`** (what the designer sees, the code, what breaks
   if ignored, the fix). That page is the single canonical list of checks; do not re-list checks here.
4. Note the text output does not print the `error_type` code (only `--json` does), so the message wording is the
   designer's lookup key — write it so it is searchable.

### Check internals (implementation notes, keyed by code)
- **`missing_file` / `path_case_mismatch`**: `Path::exists()` is case-insensitive and `\`-tolerant on Windows/NTFS,
  so the check compares the authored path against the real on-disk entry (exact case, forward slashes). A mis-cased
  configured path is still parsed afterwards, so it does not hide the checks that depend on that file. Catalog/asset
  raw paths resolve against the asset root `assets/` — the nearest ancestor of `project_dir` literally named `assets`
  **and** containing a `projects/` or `shared/` child (the name alone is not enough); a GLB path's `#Scene0`-style
  fragment is stripped first. The check is skipped (never fabricated) when no corroborated `assets` ancestor exists —
  this crate's bare `tests/fixtures/{name}/` fixtures — so the fixtures for these checks live under
  `tests/fixtures/assets/projects/{name}/`.
- **Configured paths**: all five catalog paths (`asset_catalog`, `prefab_catalog`, `stats_path`, `items_path`,
  `model_fixes_path`) are read from their configured location, falling back to the convention path when unset (unlike
  the runtime, which then loads nothing); `state_machine_path` has **no** such fallback once a `.project.ron`
  exists.
- **Scene discovery**: scene paths from actions and `initial_scene` are parsed and folded into the same cross-checked
  scene set as `scenes/*.scene.ron`, so their contents are checked too. A scene reachable only through an
  `ActionSlotDef.do_actions` is not yet covered (see `planning/claude_suggestions.md`).
- **`unreachable_trigger` / `orphan_binding`**: skipped entirely when `state_machine.ron`/a behavior file failed to
  parse (fix that parse error first). Both account for the five engine-hardcoded panel triggers
  (`close_inventory`/`close_shop`/`close_container`/`take_all_from_container`/`buy_item:{item_key}`) whenever the
  matching panel is in a scene. `orphan_binding` only inspects `ui.button_pressed:*`-shaped events.
- **`missing_reference` on `Action::Spawn`'s `spawn_point`**: a `{self}`/`{target}`-templated value is skipped because
  it is resolved before the check would see it; a `requires_item` naming a missing key fails *closed* at runtime, so
  that one also has a scene-load `warn!` (the only diagnostic a WASM-only designer sees). The `join_prefab_keys` half
  of `duplicate_gamepad_index` has no runtime counterpart (the runtime warning scans scene-instantiated players at load
  time, before any hot join).
- **`missing_file` on `PrefabDef.animation_policy`**: `entity_spawner.rs` spawns the entity `Visibility::Hidden`
  pending the policy load, so a missing path means a permanently invisible character.
- **`duplicate_node_id`**: the runtime's `jump_to` resolution only ever reaches the first node with a given id.
- **Catalog `.validate()` checks** (`invalid_asset_catalog` / `invalid_prefab_catalog` / `invalid_stat_catalog` /
  `invalid_item_catalog`) and **`invalid_project_config`** were previously enforced only at runtime
  (`project_loader.rs`). `max_frame_delta_secs` is bounded by `MIN_MAX_FRAME_DELTA_SECS`/`MAX_MAX_FRAME_DELTA_SECS`
  in `schema/project.rs` (~0.03 to 60 s): too large or near zero panics at runtime, too small runs the game in
  permanent slow motion.
- **`invalid_scene`**: `GameSceneV2::validate()` is fail-fast (one error per scene); the runtime never calls it.
- **Group layout checks** (`ui_layout_diagnostics` in `schema/scene_v2.rs`): errors are `invalid_group_value` and
  `ui_depth_exceeded`; the rest are `--strict` warnings, and the engine logs all of them as `UI layout [kind]: ...` at
  scene load.
- **Strict-only movement checks** (`jump_cannot_clear_ground_sensor`, `invalid_walkable_slope_limit`,
  `negative_coyote_time_secs`, `coyote_time_exceeds_jump_airtime`) mirror scene-load `warn!`s; the `MovementConfig`
  note in `docs/20_data_formats.md` explains the numbers.

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
- `docs/dev/10_architecture.md`
- `docs/30_runtime_events_and_logic.md`
- `docs/dev/40_determinism_and_networking.md`
- `docs/dev/50_roadmap_and_milestones.md`

## Documentation requirements for Messages/Actions

If you add or change a **Message** or **Action** (engine ABI):

- Update `docs/STATUS.md` (Engine ABI section).
- Update `docs/30_runtime_events_and_logic.md` (lists + semantics).
- Update `docs/20_data_formats.md` with an authoring example if the change is user-facing.

This keeps the ABI and docs consistent and is required for Beta 0.2.

