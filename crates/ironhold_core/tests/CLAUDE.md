# ironhold_core — Integration Test Rules

Tests in `ironhold_core/tests/` must:
- Use `setup_test_app()` from `tests/support/mod.rs`, which installs `GamePlugin` (and transitively `PhysicsPlugin`). Do not construct an `App` without `GamePlugin` — missing physics resources cause panics.
- Initialize the `Message` framework (Writer/Reader resources) before running any messaging systems. `setup_test_app()` handles this.

See `tests/support/mod.rs` for the `setup_test_app()` helper. Each test file declares it via `mod support; use support::setup_test_app;`.

## Test file layout

| File | Domain | Tests |
|---|---|---|
| `fsm_tests.rs` | Global FSM: state transitions, rules matching, `ActionQueue` FIFO ordering | 23 |
| `entity_logic_tests.rs` | Per-entity FSM (`.behavior.ron`), `{self}` substitution, intent event layer | 8 |
| `scene_lifecycle_tests.rs` | Scene load/unload, overlays, model fixes, pipeline warmup, key bindings, animation graph | 18 |
| `spawn_tests.rs` | `Action::Spawn`/`Despawn`, spawn queue rate limiting, preload, composite prefab spawn | 15 |
| `action_tests.rs` | Misc `Action` executor behaviors: variables, delayed events, floating text, target indicator | 13 |
| `npc_tests.rs` | NPC aggro/investigating states, camera shake | 8 |
| `nameplate_tests.rs` | Nameplate anchor spawn, visibility filtering, cleanup | 10 |
| `ui_tests.rs` | Button click-to-action wiring, `IconButton` icon/color/shadow sync | 9 |
| `audio_tests.rs` | PlaySound, PlayMusicLoop, StopMusic, SetVolume | 16 |
| `stats_tests.rs` | StatMap, modifiers, resolve_stat | 22 |
| `particle_tests.rs` | SpawnEffect, layers, visual effects (particles + decals + fading lights), quality tiers, budget gating | 27 |
| `ron_validation.rs` | RON schema round-trips | 188 |
| `assets_schema_version_regression.rs` | Schema version regression guard | 1 |
| `ron_lint.rs` | RON style invariants (e.g. no explicit `Some(...)` wrappers) | 1 |
| `ui_panel_blocker.rs` | Headless Bevy UI focus pipeline: panel + overlay backdrop click-blocking (`FocusPolicy::Block`) | 4 |

`fsm_tests.rs` through `ui_tests.rs` were split out of a single `integration_tests.rs` (2026-07-02) once it grew to 104 tests / 4258 lines mixing 8 distinct subsystems with no internal organization. See `planning/features/done/split_integration_tests.md`.

## Important: support module placement

`support/mod.rs` lives in a subdirectory (not `support.rs` at the top level). Files directly under `tests/` are compiled as standalone test binaries by Rust; subdirectory files are not. Keeping `setup_test_app()` in `tests/support/mod.rs` prevents the compiler from trying to build it as an independent binary.
