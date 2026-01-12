# Task History - Ironhold-lib

## Phase A: Stability & Modularization (Jan 2026)

### Documentation
- [x] Add `docs/00_overview.md`
- [x] Add `docs/10_architecture.md`
- [x] Add `docs/20_data_formats.md`
- [x] Add `docs/30_runtime_events_and_logic.md`
- [x] Add `docs/40_determinism_and_networking.md`
- [x] Add `docs/50_roadmap_and_milestones.md`
- [x] Add `docs/60_contributing.md`
- [x] Update `README.md` links

### Refactor
- [x] Split `ironhold_core/src/lib.rs` into modules:
    - `schema/` (project/scene types)
    - `runtime/` (messages/actions/interpreter/scene manager)
    - `capabilities/` (player/camera/animation/ui)
- [x] Introduce an **Action model**:
    - `Action` enum + `ActionQueue` resource
    - Implement `Action::LoadScene(String)`
- [x] Convert UI button handling:
    - UI system emits `UiMessage`
    - Scene manager interprets to `Action::LoadScene`
    - Scene manager executes the action

### CLI and Configuration
- [x] Add support for specifying custom `project.ron` path via CLI arguments.
- [x] Update `ironhold_core::start_app` and `setup` to handle optional project file path.
- [x] Maintain compatibility for `ironhold_web` (passing `None` to `start_app`).

### Tests & Validation
- [x] Add integration test: simulate UI button press → verify `Action::LoadScene` → verify `AppState` transition.
- [x] Add RON validation tests: verify `ProjectConfig` and `GameLevel` deserialization handles missing fields and invalid inputs.
- [x] Debug Bevy 0.17 test environment:
    - Added missing plugins (`StatesPlugin`, `AssetPlugin`).
    - Initialized mandatory resources (`Assets<Mesh>`, `Assets<Gltf>`, etc.).
    - Handled input resources (`MouseMotion`, `ButtonInput<KeyCode>`, etc.).
    - Refined test lifecycle with multiple `app.update()` calls for state transitions.
- [x] Migrated all tests to Bevy 0.17 API (`Message` instead of `Event`, `write` instead of `send`, etc.).
