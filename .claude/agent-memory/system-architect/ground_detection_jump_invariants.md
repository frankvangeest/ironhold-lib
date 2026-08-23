---
name: ground-detection-jump-invariants
description: Ground-cast geometry (detach needs apex > collider_radius+ground_cast_length), the jumps_used reset that starves on slopes/low jumps, is_grounded's readers, the raw_grounded-vs-published-is_grounded split coyote time introduced, and the two-physics-clocks trap (Rapier steps in PostUpdate at Variable dt while our systems tick FixedUpdate at 64 Hz)
metadata:
  type: project
---

Durable facts about `capabilities/player.rs`'s ground detection and jump bookkeeping, worked out
during the `uphill_jump_lock.md` plan review (2026-08-19).

**Ground-cast geometry.** The cast is a `Collider::ball(collider_radius)` swept from the *entity
origin* (feet) down `ground_cast_length`, `.is_some()`-tested. Geometrically it reports grounded
whenever the ground surface is within `collider_radius + ground_cast_length` of the origin — 0.7 m
at shipped defaults (0.4 + 0.3). So **the player must rise > 0.7 m before `is_grounded` can ever
go false.**

**Consequence — the jump-lock bug class is wider than slopes.** `jumps_used` resets *only* on the
`!was_grounded && is_grounded` edge. Any configuration whose jump apex is below that 0.7 m detach
threshold never produces the edge and locks jumping permanently (`double_jump` defaults off, so
`can_jump` is then always false). `jump_velocity = sqrt(2 * GRAVITY * h)` (`scene_loader.rs`'s
`resolve_jump_velocity`, `GRAVITY = 9.81`), so an authored `jump: RelativeToHeight(percent: 30)` on
a 1.8 m player (apex 0.54 m) reproduces the lock **on flat ground**, no slope needed. Raising
`ground_cast_length` (which the docs actively recommend for uneven terrain) widens the same trap.
Any fix sized by a fixed number of seconds/ticks is therefore wrong somewhere in the authorable
parameter space unless it's derived from `collider_radius`, `ground_cast_length`, and the actual
jump velocity.

**Why over-shooting a grace window is safe and under-shooting is not.** Anything that re-enables the
`jumps_used` reset before the body has cleared the 0.7 m cast reach turns jump into a hover/rocket
(jump re-fires every window; `velocity.linvel.y` is *set*, not accumulated, so it reads as sustained
lift). `primitive_world/logic/state_machine.ron` binds `player.jumped` → `PlaySound("jump")`, so the
same failure is audible, not just visual. Flat-ground detach time at defaults is ~0.13 s — any
"~0.1 s" window straddles that boundary.

**`LocomotionState.is_grounded` has exactly two readers**: `player_movement_system` (landing edge +
`can_jump`) and `animation_resolver.rs:176` (`jump_loop` selection). Nothing filters
`Changed<LocomotionState>` or `Changed<CharacterController>`. Also note the resolver's *override*
branch (`active.clip.is_some()`) wins before `is_grounded` is consulted at all, and every shipped
policy gives `jump_enter` `priority: 200, duration: 0.4` — so a sub-0.4 s lie about `is_grounded`
would be invisible in animation anyway. `npc.rs:503` is the only other writer.

**Test-harness clock hazard.** `crates/ironhold_core/tests/` drives `player_movement_system` via
`run_system_once`, *outside* `FixedUpdate` (see the `tmp_slope_jump.rs` shape:
`Time::<Fixed>::from_seconds(1000.0)` to suppress the real schedule, `TimestepMode::Fixed` for
Rapier, manual per-tick loop). In that shape `Res<Time>` is `Time<()>` carrying the **real
wall-clock frame delta**, not the fixed delta — so any `time.delta_secs()`-based countdown added to
this system is both nondeterministic and effectively untestable there. Prefer an integer tick
counter for new per-tick state in this system. See [[schedule_update_vs_fixedupdate]].

**Two physics clocks — the trap for any tick-based timer in `player_movement_system`.** Our systems
run in `FixedUpdate` (64 Hz, Bevy's `Time::<Fixed>::DEFAULT_TIMESTEP`), but **Rapier itself does
not**: `capabilities/physics.rs` adds `RapierPhysicsPlugin::<NoUserData>::default()`, whose default
is `schedule: PostUpdate` (bevy_rapier3d 0.33 `plugin.rs` `impl Default`, ~line 204) with
`TimestepMode::Variable { max_dt: 1/60, time_scale: 1.0 }` (`configuration.rs:53`, and `dt =
min(delta * time_scale, max_dt)` in `context/mod.rs:786`). So gravity/position integration advances
once per *rendered frame*, clamped at 1/60 s. Consequences: at ≥60 fps physics ≈ real time and 64
ticks ≈ 1 s of physics; **below 60 fps physics enters slow-motion while `FixedUpdate` keeps ticking
at 64/s**, so any counter measuring FixedUpdate ticks drains faster than the physics it is
estimating (2x at 30 fps; a `Time<Virtual>` max_delta 250 ms hitch burns 16 ticks against 1/60 s of
physics). `bevy_rapier` offers `.in_fixed_schedule()` + an explicit `TimestepMode::Fixed { dt:
1.0/64.0 }` to collapse the two clocks into one — not currently used, despite the
`enhanced-determinism` feature being on (see [[determinism_networking]]) and despite
`src/CLAUDE.md`'s "physics & movement must use FixedUpdate" heading (true of *our* systems only).
Note `tests/player_slope_jump_tests.rs` inserts `TimestepMode::Fixed { dt: 1.0/64.0 }`, so the test
harness is 1 tick : 1 physics step by construction and structurally cannot observe this skew.

**Coyote time split the grounded signal in two (added 2026-08-22, `feature/uphill-jump-lock`).**
`player_movement_system` now computes a function-local `raw_grounded` (sensor hit + slope-walkability)
and publishes `loco.is_grounded = raw_grounded || coyote_ticks_remaining > 0`. Only the *buffered*
value reaches the ECS; `raw_grounded` never leaves the function. So `LocomotionState.is_grounded` is
no longer "the sensor said so" — it is a debounced feel signal, and **any future consumer needing
true airtime (fall damage, footsteps, the queued step-offset backlog item) will silently get the
debounced one.** The `jumps_used` reset reads `raw_grounded` deliberately: feeding it the buffered
value made `risen_since_liftoff >= reach` (a check designed to fire *while still rising*, for the
climbing-slope case) fire on an ordinary flat-ground jump, resetting `jumps_used` mid-ascent —
caught by `grace_expiry_does_not_reset_early_when_real_physics_time_lags_ticks`. Second-order
consequence not yet documented anywhere: because `can_jump`'s branch is chosen by the *buffered*
value and `jumps_used == 1` blocks the grounded branch, the coyote window also delays *double-jump*
availability by `coyote_time_secs` after real detach.

**Tick-counting is the validated choice here, not a compromise.** Both `jump_air_grace_ticks()` and
`coyote_ticks()` multiply by a hardcoded `FIXED_TICK_RATE = 64.0` rather than reading `Time`. That is
deliberate and consistent with the test-harness clock hazard above. Coyote's exposure to the
two-clocks skew is genuinely benign (a hitch only makes the buffer last longer in wall-clock terms,
and it gates animation + jump-branch selection, never `jumps_used`), unlike `jump_air_grace`, which
needed physical-quantity backstops. If the tick rate ever becomes authorable, `Res<Time<Fixed>>`'s
`timestep()` is the safe source (correct outside `FixedUpdate` too) — but only while the harness
pins time via `TimeUpdateStrategy::ManualDuration` rather than the older
`Time::<Fixed>::from_seconds(1000.0)` trick.

**`CharacterController` construction fan-out**: 1 production site
(`entity_spawner.rs::spawn_player_entity_core`, ~line 1001) and ~13 full struct literals across
`tests/` (the rest use `..test_character_controller()` spreads). No `Default` impl, so adding a
field is a loud compile error at all of them. **Re-examined 2026-08-22 after 5 fields were added in
one feature: still no `Default`, and that is the right call** — the single production site must wire
every field from `MovementConfig`, so a `Default` there would let a newly added schema field silently
default instead of being plumbed (exactly the bug class this feature existed to fix). The churn is
entirely test-side, and `tests/support/mod.rs` (already imported by 7+ test files via `mod support;`)
is the correct relief valve — a shared `test_character_controller()` there would collapse the ~13
literals, several per-file duplicate helpers, and 7 inline literals in `action_tests.rs`.
See [[player_spawn_paths]].
