# Feature: Deterministic Fixed Timestep + Cross-Platform Divergence Harness

_Status: In Progress (v1 Done, v2 Queued)_
_Planned at: `7332d62` (2026-09-15)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Fixed-timestep Rapier physics | Done | `6f720de` (2026-09-16) |
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

**v1 (done, pending Frank's playtest — step 9 of the code change workflow):**

- [x] `capabilities/physics.rs`: `TimestepMode::Fixed { dt: 1.0/64.0, .. }` (insert the resource
      before `add_plugins`) + `in_fixed_schedule()`
- [x] Order the existing `FixedUpdate` gameplay chain (`lib.rs`) `.before(PhysicsSet::
      SyncBackend)` — folded `motion_system` into the same chain (see below) rather than a
      second, independently-ordered `.before(...)` call, since two unordered groups both writing
      `&mut Transform` would themselves be an ambiguity
- [x] Derive `FIXED_TICK_RATE` and `TimestepMode::Fixed.dt` from one shared constant — **and**
      make it authoritative over the schedule's own rate too: `PhysicsPlugin::build` now also
      inserts `Time::<Fixed>::from_hz(FIXED_TICK_RATE as f64)`. Found during v1 review
      (system-architect + alignment-reviewer, independently): the constant only *coincided* with
      Bevy's own `FixedUpdate` default before this, it didn't *drive* it — a future Bevy default
      change or an unrelated slow-motion/pause feature's own `Time::<Fixed>` call could have
      silently desynced solver `dt` from the schedule's actual tick rate.
- [x] Decide and implement render-side smoothing (interpolation vs. accept aliasing) — **decided:
      accept aliasing, no `TimestepMode::Interpolated`** (would reintroduce the wall-clock
      coupling this feature removes). Native's framepace (`lib.rs`, `FramepaceSettings`) was
      matched to `FIXED_TICK_RATE` (was an independent `60.0` literal), reasoned at the time (v1
      code review, debug-detective) to make a normal frame advance physics by exactly one tick.
      **Corrected during the v1 *playtest* (a second, independent debug-detective investigation):
      this does not actually work.** `Window`'s default `PresentMode::Fifo` (vsync) means the real
      present rate is the display's refresh rate, not this cap, and no common display refresh rate
      evenly divides 64Hz — a 60Hz display still produces ~4 double-physics-tick frames/second
      (144Hz produces *more*, ~16/s, not fewer). Multi-tick frames are routine, not a rare
      `Time<Virtual>::max_delta`-catch-up edge case. Kept the framepace-matching change anyway (a
      15.625ms sleep target under a 16.67ms vsync period never overshoots, marginally safer than
      the old independent `60.0`), but the actual fix for double-tick-frame artifacts is two
      separate things, in two different schedules — see the next item (`mark_dirty_trees`, for
      reads *inside* `FixedUpdate`) and the nameplate-stutter item further below (`fresh_global_
      transform`, for reads in `Update`).
- [x] `det_math` helper (or equivalent) routing `player.rs`'s slope-angle `acos`, `player.rs`'s
      turn rotation, and `motion.rs`'s rotation/bob math through `libm` — module doc comment
      includes a "Known residual" section (glam's per-SIMD-backend `Quat`/`Affine3A`
      composition is not itself bit-identical native/WASM even though the underlying scalar
      `libm` calls are; found during v1 review, system-architect + debug-detective independently)
- [x] Decide whether `motion_system` moves from `Update` into `FixedUpdate` — **decided: yes**,
      moved (see `motion.rs`'s doc comment)
- [x] Add `bevy::transform::systems::mark_dirty_trees` to the `FixedUpdate` chain, ordered before
      `PhysicsSet::SyncBackend` — **not originally scoped; found during v1 review**
      (debug-detective, verified against vendored `bevy_transform`/`bevy_rapier3d` source):
      Rapier's own `SyncBackend` runs Bevy's `propagate_parent_transforms`/`sync_simple_
      transforms` but not `mark_dirty_trees`, and `propagate_parent_transforms`'s static-scene
      optimization skips any subtree `mark_dirty_trees` hasn't just flagged — on a frame that
      runs two `FixedUpdate` ticks (a real, if now rarer, possibility via `Time<Virtual>::
      max_delta` catch-up), the second tick's `GlobalTransform` could go one tick stale for
      anything the chain just moved, read as a stale `feet_pos` by the ground cast.
- [ ] Re-tune jump/coyote-time constants against the fixed clock — **decided to defer, not
      dropped**: `JUMP_AIR_GRACE_SAFETY` and the two physical belt-and-braces checks in
      `player_movement_system`'s jump-reset logic are kept exactly as-is rather than guessed at
      without playtest evidence; the surrounding doc comments were updated to explain *why* they
      remain (defense-in-depth against a substep-count/drag-tuning change, not the two-clock
      lag they used to guard against). Revisit only if the playtest (step 9) surfaces a concrete
      reason to.
- [x] Rewrite `player_slope_jump_tests.rs`'s `grace_expiry_does_not_reset_early_when_real_physics_
      time_lags_ticks` doc comment (and `setup_case_full`'s, which described the same premise) —
      the two-clocks scenario they simulate is no longer reachable in the *shipped* game once `dt`
      is hardcoded to `FIXED_TICK_RATE`; kept as a synthetic probe of the physical checks, not a
      reachable production case. Underlying assertions unchanged.
- [x] Full regression pass: `cargo test -p ironhold_core --test '*'` (one-file-at-a-time per
      root `CLAUDE.md`'s disk-safety loop) — all green except a pre-existing, unrelated failure
      in `entity_logic_tests.rs` (per-player action-bar `{target}` substitution bug, confirmed via
      `git stash` to already fail identically on the unmodified base commit; logged to
      `planning/backlog.md`'s Bugs section, not part of this feature). Also required, not
      originally scoped: pinning `TimeUpdateStrategy::ManualDuration` in `tests/support/
      mod.rs::setup_test_app()` (found during v1 review, system-architect — **critical**: moving
      Rapier into `FixedUpdate` made every other integration test's physics stepping
      wall-clock-gated and nondeterministic, not just the three files this feature's harness work
      already touched) and reworking those same three files' `step()` mechanism, since
      `TimeUpdateStrategy::ManualDuration(Duration::ZERO)` + a manual `run_system_once(player_
      movement_system)` (their previous mechanism for driving movement in isolation while physics
      still stepped for free via the old unconditional `PostUpdate` schedule) stopped working
      once physics moved into the same schedule the manual duration was freezing.
- [x] Docs: `docs/40_determinism_and_networking.md`, `docs/STATUS.md`, `crates/ironhold_core/src/
      CLAUDE.md`, `crates/ironhold_core/tests/CLAUDE.md` all updated to reflect the landed fixed
      tick, `det_math`, and the render-smoothing decision.
- [x] **Fix world-space UI/camera staleness found during Frank's playtest (step 9) — not caught by
      code review or the automated suite.** Symptom: the floating health-bar/name label above the
      player visibly stuttered in `3rd_person_game_demo` (pre-existing on `main` in a much milder
      form; this feature made it clearly worse). Root cause (debug-detective, verified against
      vendored `bevy_transform`/`bevy_rapier3d` source): `world_label_screen_pos_system` and
      `target_indicator_system` (both `Update`-scheduled) read `&GlobalTransform` for the camera
      and the tracked/target entity, but `GlobalTransform` is one tick stale relative to the
      `Transform` the camera chain (also `Update`) just wrote *this same frame* — the two
      staleness errors used to cancel (camera and trackee both exactly one *frame* behind) but no
      longer do once physics ticks and render frames decouple, so a double-tick frame (routine, see
      above) pops the label/ring by one tick of motion. Fixed by a new `crate::utils::
      fresh_global_transform(transform: Option<&Transform>, global: &GlobalTransform, child_of:
      Option<&ChildOf>) -> GlobalTransform` helper — for a root-level entity, constructs
      `GlobalTransform::from(*transform)` directly (zero lag, bit-identical to what propagation
      would compute) instead of reading the possibly-stale component; falls back to the real
      component for a parented entity (none exist at any real call site today). Applied at:
      `world_label_screen_pos_system` (also gained a real, previously-missing `.after(
      camera_blend_system)` ordering fix — was unordered relative to the whole camera chain),
      `target_indicator_system`, `fixed_camera_system`'s `look_at_entity` (whole-viewport jitter
      in `Fixed` camera mode, not just a distance check — mis-triaged as harmless in the first
      draft of this fix, corrected by a second debug-detective pass), and `fading_decal_system`
      (a tracked decal sliding behind a moving target). Two iterations were needed to get the
      query changes right: the first attempt added `&Transform` as a hard query requirement and
      broke a Bevy query-conflict rule (`camera_q` vs. `label_q`'s `&mut Transform` — fixed with a
      `Without<WorldLabel>` filter); the second broke several test fixtures that spawn entities
      with `GlobalTransform` but no `Transform` component at all, since a hard `&Transform`
      requirement silently drops such an entity out of the query match entirely, not just skips
      the freshness optimization — fixed by making it `Option<&Transform>`. Both caught by the
      full `ironhold_core` test suite before reaching Frank again. 4 new unit tests in
      `utils.rs` pin the helper's truth table. Verified via 4 parallel post-fix reviews
      (alignment, architecture, debug-detective, wasm-perf) — no blocking findings; see
      `planning/claude_suggestions.md` for the logged non-blocking follow-ups (a few structurally
      identical call sites deliberately left unconverted, a `QueryData` bundle refactor, a named
      `SystemSet` to make the camera-chain ordering less fragile).

**v1 follow-ups, not blocking (see `planning/claude_suggestions.md`):** one-time spawn-time
rotation construction (`Quat::from_euler` etc. in `scene_loader.rs`/`entity_spawner.rs`/
`action_executor.rs`, setting static collider orientation) is still `std`-backed, not `det_math`
— found during v1 review (debug-detective); arguably a larger determinism hole than the per-tick
sites fixed here (it sets initial state the simulation never converges back from), but out of
scope for v1's own goal (fixing the variable timestep) and not required by its acceptance
criteria below.

**v2 (queued, unchanged by v1's review):**

- [ ] `RunMode`/`StartOptions` seam in `ironhold_core` (shared with `static_scene_mode.md`)
- [ ] Scripted `InputActionMessage` replay mechanism (native + WASM)
- [ ] Per-tick quantized state hash (position/velocity, NaN-canonicalized)
- [ ] Harness comparison across native / WASM-Chrome / WASM-Firefox / dev / release — now also
      the mechanism that would catch the `det_math` "Known residual" (glam backend-split
      quaternion math) and the deferred spawn-time-rotation gap noted above

## Open questions

- Interpolation vs. extrapolation for the render-facing transform between physics ticks — affects
  perceived input latency, matters more once client-side prediction is in scope. (v1 answered the
  *current* question — accept aliasing, no interpolation for now — this remains open only for a
  future prediction-focused revisit.)
- ~~Whether `motion_system` moving into `FixedUpdate` visibly changes bob/spin feel on any shipped
  project~~ — **answered during v1 review (debug-detective, system-architect):** yes, on any
  display refreshing faster than 64Hz, motion now visibly steps rather than updating every
  rendered frame. Accepted as part of the render-smoothing decision above; name it explicitly in
  the step-8 playtest checklist (`quick_scene`'s spinning collectibles) rather than leaving it to
  be rediscovered.

## Acceptance criteria

- Given the same scripted input stream, native and WASM builds produce identical per-tick state
  hashes across the full matrix (native/WASM-Chrome/WASM-Firefox/dev/release) — or, if not, the
  harness identifies the exact tick and quantity that diverged.
- Given the fixed-timestep change alone, existing jump/slope/coyote-time playtests
  (`local_coop_demo`, `3rd_person_game_demo`) show no regression versus the pre-change baseline,
  confirmed by Frank — this is the primary correctness signal for v1, since the existing automated
  suite already runs against `TimestepMode::Fixed` and cannot independently verify the change.
  **This criterion did its job**: jump/slope/movement itself showed no regression on first
  playtest, but Frank caught a real regression this criterion was designed to catch — a
  nameplate/health-bar stutter, worse than the pre-existing (barely-noticeable) baseline on
  `main` — which the automated suite had no way to see. Fixed (see the Tasks list above,
  `fresh_global_transform`) and re-playtested before this criterion is considered met.
- Given the harness's result, `planning/features/networking_multiplayer.md`'s Form 1 sync-strategy
  choice (lockstep vs. server-authoritative) is made from evidence, not assumption.
