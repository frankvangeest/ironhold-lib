# ironhold_core — Integration Test Rules

Tests in `ironhold_core/tests/` must:
- Use `setup_test_app()` from `tests/support/mod.rs`, which installs `GamePlugin` (and transitively `PhysicsPlugin`). Do not construct an `App` without `GamePlugin` — missing physics resources cause panics.
- Initialize the `Message` framework (Writer/Reader resources) before running any messaging systems. `setup_test_app()` handles this.

See `tests/support/mod.rs` for the `setup_test_app()` helper. Each test file declares it via `mod support; use support::setup_test_app;`.

## Test file layout

| File | Domain | Tests |
|---|---|---|
| `integration_tests.rs` | Core scene/UI/input/FSM/spawner | 69 |
| `audio_tests.rs` | PlaySound, PlayMusicLoop, StopMusic, SetVolume | 16 |
| `stats_tests.rs` | StatMap, modifiers, resolve_stat | 19 |
| `particle_tests.rs` | SpawnEffect, layers, visual effects (particles + decals + fading lights) | 15 |
| `ron_validation.rs` | RON schema round-trips | 174 |
| `assets_schema_version_regression.rs` | Schema version regression guard | 1 |

## Important: support module placement

`support/mod.rs` lives in a subdirectory (not `support.rs` at the top level). Files directly under `tests/` are compiled as standalone test binaries by Rust; subdirectory files are not. Keeping `setup_test_app()` in `tests/support/mod.rs` prevents the compiler from trying to build it as an independent binary.
