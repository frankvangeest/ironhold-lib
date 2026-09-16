---
name: project-max-fixed-delta-config
description: ProjectConfig.max_fixed_delta_secs — opt-in RON cap on Time<Virtual>::max_delta, applied once at project load; why it is load-time-only, and the two foot-guns (it clamps Update delta too, and a sub-tick value causes permanent slow-motion)
metadata:
  type: project
---

Branch `feature/fixed_timestep_max_delta` (reviewed 2026-09-16, follow-up to
[[project-deterministic-fixed-timestep]]; measurement in
`planning/investigations/fixed_timestep_max_delta.md`). Verify against current
`runtime/scene_manager/project_loader.rs` before relying on this.

**Load-time-only, confirmed.** `apply_max_fixed_delta()` is a free function called from the shared
tail of `check_project_loaded`, which is registered
`add_systems(Update, check_project_loaded.run_if(in_state(AppState::LoadingProject)))`
(`lib.rs:218`). `LoadingProject` is entered exactly once, from the Startup-side setup system
(`lib.rs:498`), and the tail sets `AppState::LoadingScene`, so the system stops running for the
rest of the session. Zero steady-state per-frame cost. Don't re-flag it as a hot-path system.

**`ResMut<Time<Virtual>>` on a load-only system is free on WASM.** Its only cost is reduced
native system parallelism during `LoadingProject`; single-threaded WASM has no parallelism to
lose. Same class as the `Option<&Transform>` note in [[project-fresh-global-transform]].

**Foot-gun 1 — `max_delta` is not FixedUpdate-only.** Bevy clamps `Time<Virtual>::delta` with it
*before* the `Time<Fixed>` accumulator is fed and before `Time` (default generic) is set for
`Update`. So a low value also slows every `Update`-driven system (camera smoothing, animation,
motion) during any long frame, not just physics catch-up. The field's doc comment and
`docs/20_data_formats.md` describe it purely as a FixedUpdate catch-up cap.

**Foot-gun 2 — values below the tick period cause permanent slow-motion.** `FIXED_TICK_RATE` is
64 Hz (`capabilities/physics.rs:12`), so one tick needs 15.625 ms of virtual time. Any
`max_fixed_delta_secs` at or below ~1/64 makes the sim fall behind even on a perfectly healthy
frame. `ProjectConfig::validate()` only rejects non-positive/non-finite, so 0.01 passes both the
CLI and the runtime guard. A sane floor is ~2 tick periods (0.03125).

**No project opts in** (grep of `assets/` finds zero uses as of this review), so all of the
investigation's measured numbers still describe every shipped project unchanged — and no browser
test exercises the non-default path.

Cost model if a project does opt in: lower value = strictly cheaper worst-case frame (fewer
catch-up ticks), earlier visible slow-motion; higher value = the reverse. Consistent with the
investigation's own tradeoff framing; nothing about the measurement changes.
