---
name: is-grounded-overloaded-three-consumers
description: LocomotionState.is_grounded feeds animation AND can_jump's branch selection but deliberately NOT the jumps_used reset — so coyote buffering silently lengthens the double-jump dead window and a large coyote_time_secs disables double jump entirely
metadata:
  type: project
---

`player_movement_system` (`crates/ironhold_core/src/capabilities/player.rs`) computes three
different "grounded" values from one shape-cast, and each consumer reads a different one:

- `raw_grounded` (local) — the un-debounced sensor+slope reading. **Only** the
  `jumps_used`/`jump_liftoff_y` reset reads this. Reading the buffered value here was tried and
  reverted (it let the `risen_since_liftoff` fallback fire mid-ascent on a flat-ground jump).
- `LocomotionState.is_grounded` = `raw_grounded || coyote_ticks_remaining > 0` — read by
  `animation_resolver.rs` (airborne clip) **and** by `can_jump`'s grounded-vs-airborne branch.
- `jump_air_grace` — gates only the reset, never `is_grounded`.

**Why:** `can_jump`'s branch selection reading the *buffered* value is what couples coyote time to
double jump. The grounded branch is `jumps_used == 0`, so coyote can never *grant* an extra jump
(structurally safe — the ground jump sets `jumps_used = 1` in the same tick). But it can only be
reached while buffered-grounded, so it *withholds* the airborne branch. Measured at shipped
defaults (radius 0.4 + cast 0.3 = 0.70 m reach, v 5.94, GRAVITY 9.81): real detach at tick +9
(141 ms), coyote 6 ticks, so the second jump first lands at tick **+15 (234 ms)** — up from 141 ms
pre-coyote. At `coyote_time_secs: 0.3` it's 438 ms. Jump input is `just_pressed`
(`runtime/input.rs`) with no buffering, so a press in that window is *discarded*, not queued. At
`coyote_time_secs: 100` the airborne branch is never reachable: single jumps keep working, double
jump is silently off forever.

**How to apply:** any future change to what `is_grounded` means must be evaluated against all
three consumers separately — a change that is right for animation is not automatically right for
`can_jump`, and neither is right for the reset. Coyote forgiveness is only meaningful when
`jumps_used == 0`; gating it on that (`raw_grounded || (coyote > 0 && jumps_used == 0)`) removes
both the dead-window regression and the large-value trap in one line. Note `coyote_time_secs` has
no runtime `warn!` and no `ironhold_cli validate --strict` check, unlike its sibling
`max_walkable_slope_deg` (which got both) — see [[slope-jump-test-harness-gotchas]].
