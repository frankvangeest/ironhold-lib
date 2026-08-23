---
name: slope-jump-test-harness-gotchas
description: player_slope_jump_tests' harness climbs off its own 60m test slab at tick ~215 onto a legitimately-walkable 30deg end cap, and its settle loop never runs player_movement_system so coyote_ticks_remaining is always 0 at the first step()
metadata:
  type: project
---

Two non-obvious properties of `crates/ironhold_core/tests/player_slope_jump_tests.rs`'s
`setup_case`/`step` harness that make assertions mean less than they appear to:

**1. The player climbs off the test slab.** `step(moving: true, ...)` writes `linvel.x` directly
and movement never consults `is_grounded` (deliberate — see `MovementConfig
::max_walkable_slope_deg`'s doc comment: an "unwalkable" incline is still walked up at full
speed). At `run_speed 10` on the 60° `Collider::cuboid(60.0, 0.25, 60.0)`, the player reaches
x≈29.9 / y≈52.1 — the slab's top edge — at **tick 215**. The slab's local +X end cap has normal
(1,0,0), which rotated 60° about Z becomes (0.5, 0.866, 0) = 30° from vertical, i.e. *legitimately
walkable* under the 45° default. So `unwalkable_slope_never_reports_grounded`'s 200-tick loop has
only ~7% margin, and past that point the "unwalkable slide" tests are really measuring a walkable
end cap. Anything that speeds the climb (run_speed, friction, damping, a rapier bump) flips them
to failing with a message that blames the slope gate.

**2. `coyote_ticks_remaining` is always 0 at the first `step()`.** `player_movement_system` is
registered in `FixedUpdate` (`lib.rs`), and the harness pins
`TimeUpdateStrategy::ManualDuration(ZERO)`, so the 40-iteration `app.update()` settle loop steps
Rapier (`TimestepMode::Fixed`) but never runs the movement system. No test can therefore observe a
*live* coyote counter arriving on new geometry — e.g. walking from walkable ground onto an
unwalkable face — without seeding the field by hand.

**Why:** both were found by probe-instrumenting the harness during the coyote-time review; neither
is visible from reading the test file. Property 2 is why the unwalkable-slope tests can assert
`!grounded` on the very first tick at all.

**How to apply:** when a slope/jump test's result depends on where the player *is*, log
`Transform.translation` before trusting it, and prefer `moving: false` for pure grounding
assertions. To exercise a coyote transition, seed `coyote_ticks_remaining` explicitly. Verified
safe by probe: on a genuinely unwalkable face `raw_grounded` never becomes true, so a seeded coyote
counter yields exactly one extra bounded jump and the reset can never fire — the slope gate holds.
See [[is-grounded-overloaded-three-consumers]].
