---
name: coyote-time-has-a-real-upper-bound
description: coyote_time_secs >= ~0.8s makes LocomotionState.is_grounded never go false during a normal jump, silently killing the airborne animation and the jump_exit landing event — contradicting the code comments that claim any non-negative value is valid
metadata:
  type: project
---

`CharacterController.coyote_ticks_remaining` refreshes to `coyote_ticks(coyote_time_secs)` on
*every* tick the raw ground sensor reports walkable — including the ~8 ticks of residual sensor
contact right after a jump impulse. So the buffer is full at real liftoff and only then starts
counting down, meaning `loco.is_grounded` stays `true` for roughly `sensor_clear_time +
coyote_time_secs` into every jump. When that exceeds the jump's own airtime, `is_grounded` never
goes false for the whole arc.

Measured 2026-08-23 at shipped defaults (jump apex 1.5 m, airtime ~1.21 s, `collider_radius` 0.4 +
`ground_cast_length` 0.3), counting ticks where `is_grounded == false` over a full jump:

| `coyote_time_secs` | ungrounded ticks | `jump_exit` fired |
|---|---|---|
| 0.1 (default) | 46 | yes |
| 0.5 | 20 | yes |
| 0.8 | **1** | yes |
| >= 1.0 | **0** | **no** |

Two consequences, both animation-only (apex was 1.502 m in every row — physics is untouched):
`animation_resolver.rs`'s `else if !loco.is_grounded` airborne branch never runs, and the
`!was_grounded && loco.is_grounded` edge that pushes `"jump_exit"` never fires, so the landing clip
is lost. At 0.8 the fall state flashes for a single tick, which looks worse than not at all.

**Why it matters:** `validate.rs`'s `--strict` check and
`scene_loader.rs::warn_negative_coyote_time_secs` both explicitly assert the opposite — "any
non-negative value just makes the debounce buffer bigger or smaller" — and therefore only warn on
negatives. That claim is false; there is a real upper bound, and it is jump-height-dependent
(roughly: airtime `2v/g` minus sensor-clear time), not a fixed number.

**How to apply:** no shipped project is affected (`3rd_person_game_demo` authors 0.1, everything
else defaults to 0.1), so this is latent/designer-facing. If a "falling animation never plays" or
"landing animation missing" report ever arrives, check `coyote_time_secs` against the prefab's jump
apex before touching the animation resolver. The double-jump path is *not* affected — the
deliberately asymmetric `can_jump` (`raw_grounded || (coyote > 0 && jumps_used == 0)`) holds up:
verified 2 jumps at both 0.1 and 1.5. See [[ground-cast-sees-sensors]],
[[is-grounded-overloaded-three-consumers]].
