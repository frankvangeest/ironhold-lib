---
name: jump-rearm-coupling
description: Jump re-arm depends on the ground-check sphere-cast clearing; couples MovementConfig jump height, collider_radius/primitive.radius and ground_cast_length into one undocumented invariant
metadata:
  type: project
---

A player's jump is re-armed (`jumps_used` reset) **only** on a `not grounded -> grounded` edge of
the downward sphere-cast ground check. Effective cast reach is `collider_radius + ground_cast_length`
(primitive players substitute `primitive.radius`). Consequence: any authored combination where the
jump's rise never exceeds that reach can never produce the edge, and the jump dies silently — the
"uphill jump lock" bug is one instance (a climbing slope closes the gap), a low `jump:` height or an
inflated `ground_cast_length` is another.

Designer-authorable fields entangled in this one invariant: `jump` (JumpConfig), `double_jump`,
`collider_radius` / `primitive.radius`, `collider_height`, `ground_cast_length`.

Doc state (checked 2026-08-19, HEAD 48edf00):
- `docs/20_data_formats.md` MovementConfig table (~2212-2231) documents each field in isolation.
  Nothing states that jumps re-arm on a landing edge, and the effective-reach formula appears nowhere.
- The `ground_cast_length` row actively advises "increase for uneven terrain or fast vertical
  movement" — which enlarges the false-positive window and makes the lock more likely, not less.
- `docs/STATUS.md:51` still lists only `walk_speed, run_speed, rot_speed, jump, double_jump,
  collider_radius, collider_height` — stale, missing `double_jump_height, idle_drag, linear_damping,
  angular_damping, ground_cast_length`.

Designer-visible side effects of any change to grounded state:
- `loco.is_grounded == false` selects the `base.jump_loop` clip; the landing edge also pushes the
  reserved `jump_exit` override (`Jump_Land`) — so a *fake* landing edge produces a visible land
  animation mid-air.
- Every successful jump emits `player.jumped`; `primitive_world/logic/state_machine.ron` binds
  `PlaySound(key: "jump")` to it, making primitive_world the best audible canary for re-jump cadence
  regressions. `double_jump: true` ships in primitive_world, particles_demo, effect_mayhem_demo,
  stats_demo.

**Why:** reviewed for `planning/features/uphill_jump_lock.md`; the proposed fix (forced-airborne
grace window) is tuned against shipped defaults only, so every one of the coupled fields above can
re-break or over-trigger it.

**How to apply:** on any movement/ground-detection/jump change, check all five coupled fields, and
require both a MovementConfig-table callout in `docs/20_data_formats.md` and a primitive_world
audio/animation-cadence playtest step — not just a `crates/.../CLAUDE.md` note.
