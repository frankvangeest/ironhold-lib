# Feature: Deterministic Fixed Timestep + Cross-Platform Divergence Harness

_Status: In Progress (v1 Active, v2 Queued)_
_Planned at: `7332d62` (2026-09-15)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Fixed-timestep Rapier physics | Active | — |
| v2 | Cross-platform determinism harness | Queued | — |

## What

**v1** moves Rapier physics from its current variable, wall-clock-driven timestep onto the
engine's existing `FixedUpdate` tick (which already runs at 64Hz, see Approach), so every device —
60Hz, 144Hz, a throttled browser tab — steps the solver with the same `dt`.

**v2** builds a scripted-input, per-tick state-hash harness that runs the same input stream
through native and WASM (Chrome + Firefox) builds and reports the first tick, if any, where
simulated state diverges. This turns "is cross-platform determinism achievable here" from an
assumption into a measured fact before any networking model is chosen.

## Why

This traces back to `planning/stakeholder_priority_list.md`'s system-architect item #1
("Rapier cross-platform float divergence blocks the entire multiplayer roadmap"). A
system-architect investigation (2026-09-15, see report cited in this session) found that premise
was **already wrong**: `bevy_rapier3d` has had Rapier's `enhanced-determinism` feature enabled
since it was added (`crates/ironhold_core/Cargo.toml:16` — `features = ["enhanced-determinism",
"serde-serialize"]`), which per Rapier's own documentation unifies transcendental math via `libm`
and disables the native-only MXCSR denormal-flush-to-zero optimization, giving bit-level
cross-platform determinism **for rapier/parry/nalgebra's own math**, on IEEE-754-2008-compliant
platforms, using the same rapier version and feature flags on every peer — conditions this engine
already meets, but which have never been checked end-to-end against our actual usage.

A second, independent system-architect review (same day, plan-review of this file's first draft)
re-verified every claim below directly against vendored source and current `ironhold_core` code —
not taken on the first investigation's word — and found three corrections, folded in below.

The investigation found the **actual** remaining blockers are more mundane:

1. **Variable timestep is the real hard blocker.** `capabilities/physics.rs:12` runs
   `RapierPhysicsPlugin::<NoUserData>::default()` in `PostUpdate` with the default
   `TimestepMode::Variable { max_dt: 1.0/60.0, .. }` — the solver's `dt` is real wall-clock frame
   delta, clamped. Two machines at different frame rates feed the solver different inputs
   regardless of how deterministic the math is. This also already causes real gameplay jank today,
   independent of multiplayer — it's why `capabilities/player.rs`'s coyote-time/jump-grace logic
   (`player.rs:199-250, 521-560`) is built around reconciling two independent clocks.
2. **Three call sites in our own gameplay code bypass Rapier's libm-forcing** (corrected — the
   first draft of this investigation found two; plan-review found a third, worse-shaped one):
   - `player.rs:289` — `acos` for slope-angle, feeding a grounded/airborne branch: worst possible
     shape for a 1-ULP difference since it can flip a boolean.
   - `player.rs:579` — `sin_cos` via `Transform::rotate_y` (an accumulating quaternion).
   - `motion.rs:38-55` — `motion_system`'s `Quat::from_rotation_{x,y,z}` and `.sin()` bob. This one
     is structurally worse than the other two: it's registered in **`Update`**, not `FixedUpdate`
     (`lib.rs:332` era of the schedule — confirm exact line at implementation time), reads
     wall-clock `Time::elapsed_secs()`/`delta_secs()` directly, and writes `Transform` on entities
     that can carry colliders/sensors — so it pushes a transcendental-derived, wall-clock-driven
     transform into the physics world every rendered frame, which fixing the physics timestep alone
     does **not** address. Motion-carrying prefabs with colliders need this system's *scheduling*
     revisited, not just its math.

   (Everything else was checked and is fine: `npc.rs`'s `distance`/`normalize` calls are sqrt-only,
   which is IEEE-754 correctly-rounded and therefore already deterministic; one-time spawn-time
   `to_radians()`/`Quat::from_euler` calls in `scene_loader.rs`/`entity_spawner.rs`/
   `action_executor.rs` run once from author-stable RON inputs and would divergence-check
   identically regardless of platform.)
3. **Nobody has actually measured it.** `enhanced-determinism`'s cross-platform claim is Rapier's
   own documentation, not something verified against this engine's actual usage (query-heavy,
   rotation-locked capsules, gameplay-overwritten velocity — a narrower and better-behaved surface
   than general rigid-body simulation).

Fixing the timestep is worth doing on its own merits even if multiplayer never ships — it
simplifies the existing dual-clock jump/coyote-time logic. The harness is what actually tells us,
with evidence, whether `planning/features/networking_multiplayer.md`'s Form 1 lockstep commitment
is viable, before more of the roadmap is built on top of an unverified assumption.

## Approach

**v1 — fixed timestep:**
- `capabilities/physics.rs`: switch to `RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule()`
  (`bevy_rapier3d-0.33.0/src/plugin/plugin.rs:111-113`) with an explicit
  `TimestepMode::Fixed { dt, substeps }` (`plugin/configuration.rs:14-23`). `TimestepMode` is a
  `Resource`, not a field of the `RapierConfiguration` component — `insert_resource(TimestepMode::
  Fixed { .. })` must be called **before** `add_plugins(RapierPhysicsPlugin::...)`, or the plugin's
  own `init_resource` inside `build()` wins and you get a spurious startup `warn!`
  (`plugin.rs:331-337`).
- **Tick rate is a decision, not an open question: `dt = 1.0 / 64.0`, matching
  `FIXED_TICK_RATE` (`crates/ironhold_core/src/capabilities/player.rs:13`, already `64.0`) —
  not 60Hz.** `FixedUpdate` already runs at 64Hz (Bevy's `Time<Fixed>` default) and every
  tick-derived gameplay constant (`jump_air_grace_ticks()`, `coyote_ticks()`) is baked against that
  rate. `max_dt` in the *current* variable-timestep config is a clamp on a wall-clock step, not the
  schedule rate — it is unrelated to what `FixedUpdate`'s own tick rate is, and reusing its number
  here would run physics 64/60 ≈ 1.067× faster than every existing tick-derived constant assumes.
  Derive both `FIXED_TICK_RATE` and `TimestepMode::Fixed.dt` from one shared constant so they can
  never drift apart independently.
- **New required task, not previously scoped: order physics explicitly against the existing
  `FixedUpdate` chain.** `lib.rs:281-289` already chains `gamepad_bind_system →
  input_translator_system → player_movement_system → player_view_box_clamp_system →
  collectible_system → trigger_zone_system → npc_behavior_system` in `FixedUpdate`. Today that
  chain is implicitly ordered relative to physics only because physics lives in `PostUpdate`. Once
  Rapier's `PhysicsSet::{SyncBackend, StepSimulation, Writeback}` run in the same schedule, Bevy
  does **not** order the two chains against each other by default — this is exactly the class of
  schedule race that has previously caused real, hard-to-reproduce test flakiness in this codebase
  (see the ambiguity-detection backlog item and `crates/ironhold_core/src/CLAUDE.md`'s targeting-race
  notes). Required: order the existing gameplay chain `.before(PhysicsSet::SyncBackend)` (movement
  writes velocity → physics steps → writeback), and any system reading post-step physics state
  `.after(PhysicsSet::Writeback)`. This must be designed and reviewed as part of v1, not discovered
  during playtest.
- Resolve rendering smoothness explicitly as part of this phase — either `TimestepMode::Interpolated`
  + `TransformInterpolation`, or accept the aliasing — do not leave it for a later surprise.
- Route `player.rs:289`'s `acos`, `player.rs:579`'s rotation, and `motion.rs`'s
  `Quat::from_rotation_*`/`.sin()` bob through `libm` directly (small internal helper, e.g.
  `det_math::acos`/`det_math::sin_cos`) so the known std-transcendental call sites in gameplay code
  match Rapier's own determinism guarantee. For `motion_system` specifically, also decide whether
  it moves out of `Update` into `FixedUpdate` — the scheduling change is the larger of its two
  issues, and a real behavior change for bob/spin feel that needs its own sign-off, not just a
  math swap.
- Re-tune `jump_air_grace_ticks()` and related coyote-time constants in `player.rs` against the new
  single clock — expect values to simplify, not just shift.
- **The existing physics-adjacent test suite cannot regression-catch this change and must not be
  read as if it can.** `player_slope_jump_tests.rs:101`, `prop_ground_veto_tests.rs:171`, and
  `wall_friction_tests.rs:124` already construct `TimestepMode::Fixed { dt: 1.0/64.0, substeps: 1 }`
  directly in their test harness — the whole jump/slope/coyote/wall-friction suite has been
  validating a timestep mode **the shipped game doesn't actually use today**, and will stay green
  whether v1 lands or not. Two consequences: (a) the acceptance criterion below about no playtest
  regression is the *only* real signal for this change — treat it as load-bearing, not a formality;
  (b) `player_slope_jump_tests.rs`'s `grace_expiry_does_not_reset_early_when_real_physics_time_lags_
  ticks` test exists specifically to decouple the fixed-tick clock from a lagging physics clock
  (`physics_dt = 1/256`) — after v1 those two clocks are one and the same, so the test's *premise*
  is gone even though its underlying assertions (the `jump_liftoff_y` / velocity belt-and-braces
  check) should stay. Task: rewrite that test's doc comment to reflect the new premise; do not
  delete the test.
- **This phase touches the most playtest-sensitive code in the engine** (ground-cast/jump/slope
  logic, per `crates/ironhold_core/src/CLAUDE.md`'s own notes on how fragile it's been). Full
  playtest matrix required, not just the automated suite.

**v2 — divergence harness:**
- **Run-mode plumbing: add a `RunMode` enum / `StartOptions` struct in `ironhold_core`, not a
  third positional parameter on `start_app`.** `ironhold_core::start_app(project_path,
  scene_override)` (`lib.rs:755`) currently takes exactly two positional `Option<String>`s, called
  directly by both `ironhold_web` and `ironhold_native`. Bolting on `?determinism_probe=1` (or a
  native flag) as a third positional argument means editing both thin runners for every future run
  mode — and there is already a second queued consumer of the same seam
  (`planning/backlog.md`'s `static_scene_mode.md`, `?static=1`). Instead: `StartOptions { project,
  scene, run_mode: RunMode }` (default `RunMode::Normal`) in `ironhold_core`, with each runner
  parsing its own platform's flag/query-param into it. Keeps platform-specific parsing in the thin
  runners (the correct boundary) and makes the next run mode an enum variant, not another
  three-crate signature break.
- Replay a scripted `InputActionMessage` stream for a fixed tick count, computing a per-tick state
  hash (positions/velocities quantized to avoid trivial float-formatting noise; NaN bit patterns
  canonicalized before hashing, since WASM permits nondeterministic NaN payloads per spec).
- Print/expose the hash stream so it can be scraped — reuse `test_web.py`'s existing headless
  Chromium harness and DOM-scraping pattern; extend the comparison to Firefox and to native
  dev vs. release builds (`opt-level = "s"` vs `"3"` should not affect results, but this is exactly
  the kind of assumption this harness exists to check rather than take on faith).
- Compare native × WASM/Chrome × WASM/Firefox × dev × release. First divergent tick + which
  quantity diverged is the report; a clean run across the full matrix is the report too.
- This is Milestone B ("Replay tooling" / tick-level state hashing) from
  `docs/40_determinism_and_networking.md:184-186` (not lines 150-157, which is the Rollback netcode
  section — corrected citation), pulled forward and scoped concretely.
- **Build this as a permanent, repeatable tool, not a one-off investigation script.** It is the
  only mechanism that would ever catch a future regression in the `FixedUpdate`/`PhysicsSet`
  ordering above, or a newly-added transcendental call site — and
  `planning/stakeholder_priority_list.md` item #4 already names the absence of schedule-graph/
  ambiguity assertions as a standing gap in this codebase. This harness is the closest thing the
  repo would have to one.

## Tasks

- [ ] `capabilities/physics.rs`: `TimestepMode::Fixed { dt: 1.0/64.0, .. }` (insert the resource
      before `add_plugins`) + `in_fixed_schedule()`
- [ ] Order the existing `FixedUpdate` gameplay chain (`lib.rs:281-289`) `.before(PhysicsSet::
      SyncBackend)`, and any post-step reader `.after(PhysicsSet::Writeback)`
- [ ] Derive `FIXED_TICK_RATE` and `TimestepMode::Fixed.dt` from one shared constant
- [ ] Decide and implement render-side smoothing (interpolation vs. accept aliasing)
- [ ] `det_math` helper (or equivalent) routing `player.rs:289`, `player.rs:579`, and
      `motion.rs`'s rotation/bob math through `libm`
- [ ] Decide whether `motion_system` moves from `Update` into `FixedUpdate`
- [ ] Re-tune jump/coyote-time constants against the fixed clock
- [ ] Rewrite `player_slope_jump_tests.rs`'s `grace_expiry_does_not_reset_early_when_real_physics_
      time_lags_ticks` doc comment — its two-clocks premise is gone post-v1; keep its underlying
      assertions
- [ ] `RunMode`/`StartOptions` seam in `ironhold_core` (shared with `static_scene_mode.md`)
- [ ] Scripted `InputActionMessage` replay mechanism (native + WASM)
- [ ] Per-tick quantized state hash (position/velocity, NaN-canonicalized)
- [ ] Harness comparison across native / WASM-Chrome / WASM-Firefox / dev / release
- [ ] Full regression pass: `cargo test -p ironhold_core --test '*'` (one-file-at-a-time per
      root `CLAUDE.md`'s disk-safety loop) — note this suite already runs `Fixed{1/64}` and cannot
      by itself prove v1 correct; full playtest matrix on jump/slope/coyote-time behavior is the
      real gate
- [ ] Docs: update `docs/40_determinism_and_networking.md` with the harness results (see doc
      corrections tracked alongside this feature)

## Open questions

- Interpolation vs. extrapolation for the render-facing transform between physics ticks — affects
  perceived input latency, matters more once client-side prediction is in scope.
- Whether `motion_system` moving into `FixedUpdate` (see Tasks) visibly changes bob/spin feel on
  any shipped project — check against existing motion-carrying prefabs during playtest.

## Acceptance criteria

- Given the same scripted input stream, native and WASM builds produce identical per-tick state
  hashes across the full matrix (native/WASM-Chrome/WASM-Firefox/dev/release) — or, if not, the
  harness identifies the exact tick and quantity that diverged.
- Given the fixed-timestep change alone, existing jump/slope/coyote-time playtests
  (`local_coop_demo`, `3rd_person_game_demo`) show no regression versus the pre-change baseline,
  confirmed by Frank — this is the primary correctness signal for v1, since the existing automated
  suite already runs against `TimestepMode::Fixed` and cannot independently verify the change.
- Given the harness's result, `planning/features/networking_multiplayer.md`'s Form 1 sync-strategy
  choice (lockstep vs. server-authoritative) is made from evidence, not assumption.
