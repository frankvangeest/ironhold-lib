---
name: jump-rearm-coupling
description: Jump re-arm depends on the ground-check sphere-cast clearing; couples MovementConfig jump height, collider_radius/primitive.radius and ground_cast_length into one invariant — now documented as a callout in docs/20 (~2293-2295)
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

**CLOSED — the re-arm formula is now documented.** `docs/20_data_formats.md`'s MovementConfig
section (~2293-2295) has a blockquote: "A jump must clear `collider_radius + ground_cast_length`,
or it re-arms on a delay instead of instantly" — explains the sphere-cast mechanic, the apex-vs-reach
relationship, and the internal delayed-fallback re-arm. Do not re-flag this as undocumented.

Still-open doc gap: the `ground_cast_length` row still advises "increase for uneven terrain or fast
vertical movement" without a cross-reference back to the invariant blockquote — a designer reading
only the field table (not the later callout) can still enlarge the false-positive window. Verify
`docs/STATUS.md`'s MovementConfig field list is current before citing it (was stale as of
2026-08-19, missing `double_jump_height, idle_drag, linear_damping, angular_damping,
ground_cast_length`).

Designer-visible side effects of any change to grounded state:
- `loco.is_grounded == false` selects the `base.jump_loop` clip; the landing edge also pushes the
  reserved `jump_exit` override (`Jump_Land`) — so a *fake* landing edge produces a visible land
  animation mid-air.
- Every successful jump emits `player.jumped`; `primitive_world/logic/state_machine.ron` binds
  `PlaySound(key: "jump")` to it, making primitive_world the best audible canary for re-jump cadence
  regressions. `double_jump: true` ships in primitive_world, particles_demo, effect_mayhem_demo,
  stats_demo.

**Playtest-target facts (verified 2026-10-02, airborne_ground_reacquisition plan-review):**
- `3rd_person_game_demo` main scene is a FLAT 100x100 cuboid — no hills/ledges. Its only raised
  geometry: `loot_display_01` (0.2 m platform + chest top ~1.0), `chest_01` (top 0.8, compound
  collider), `anvil_01`. Plans that say "3rd_person hills" are wrong.
- `quick_scene` (heightmap terrain + `player_warrior`) is the slope/uphill-cadence canary.
- `primitive_world` is the ONLY project binding `player.jumped` (PlaySound "jump") — the only way
  to observe jump count in-browser; also has double_jump: true.
- room10 `cube_obstacle_room10` top = 1.2 m; with default 1.8 apex it is the only shipped surface
  in the "apex-hover" band (top within 0.7 m below apex). room10 P2 has no rig → no landing clip.
- Landing clip (`jump_exit`) fires when within reach on the way DOWN (~0.13 s early on flat,
  up to ~0.35 s over a near-apex platform) — undocumented in the docs/20 jump_exit callout.

**Why:** reviewed for `planning/features/uphill_jump_lock.md`; the proposed fix (forced-airborne
grace window) is tuned against shipped defaults only, so every one of the coupled fields above can
re-break or over-trigger it.

**How to apply:** on any movement/ground-detection/jump change, check all five coupled fields, and
require both a MovementConfig-table callout in `docs/20_data_formats.md` and a primitive_world
audio/animation-cadence playtest step — not just a `crates/.../CLAUDE.md` note.
