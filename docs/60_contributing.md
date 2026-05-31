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
| Bevy system / ECS behavior | `tests/integration_tests.rs` |
| RON file loads correctly | `tests/ron_validation.rs` |

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

### `--json` flag

Any `inspect` subcommand accepts `--json` (before the subcommand name) for machine-readable output.
Exit codes: `0` = success, `2` = error (file not found, unsupported format, etc.).

---

## Pull request checklist

- [ ] Documentation updated (use ✅/🧪/🧭 labeling)
- [ ] Example project updated or a new example added
- [ ] Tests added/updated (unit/integration as appropriate)
- [ ] Browser tests pass (`python test_web.py --skip-build`); baselines updated if rendering changed
- [ ] Schema compatibility considered (version bump + migration notes if needed)
- [ ] No accidental platform-specific behavior in core logic

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

