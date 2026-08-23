---
name: fixedupdate-vs-rapier-clock
description: Player logic runs FixedUpdate at 64 Hz but Rapier steps TimestepMode::Variable{max_dt:1/60} once per frame in PostUpdate — the two clocks diverge below 60 FPS, and any test that sets TimestepMode::Fixed hides it
metadata:
  type: project
---

Two independent clocks drive player physics, and tick counts are NOT physics time:

- `player_movement_system` (and all player/NPC movement) runs in **`FixedUpdate` at Bevy's default
  64 Hz** (`crates/ironhold_core/src/lib.rs`, the `add_systems(FixedUpdate, (...).chain())` block).
  Nothing in the crate overrides `Time<Fixed>` — verified by grep for `Time::<Fixed>` / `from_hz`.
- Rapier steps in **`PostUpdate`, once per rendered frame, `TimestepMode::Variable { max_dt: 1/60,
  substeps: 1 }`** — `capabilities/physics.rs` adds `RapierPhysicsPlugin::<NoUserData>::default()`
  with no overrides, and those are bevy_rapier3d-0.33's defaults (`plugin/configuration.rs:53`,
  `plugin/plugin.rs:204`).

So below 60 FPS, `FixedUpdate` ticks accumulate *faster* than physics time advances (at 30 FPS: ~2 ticks
per single 1/60 s step, i.e. physics runs at roughly half speed). Above 60 FPS the reverse. Any mechanism
that counts `FixedUpdate` ticks and expects a corresponding amount of *physical* motion (a windowed grace
counter, a "should have risen by now" assumption, a timeout in ticks) is framerate-coupled and wrong at
both ends.

**Why:** discovered 2026-08-20 reviewing the uphill-jump-lock fix, whose `CharacterController.jump_air_grace`
is a tick countdown; the `jump_liftoff_y` / `velocity.linvel.y <= 0.0` fallbacks exist precisely because the
tick count can't be trusted as elapsed physics time.

**How to apply:** prefer a *physical* predicate (velocity sign, distance risen, position delta) over a tick
count whenever correctness depends on the body having actually moved. When reviewing a physics test, check
whether it inserts `TimestepMode::Fixed { dt: .. }` — `player_slope_jump_tests.rs`'s `setup_case` does, which
makes it deterministic and readable but means it exercises a timestep mode production never uses, so the
clock-divergence failure mode is invisible to it. To exercise the real mode, leave `TimestepMode` at default
and drive the app with `TimeUpdateStrategy::ManualDuration` at a sub-60 FPS frame time.

Related: [[project_dt_scaled_discrete_impulse]], [[project_test_harness_message_buffers_never_rotate]].
