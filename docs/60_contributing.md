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
  - unit test
  - integration test
  - data validation test

### Strongly recommended 🧪
- Add a “golden” RON file under `assets/` for new schema features.
- Add a regression test that loads it.

---

## Pull request checklist

- [ ] Documentation updated (use ✅/🧪/🧭 labeling)
- [ ] Example project updated or a new example added
- [ ] Tests added/updated (unit/integration as appropriate)
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

